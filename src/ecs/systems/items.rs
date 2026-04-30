//! Item interaction systems

use super::super::components::*;
use crate::events::PlayerAction;
use bevy::prelude::*;
use packets::client::Pickup;

/// Handles keyboard-based item pickup from player input.
/// Picks up items at the player's current position or one tile ahead.
pub fn keyboard_item_pickup_system(
    mut player_actions: MessageReader<PlayerAction>,
    player_query: Query<(&Position, &Direction), With<LocalPlayer>>,
    item_query: Query<(&Position, &EntityId), With<ItemSprite>>,
    outbox: Option<Res<crate::network::PacketOutbox>>,
) {
    let Ok((player_pos, player_dir)) = player_query.single() else {
        return;
    };

    for event in player_actions.read() {
        if let PlayerAction::ItemPickupBelow = event {
            let Some(outbox) = &outbox else {
            return;
            };

            // Check the tile the player is currently standing on first
            let player_tile = player_pos.to_vec2();

            // Also check the tile in front of the player
            let delta = player_dir.vec2_delta();
            let front_tile = player_tile + delta;

            // Find items at player's current position first (priority)
            let mut item_underneath = None;
            let mut item_in_front = None;

            for (item_pos, _) in item_query.iter() {
            let item_tile = item_pos.to_vec2();

            // Check if item is at player's position
            if item_tile.distance_squared(player_tile) < 0.25 {
                item_underneath = Some((item_pos.x as u16, item_pos.y as u16));
                break; // Found item underneath, prioritize this
            }
            // Check if item is in front of player
            else if item_in_front.is_none() && item_tile.distance_squared(front_tile) < 0.25 {
                item_in_front = Some((item_pos.x as u16, item_pos.y as u16));
            }
            }

            // Pick up item underneath first, then front
            if let Some((x, y)) = item_underneath.or(item_in_front) {
            outbox.send(&Pickup {
                destination_slot: 0,
                source_point: (x, y),
            });
            tracing::info!("Picking up item at ({}, {})", x, y);
            } else {
            tracing::debug!(
                "No item found at player position ({}, {}) or in front ({}, {})",
                player_tile.x,
                player_tile.y,
                front_tile.x,
                front_tile.y
            );
            }
        }
    }
}
