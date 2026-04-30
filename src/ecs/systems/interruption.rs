use crate::ecs::components::{LocalPlayer, PathfindingState};
use crate::ecs::spell_casting::SpellCastingState;
use crate::events::{InteractionIntentEvent, PlayerAction, TileClickEvent};
use crate::input::{GameAction, GamepadConfig, UnifiedInputBindings};
use bevy::prelude::*;

/// Centralizes player action interruptions to ensure mutual exclusion
/// between manual movement, pathfinding, and spell casting.
pub fn player_interruption_system(
    mut actions: MessageReader<PlayerAction>,
    mut tile_clicks: MessageReader<TileClickEvent>,
    mut interaction_intents: MessageReader<InteractionIntentEvent>,
    mut spells: ResMut<SpellCastingState>,
    mut commands: Commands,
    player: Query<Entity, With<LocalPlayer>>,
    input: Res<ButtonInput<KeyCode>>,
    bindings: Res<UnifiedInputBindings>,
    gamepads: Query<&Gamepad>,
    config: Res<GamepadConfig>,
) {
    let Ok(player_entity) = player.single() else {
        return;
    };

    let is_manual_move = bindings.any_pressed(
        &[
            GameAction::MoveUp,
            GameAction::MoveDown,
            GameAction::MoveLeft,
            GameAction::MoveRight,
        ],
        &input,
        Some(&gamepads),
        Some(&config),
    ) || actions.read().any(|a| a.is_manual());

    let is_new_path = tile_clicks.read().any(|e| e.button == MouseButton::Right);
    let has_interaction_intent = interaction_intents.read().next().is_some();

    if is_manual_move {
        spells.active_cast = None;
        commands.entity(player_entity).remove::<PathfindingState>();
    }

    if is_new_path {
        spells.active_cast = None;
    }

    if has_interaction_intent {
        spells.active_cast = None;
        commands.entity(player_entity).remove::<PathfindingState>();
    }
}
