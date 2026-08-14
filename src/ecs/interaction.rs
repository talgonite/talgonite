//! Interaction types for entity hover/click handling

use bevy::prelude::*;

// Re-export events from the central events module
pub use crate::events::{EntityClickEvent, EntityHoverEvent};

/// Resource tracking which entity is currently hovered by the mouse
#[derive(Resource, Default)]
pub struct HoveredEntity(pub Option<Entity>);

/// Resource tracking the active UI drag operation (if any)
#[derive(Resource, Default, Debug, Clone)]
pub struct ActiveDragState {
    pub is_dragging: bool,
    pub source_panel: game_types::SlotPanelType,
    pub source_index: i32,
}
