//! Game ECS Plugin
//!
//! This plugin registers all game systems with proper ordering using [`GameSet`].
//! Systems are organized into logical phases that execute in a deterministic order.

use bevy::prelude::*;
use tracing::{info, warn};

use super::animation;
use super::collision::WallCollisionTable;
use super::spell_casting::{self, SpellCastingState};
use super::systems::{self, GameSet};
use crate::audio;

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        // Configure system set ordering
        systems::configure_game_sets(app);

        app.init_resource::<SpellCastingState>()
            .init_resource::<crate::resources::LobbyPortraits>()
            .init_resource::<crate::resources::ItemTileCounters>()
            .init_resource::<super::components::MapDoorQueue>()
            .add_message::<super::components::MapPrepared>()
            .add_systems(
                OnEnter(crate::app_state::AppState::InGame),
                audio::setup_audio_settings,
            )
            .add_systems(
                Update,
                systems::sync_lobby_portraits
                    .run_if(in_state(crate::app_state::AppState::MainMenu))
                    .run_if(resource_exists::<crate::PlayerAssetStoreState>)
                    .run_if(resource_exists::<crate::RendererState>)
                    .run_if(resource_exists::<crate::game_files::GameFiles>),
            )
            // === Event Processing Systems ===
            // These don't need renderer resources
            .add_systems(
                Update,
                (
                    systems::initialize_game_world,
                    load_collision_table.run_if(not(resource_exists::<WallCollisionTable>)),
                    audio::play_sound,
                    audio::sync_audio_settings,
                    spell_casting::start_spell_cast,
                    spell_casting::update_spell_casting,
                    spell_casting::handle_spell_targeting
                        .after(crate::plugins::mouse_interaction::MouseInteractionSet),
                    spell_casting::update_targeting_hover,
                    systems::pathfinding_target_system
                        .after(crate::plugins::mouse_interaction::MouseInteractionSet),
                    systems::player_interruption_system,
                    crate::ecs::hotbar::sync_hotbar_panel_to_settings,
                    systems::handle_public_messages,
                    systems::expire_speech_bubbles,
                    systems::expire_chant_labels,
                    systems::expire_health_bars,
                )
                    .run_if(in_state(crate::app_state::AppState::InGame))
                    .in_set(GameSet::EventProcessing),
            )
            // === Spawning Systems ===
            .add_systems(
                Update,
                (
                    systems::map_system,
                    systems::handle_doors,
                    systems::spawn_entities_system,
                    systems::dedupe_entities_by_id,
                    systems::health_bar_system,
                    systems::queue_creatures_for_loading
                        .run_if(resource_exists::<crate::CreatureAssetStoreState>),
                )
                    .chain()
                    .run_if(in_state(crate::app_state::AppState::InGame))
                    .in_set(GameSet::Spawning),
            )
            // === Movement Systems ===
            .add_systems(
                Update,
                (
                    systems::player_reconciliation_system,
                    systems::pathfinding_execution_system.before(systems::player_movement_system),
                    systems::player_movement_system,
                    systems::entity_motion_system,
                    systems::player_animation_start_system,
                    systems::entity_effect_system,
                )
                    .run_if(in_state(crate::app_state::AppState::InGame))
                    .in_set(GameSet::Movement),
            )
           // === Item systems ===
           .add_systems(
               Update,
               systems::keyboard_item_pickup_system
                   .run_if(in_state(crate::app_state::AppState::InGame))
                   .after(crate::plugins::input::InputPumpSet)
                   .in_set(GameSet::EventProcessing),
           )
            // === Physics Systems ===
            .add_systems(
                Update,
                systems::movement_tween_system
                    .run_if(in_state(crate::app_state::AppState::InGame))
                    .in_set(GameSet::Physics),
            )
            // === Animation Systems ===
            .add_systems(
                Update,
                (animation::animation_system, systems::map_animation_system)
                    .run_if(in_state(crate::app_state::AppState::InGame))
                    .in_set(GameSet::Animation),
            )
            // === Camera Systems ===
            .add_systems(
                Update,
                (
                    systems::camera_follow_system,
                    systems::camera_position_sync,
                    systems::camera_xray_sync,
                )
                    .chain()
                    .run_if(in_state(crate::app_state::AppState::InGame))
                    .in_set(GameSet::Camera),
            )
            // === Render Sync Systems ===
            // These require GPU renderer resources
            .add_systems(
                Update,
                (
                    systems::creature_load_system,
                    systems::sync_items_to_renderer,
                    systems::update_items_to_renderer,
                    systems::sync_players_to_renderer,
                    systems::update_player_sprites,
                    systems::creature_movement_sync,
                    systems::sync_player_portrait,
                    systems::sync_profile_portrait,
                )
                    .run_if(resource_exists::<crate::CreatureAssetStoreState>)
                    .run_if(resource_exists::<crate::PlayerBatchState>)
                    .run_if(in_state(crate::app_state::AppState::InGame))
                    .in_set(GameSet::RenderSync),
            )
            // === Effect Systems ===
            // These run after other render sync, require EffectManagerState
            .add_systems(
                Update,
                (
                    systems::spawn_effects_system,
                    systems::effect_follow_entity_system,
                    systems::update_effects_system,
                )
                    .chain()
                    .run_if(resource_exists::<crate::EffectManagerState>)
                    .run_if(in_state(crate::app_state::AppState::InGame))
                    .in_set(GameSet::RenderSync),
            );
    }
}

/// Loads the wall collision table from game files.
fn load_collision_table(mut commands: Commands, game_files: Res<crate::game_files::GameFiles>) {
    match game_files.get_file("ia/sotp.dat") {
        Some(bytes) => {
            info!(
                "Loaded wall collision table (ia/sotp.dat), {} bytes",
                bytes.len()
            );
            commands.insert_resource(WallCollisionTable::from_sotp_bytes(bytes));
        }
        None => {
            warn!("ia/sotp.dat not found, wall collision disabled");
            commands.insert_resource(WallCollisionTable::from_sotp_bytes(vec![]));
        }
    }
}
