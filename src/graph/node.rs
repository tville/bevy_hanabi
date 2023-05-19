use std::num::NonZeroU32;

use crate::{
    graph::expr::Handle, Attribute, AttributeExpr, BuiltInExpr, BuiltInOperator, Expr, ExprError,
    UnaryNumericOperator, ValueType,
};

/// Identifier of a node in a graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(NonZeroU32);

impl NodeId {
    /// Create a new node identifier.
    pub fn new(id: NonZeroU32) -> Self {
        Self(id)
    }

    /// Get the one-based node index.
    pub fn id(&self) -> NonZeroU32 {
        self.0
    }

    /// Get the zero-based index of the node in the underlying graph node array.
    pub fn index(&self) -> usize {
        (self.0.get() - 1) as usize
    }
}

/// Identifier of a slot in a graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SlotId(NonZeroU32);

impl SlotId {
    /// Create a new slot identifier.
    pub fn new(id: NonZeroU32) -> Self {
        Self(id)
    }

    /// Get the one-based slot index.
    pub fn id(&self) -> NonZeroU32 {
        self.0
    }

    /// Get the zero-based index of the slot in the underlying graph slot array.
    pub fn index(&self) -> usize {
        (self.0.get() - 1) as usize
    }
}

/// Node slot direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SlotDir {
    /// Input slot receiving data from outside the node.
    Input,
    /// Output slot providing data generated by the node.
    Output,
}

/// Definition of a slot of a node.
#[derive(Debug, Clone)]
pub struct SlotDef {
    /// Slot name.
    name: String,
    /// Slot direaction.
    dir: SlotDir,
    /// Type of values accepted by the slot. This may be `None` for variant
    /// slots, if the type depends on the inputs of the node during evaluation.
    value_type: Option<ValueType>,
}

impl SlotDef {
    /// Create a new input slot.
    pub fn input(name: impl Into<String>, value_type: Option<ValueType>) -> Self {
        Self {
            name: name.into(),
            dir: SlotDir::Input,
            value_type,
        }
    }

    /// Create a new output slot.
    pub fn output(name: impl Into<String>, value_type: Option<ValueType>) -> Self {
        Self {
            name: name.into(),
            dir: SlotDir::Output,
            value_type,
        }
    }

    /// Get the slot name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the slot direction.
    pub fn dir(&self) -> SlotDir {
        self.dir
    }

    /// Get the slot value type.
    pub fn value_type(&self) -> Option<ValueType> {
        self.value_type
    }
}

/// Single slot of a node.
#[derive(Debug, Clone)]
pub struct Slot {
    /// Owner node identifier.
    node_id: NodeId,
    /// Identifier.
    id: SlotId,
    /// Slot definition.
    def: SlotDef,
    /// Linked slots.
    linked_slots: Vec<SlotId>,
}

impl Slot {
    /// Create a new slot.
    pub fn new(node_id: NodeId, slot_id: SlotId, slot_def: SlotDef) -> Self {
        Slot {
            node_id,
            id: slot_id,
            def: slot_def,
            linked_slots: vec![],
        }
    }

    /// Get the node identifier of the node this slot is from.
    pub fn node_id(&self) -> NodeId {
        self.node_id
    }

    /// Get the slot identifier.
    pub fn id(&self) -> SlotId {
        self.id
    }

    /// Get the slot definition.
    pub fn def(&self) -> &SlotDef {
        &self.def
    }

    /// Get the slot direction.
    pub fn dir(&self) -> SlotDir {
        self.def.dir()
    }

    /// Check if this slot is an input slot.
    ///
    /// This is a convenience helper for `self.dir() == SlotDir::Input`.
    pub fn is_input(&self) -> bool {
        self.dir() == SlotDir::Input
    }

    /// Check if this slot is an output slot.
    ///
    /// This is a convenience helper for `self.dir() == SlotDir::Output`.
    pub fn is_output(&self) -> bool {
        self.dir() == SlotDir::Output
    }

    /// Link this output slot to an input slot.
    ///
    /// # Panic
    ///
    /// Panics if this slot's direction is `SlotDir::Input`.
    fn link_to(&mut self, input: SlotId) {
        assert!(self.is_output());
        if !self.linked_slots.contains(&input) {
            self.linked_slots.push(input);
        }
    }

    fn unlink_from(&mut self, input: SlotId) -> bool {
        assert!(self.is_output());
        if let Some(index) = self.linked_slots.iter().position(|&s| s == input) {
            self.linked_slots.remove(index);
            true
        } else {
            false
        }
    }

    fn link_input(&mut self, output: SlotId) {
        assert!(self.is_input());
        if self.linked_slots.is_empty() {
            self.linked_slots.push(output);
        } else {
            self.linked_slots[0] = output;
        }
    }

    fn unlink_input(&mut self) {
        assert!(self.is_input());
        self.linked_slots.clear();
    }
}

/// Effect graph.
pub struct Graph {
    nodes: Vec<Box<dyn Node>>,
    slots: Vec<Slot>,
}

impl std::fmt::Debug for Graph {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Graph").field("slots", &self.slots).finish()
    }
}

impl Graph {
    /// Create a new graph.
    pub fn new() -> Self {
        Self {
            nodes: vec![],
            slots: vec![],
        }
    }

    /// Add a node to the graph.
    pub fn add_node(&mut self, node: Box<dyn Node>) -> NodeId {
        let index = self.nodes.len() as u32;
        let node_id = NodeId::new(NonZeroU32::new(index + 1).unwrap());

        for slot_def in node.slots() {
            let slot_id = SlotId::new(NonZeroU32::new(self.slots.len() as u32 + 1).unwrap());
            let slot = Slot::new(node_id, slot_id, slot_def.clone());
            self.slots.push(slot);
        }

        self.nodes.push(node);

        node_id
    }

    /// Link an output slot of a node to an input slot of another node.
    pub fn link(&mut self, output: SlotId, input: SlotId) {
        let out_slot = self.get_slot_mut(output);
        assert!(out_slot.is_output());
        out_slot.link_to(input);

        let in_slot = self.get_slot_mut(input);
        assert!(in_slot.is_input());
        in_slot.link_input(output);
    }

    /// Unlink an output slot of a node from an input slot of another node.
    pub fn unlink(&mut self, output: SlotId, input: SlotId) {
        let out_slot = self.get_slot_mut(output);
        assert!(out_slot.is_output());
        if out_slot.unlink_from(input) {
            let in_slot = self.get_slot_mut(input);
            assert!(in_slot.is_input());
            in_slot.unlink_input();
        }
    }

    /// Unlink all remote slots from a given slot.
    pub fn unlink_all(&mut self, slot_id: SlotId) {
        let slot = self.get_slot_mut(slot_id);
        let linked_slots = std::mem::take(&mut slot.linked_slots);
        for remote_id in &linked_slots {
            let remote_slot = self.get_slot_mut(*remote_id);
            if remote_slot.is_input() {
                remote_slot.unlink_input();
            } else {
                remote_slot.unlink_from(slot_id);
            }
        }
    }

    /// Get all slots of a node.
    pub fn slots(&self, node_id: NodeId) -> Vec<SlotId> {
        self.slots
            .iter()
            .filter_map(|s| {
                if s.node_id() == node_id {
                    Some(s.id())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get all input slots of a node.
    pub fn input_slots(&self, node_id: NodeId) -> Vec<SlotId> {
        self.slots
            .iter()
            .filter_map(|s| {
                if s.node_id() == node_id && s.is_input() {
                    Some(s.id())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get all output slots of a node.
    pub fn output_slots(&self, node_id: NodeId) -> Vec<SlotId> {
        self.slots
            .iter()
            .filter_map(|s| {
                if s.node_id() == node_id && s.is_output() {
                    Some(s.id())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Find a slot ID by slot name.
    pub fn get_slot_id<'a, 'b: 'a, S: Into<&'b str>>(&'a self, name: S) -> Option<SlotId> {
        let name = name.into();
        self.slots
            .iter()
            .find(|&s| s.def().name() == name)
            .map(|s| s.id)
    }

    #[allow(dead_code)] // TEMP
    fn get_slot(&self, id: SlotId) -> &Slot {
        let index = id.index();
        assert!(index < self.slots.len());
        &self.slots[index]
    }

    fn get_slot_mut(&mut self, id: SlotId) -> &mut Slot {
        let index = id.index();
        assert!(index < self.slots.len());
        &mut self.slots[index]
    }
}

/// Generic graph node.
pub trait Node {
    /// Get the list of slots of this node.
    ///
    /// The list contains both input and output slots, without any guaranteed
    /// order.
    fn slots(&self) -> &[SlotDef];

    /// Evaluate the node from the given input expressions, and optionally
    /// produce output expression(s).
    ///
    /// The expressions themselves are not evaluated (that is, _e.g._ "3 + 2" is
    /// _not_ reduced to "5").
    fn eval(&self, inputs: Vec<Handle<Expr>>) -> Result<Vec<Handle<Expr>>, ExprError>;
}

/// Graph node to add two values.
#[derive(Debug, Clone)]
pub struct AddNode {
    slots: [SlotDef; 3],
}

impl AddNode {
    /// Create a new node.
    pub fn new() -> Self {
        Self {
            slots: [
                SlotDef::input("lhs", None),
                SlotDef::input("rhs", None),
                SlotDef::output("result", None),
            ],
        }
    }
}

impl Node for AddNode {
    fn slots(&self) -> &[SlotDef] {
        &self.slots
    }

    fn eval(&self, inputs: Vec<Handle<Expr>>) -> Result<Vec<Handle<Expr>>, ExprError> {
        if inputs.len() != 2 {
            return Err(ExprError::GraphEvalError(format!(
                "Unexpected input count to AddNode::eval(): expected 2, got {}",
                inputs.len()
            )));
        }
        let mut inputs = inputs.into_iter();
        let lhs = inputs.next().unwrap();
        let rhs = inputs.next().unwrap();
        Ok(vec![Box::new(AddExpr::new(lhs, rhs))])
    }
}

/// Graph node to subtract two values.
#[derive(Debug, Clone)]
pub struct SubNode {
    slots: [SlotDef; 3],
}

impl SubNode {
    /// Create a new node.
    pub fn new() -> Self {
        Self {
            slots: [
                SlotDef::input("lhs", None),
                SlotDef::input("rhs", None),
                SlotDef::output("result", None),
            ],
        }
    }
}

impl Node for SubNode {
    fn slots(&self) -> &[SlotDef] {
        &self.slots
    }

    fn eval(&self, inputs: Vec<Handle<Expr>>) -> Result<Vec<Handle<Expr>>, ExprError> {
        if inputs.len() != 2 {
            return Err(ExprError::GraphEvalError(format!(
                "Unexpected input count to SubNode::eval(): expected 2, got {}",
                inputs.len()
            )));
        }
        let mut inputs = inputs.into_iter();
        let lhs = inputs.next().unwrap();
        let rhs = inputs.next().unwrap();
        Ok(vec![Box::new(SubExpr::new(lhs, rhs))])
    }
}

/// Graph node to multiply two values.
#[derive(Debug, Clone)]
pub struct MulNode {
    slots: [SlotDef; 3],
}

impl MulNode {
    /// Create a new node.
    pub fn new() -> Self {
        Self {
            slots: [
                SlotDef::input("lhs", None),
                SlotDef::input("rhs", None),
                SlotDef::output("result", None),
            ],
        }
    }
}

impl Node for MulNode {
    fn slots(&self) -> &[SlotDef] {
        &self.slots
    }

    fn eval(&self, inputs: Vec<Handle<Expr>>) -> Result<Vec<Handle<Expr>>, ExprError> {
        if inputs.len() != 2 {
            return Err(ExprError::GraphEvalError(format!(
                "Unexpected input count to MulNode::eval(): expected 2, got {}",
                inputs.len()
            )));
        }
        let mut inputs = inputs.into_iter();
        let lhs = inputs.next().unwrap();
        let rhs = inputs.next().unwrap();
        Ok(vec![Box::new(MulExpr::new(lhs, rhs))])
    }
}

/// Graph node to divide two values.
#[derive(Debug, Clone)]
pub struct DivNode {
    slots: [SlotDef; 3],
}

impl DivNode {
    /// Create a new node.
    pub fn new() -> Self {
        Self {
            slots: [
                SlotDef::input("lhs", None),
                SlotDef::input("rhs", None),
                SlotDef::output("result", None),
            ],
        }
    }
}

impl Node for DivNode {
    fn slots(&self) -> &[SlotDef] {
        &self.slots
    }

    fn eval(&self, inputs: Vec<Handle<Expr>>) -> Result<Vec<Handle<Expr>>, ExprError> {
        if inputs.len() != 2 {
            return Err(ExprError::GraphEvalError(format!(
                "Unexpected input count to DivNode::eval(): expected 2, got {}",
                inputs.len()
            )));
        }
        let mut inputs = inputs.into_iter();
        let lhs = inputs.next().unwrap();
        let rhs = inputs.next().unwrap();
        Ok(vec![Box::new(DivExpr::new(lhs, rhs))])
    }
}

/// Graph node to get any single particle attribute.
#[derive(Debug, Clone)]
pub struct AttributeNode {
    /// The attribute to get.
    attr: Attribute,
    /// The output slot corresponding to the get value.
    slots: [SlotDef; 1],
}

impl AttributeNode {
    /// Create a new attribute node for the given [`Attribute`].
    pub fn new(attr: Attribute) -> Self {
        Self {
            attr,
            slots: [SlotDef::output(attr.name(), Some(attr.value_type()))],
        }
    }
}

impl AttributeNode {
    /// Get the attribute this node reads.
    pub fn attr(&self) -> Attribute {
        self.attr
    }

    /// Set the attribute this node reads.
    pub fn set_attr(&mut self, attr: Attribute) {
        self.attr = attr;
    }
}

impl Node for AttributeNode {
    fn slots(&self) -> &[SlotDef] {
        &self.slots
    }

    fn eval(&self, inputs: Vec<Handle<Expr>>) -> Result<Vec<Handle<Expr>>, ExprError> {
        if !inputs.is_empty() {
            return Err(ExprError::GraphEvalError(
                "Unexpected non-empty input to AttributeNode::eval().".to_string(),
            ));
        }
        Ok(vec![Box::new(AttributeExpr::new(self.attr))])
    }
}

/// Graph node to get various time values related to the effect system.
#[derive(Debug, Clone)]
pub struct TimeNode {
    /// Output slots corresponding to the various time-related quantities.
    slots: [SlotDef; 2],
}

impl TimeNode {
    /// Create a new time node.
    pub fn new() -> Self {
        Self {
            slots: [BuiltInOperator::Time, BuiltInOperator::DeltaTime]
                .map(|op| SlotDef::output(op.name(), Some(op.value_type()))),
        }
    }
}

impl Node for TimeNode {
    fn slots(&self) -> &[SlotDef] {
        &self.slots
    }

    fn eval(&self, inputs: Vec<Handle<Expr>>) -> Result<Vec<Handle<Expr>>, ExprError> {
        if !inputs.is_empty() {
            return Err(ExprError::GraphEvalError(
                "Unexpected non-empty input to TimeNode::eval().".to_string(),
            ));
        }
        Ok([BuiltInOperator::Time, BuiltInOperator::DeltaTime]
            .map(BuiltInExpr::new)
            .to_vec())
    }
}

/// Graph node to normalize a vector value.
#[derive(Debug, Clone)]
pub struct NormalizeNode {
    /// Input and output vectors.
    slots: [SlotDef; 2],
}

impl NormalizeNode {
    /// Create a new normalize node.
    pub fn new() -> Self {
        Self {
            slots: [SlotDef::output("in", None), SlotDef::output("out", None)],
        }
    }
}

impl Node for NormalizeNode {
    fn slots(&self) -> &[SlotDef] {
        &self.slots
    }

    fn eval(&self, inputs: Vec<Handle<Expr>>) -> Result<Vec<Handle<Expr>>, ExprError> {
        if inputs.len() != 1 {
            return Err(ExprError::GraphEvalError(
                "Unexpected input slot count to NormalizeNode::eval() not equal to one."
                    .to_string(),
            ));
        }
        let input = inputs.into_iter().next().unwrap();
        Ok(vec![Box::new(UnaryNumericOpExpr::new(
            input,
            UnaryNumericOperator::Normalize,
        ))])
    }
}

#[cfg(test)]
mod tests {
    use bevy::prelude::Vec3;

    use crate::{graph::LiteralExpr, ToWgslString};

    use super::*;

    #[test]
    fn add() {
        let node = AddNode::new();

        let ret = node.eval(vec![]);
        assert!(matches!(ret, Err(ExprError::GraphEvalError(_))));
        let ret = node.eval(vec![Box::new(LiteralExpr::new(3))]);
        assert!(matches!(ret, Err(ExprError::GraphEvalError(_))));

        let outputs = node
            .eval(vec![
                Box::new(LiteralExpr::new(3)),
                Box::new(LiteralExpr::new(2)),
            ])
            .unwrap();
        assert_eq!(outputs.len(), 1);
        let out = &outputs[0];
        assert_eq!(out.to_wgsl_string(), "(3) + (2)".to_string());
    }

    #[test]
    fn sub() {
        let node = SubNode::new();

        let ret = node.eval(vec![]);
        assert!(matches!(ret, Err(ExprError::GraphEvalError(_))));
        let ret = node.eval(vec![Box::new(LiteralExpr::new(3))]);
        assert!(matches!(ret, Err(ExprError::GraphEvalError(_))));

        let outputs = node
            .eval(vec![
                Box::new(LiteralExpr::new(3)),
                Box::new(LiteralExpr::new(2)),
            ])
            .unwrap();
        assert_eq!(outputs.len(), 1);
        let out = &outputs[0];
        assert_eq!(out.to_wgsl_string(), "(3) - (2)".to_string());
    }

    #[test]
    fn mul() {
        let node = MulNode::new();

        let ret = node.eval(vec![]);
        assert!(matches!(ret, Err(ExprError::GraphEvalError(_))));
        let ret = node.eval(vec![Box::new(LiteralExpr::new(3))]);
        assert!(matches!(ret, Err(ExprError::GraphEvalError(_))));

        let outputs = node
            .eval(vec![
                Box::new(LiteralExpr::new(3)),
                Box::new(LiteralExpr::new(2)),
            ])
            .unwrap();
        assert_eq!(outputs.len(), 1);
        let out = &outputs[0];
        assert_eq!(out.to_wgsl_string(), "(3) * (2)".to_string());
    }

    #[test]
    fn div() {
        let node = DivNode::new();

        let ret = node.eval(vec![]);
        assert!(matches!(ret, Err(ExprError::GraphEvalError(_))));
        let ret = node.eval(vec![Box::new(LiteralExpr::new(3))]);
        assert!(matches!(ret, Err(ExprError::GraphEvalError(_))));

        let outputs = node
            .eval(vec![
                Box::new(LiteralExpr::new(3)),
                Box::new(LiteralExpr::new(2)),
            ])
            .unwrap();
        assert_eq!(outputs.len(), 1);
        let out = &outputs[0];
        assert_eq!(out.to_wgsl_string(), "(3) / (2)".to_string());
    }

    #[test]
    fn attr() {
        let node = AttributeNode::new(Attribute::POSITION);

        let ret = node.eval(vec![Box::new(LiteralExpr::new(3))]);
        assert!(matches!(ret, Err(ExprError::GraphEvalError(_))));

        let outputs = node.eval(vec![]).unwrap();
        assert_eq!(outputs.len(), 1);
        let out = &outputs[0];
        assert_eq!(
            out.to_wgsl_string(),
            format!("particle.{}", Attribute::POSITION.name())
        );
    }

    #[test]
    fn time() {
        let node = TimeNode::new();

        let ret = node.eval(vec![Box::new(LiteralExpr::new(3))]);
        assert!(matches!(ret, Err(ExprError::GraphEvalError(_))));

        let outputs = node.eval(vec![]).unwrap();
        assert_eq!(outputs.len(), 2);
        assert_eq!(
            outputs[0].to_wgsl_string(),
            BuiltInOperator::Time.to_wgsl_string()
        );
        assert_eq!(
            outputs[1].to_wgsl_string(),
            BuiltInOperator::DeltaTime.to_wgsl_string()
        );
    }

    #[test]
    fn normalize() {
        let node = NormalizeNode::new();

        let ret = node.eval(vec![]);
        assert!(matches!(ret, Err(ExprError::GraphEvalError(_))));

        let outputs = node
            .eval(vec![Box::new(LiteralExpr::new(Vec3::ONE))])
            .unwrap();
        assert_eq!(outputs.len(), 1);
        assert_eq!(
            outputs[0].to_wgsl_string(),
            "normalize(vec3<f32>(1.,1.,1.))".to_string()
        );
    }

    // #[test]
    // fn graph() {
    //     let n1 = AttributeNode::new(Attribute::POSITION);
    //     let n2 = AttributeNode::new(Attribute::POSITION);

    //     let mut g = Graph::new();
    //     let nid1 = g.add_node(Box::new(n1));
    //     let nid2 = g.add_node(Box::new(n2));
    //     let sid1 = g.output_slots(nid1)[0];
    //     let sid2 = g.input_slots(nid2)[0];
    //     g.link(sid1, sid2);
    // }
}
