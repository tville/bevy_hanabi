//! Modifiers influencing a secondary translation of particles, with the suggested
//! use case of floating origin recentering.
//!
//! The secondary translation offset is set via the translation_offset expression,
//! which can be updated for example via a property. The offset is reflected in new
//! particles, and preivously spawned particles will be updated to stay consistent
//! with the re-centered world.
//!
//! Since this modifies all existing particles, it has a performance penalty compared
//! to more specific solutions such as using different emitters in different cells
//! in the floating origin grid system, but is arguably simpler to interface with
//! when performance is not cruical.
use std::hash::Hash;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    expr::PropertyHandle, graph::ExprError, Attribute, BoxedModifier, EvalContext, ExprHandle, Modifier, ModifierContext, Module, ShaderWriter
};

/// A modifier to apply a secondary translation to all particles, commonly used
/// when using a floating origin to re-center the world in order to keep high
/// floating point precision near the camera.
///
/// The secondary translation, or offset, is applied both during particle init,
/// and updated on already existing particles whenever the provided expression
/// value changes.
/// 
/// A typical example would be to add this modifier as an update modifier to 
/// the relevant effect asset, and tie it via the translation_offset handle
/// to a property that is updated when needed.
/// 
/// # Attributes
///
/// This modifier requires the following particle attributes:
/// - [`Attribute::POSITION`]
/// - [`Attribute::F32X3_0`]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect, Serialize, Deserialize)]
pub struct FloatingOriginModifier {
    /// The translation offset to apply to all particles.
    ///
    /// Expression type: `Vec3`
    translation_offset: ExprHandle,
}

impl FloatingOriginModifier {
    /// Create a new modifier from a translation offset expression.
    pub fn new(translation_offset: ExprHandle) -> Self {
        Self { translation_offset }
    }

    /// Create a new modifier with an offset derived from a property.
    ///
    /// To create a new property, use [`Module::add_property()`].
    pub fn via_property(module: &mut Module, property: PropertyHandle) -> Self {
        Self {
            translation_offset: module.prop(property),
        }
    }

    /// Create a new modifier with a constant offset.
    pub fn constant(module: &mut Module, offset: Vec3) -> Self {
        Self {
            translation_offset: module.lit(offset),
        }
    }
}

#[cfg_attr(feature = "serde", typetag::serde)]
impl Modifier for FloatingOriginModifier {
    fn context(&self) -> ModifierContext {
        ModifierContext::Update
    }

    fn attributes(&self) -> &[Attribute] {
        &[Attribute::POSITION, Attribute::F32X3_0]
    }

    fn boxed_clone(&self) -> BoxedModifier {
        Box::new(*self)
    }

    fn apply(&self, module: &mut Module, context: &mut ShaderWriter) -> Result<(), ExprError> {
        let attr_pos_offset = module.attr(Attribute::F32X3_0);
        let attr_pos_offset = context.eval(module, attr_pos_offset)?;
        let expr = context.eval(module, self.translation_offset)?;

        context.main_code += &format!(
            r##"
    if (any(vec3<bool>({1}.x != {2}.x, 
            {1}.y != {2}.y, 
            {1}.z != {2}.z))) {{
        // Adjust for changed offset, e.g. floating origin recentering.
        particle.{0} += {2} - {1};
        // Then store the new offset
        {1} = {2};
    }}
            "##,
            Attribute::POSITION.name(),
            attr_pos_offset,
            expr,
        );
        Ok(())
    }
}