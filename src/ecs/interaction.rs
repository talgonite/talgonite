//! Interaction types for entity hover/click handling

use bevy::prelude::*;

// Re-export events from the central events module
pub use crate::events::{EntityClickEvent, EntityHoverEvent};

/// Resource tracking which entity is currently hovered by the mouse
#[derive(Resource, Default)]
pub struct HoveredEntity(pub Option<Entity>);

