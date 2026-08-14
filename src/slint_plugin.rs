//! Slint UI bridge plugin for Bevy.
//!
//! This module provides the `SlintBridgePlugin` that integrates Slint UI with Bevy ECS,
//! handling profile syncing, input events, and UI state management.

use bevy::prelude::*;

use crate::app_state::AppState;
use crate::resources::{DebugLog, FrameMetrics};
#[cfg(feature = "debug")]
use crate::slint_support::debug_console::{
    ConsoleMetricsModel, DebugConsoleWindow, update_debug_console,
};
use crate::slint_support::popups::PopupManager;
use crate::slint_support::state_bridge::{
    LastWorldLabelState, SlintUiChannels, apply_core_to_slint, drain_slint_inbound,
    reset_label_sync_cache, sync_group_to_slint, sync_installer_to_slint, sync_map_name_to_slint,
    sync_popup_to_slint, sync_settings_to_slint, sync_skill_cooldowns_to_slint,
    sync_spell_casting_to_slint, sync_world_labels_to_slint,
};
use crate::slint_support::{handle_show_self_profile, sync_profile_to_slint};

// Re-export attach_slint_ui for convenience
pub use crate::slint_support::attach_slint_ui;

// Re-export types for backward compatibility
pub use crate::slint_support::ShowSelfProfileEvent;
pub use crate::slint_support::SlintDoubleClickEvent;
pub use crate::slint_support::SlintGpuReady;

pub struct SlintBridgePlugin;

impl Plugin for SlintBridgePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SlintGpuReady>()
            .init_resource::<PopupManager>()
            .init_resource::<FrameMetrics>()
            .init_resource::<DebugLog>()
            .init_resource::<LastWorldLabelState>()
            .insert_resource(SlintUiChannels::default())
            .add_message::<SlintDoubleClickEvent>()
            .add_message::<ShowSelfProfileEvent>()
            .add_systems(PreUpdate, drain_slint_inbound)
            .add_systems(
                OnEnter(AppState::MainMenu),
                crate::slint_support::state_bridge::show_prelogin_ui
                    .run_if(resource_exists::<crate::slint_support::state_bridge::SlintWindow>),
            )
            .add_systems(OnEnter(AppState::InGame), reset_label_sync_cache)
            .add_systems(
                Update,
                (
                    apply_core_to_slint
                        .run_if(resource_exists::<crate::slint_support::state_bridge::SlintWindow>)
                        .run_if(in_ui_state),
                    sync_popup_to_slint
                        .after(crate::webui::plugin::handle_popup_requests)
                        .run_if(resource_exists::<crate::slint_support::state_bridge::SlintWindow>),
                    sync_settings_to_slint
                        .run_if(resource_exists::<crate::slint_support::state_bridge::SlintWindow>),
                    crate::slint_support::state_bridge::sync_portrait_to_slint
                        .run_if(resource_exists::<crate::slint_support::state_bridge::SlintWindow>)
                        .run_if(in_state(AppState::InGame)),
                    crate::slint_support::state_bridge::sync_lobby_portraits_to_slint
                        .run_if(resource_exists::<crate::slint_support::state_bridge::SlintWindow>)
                        .run_if(in_state(AppState::MainMenu)),
                    crate::slint_support::state_bridge::sync_character_creator_preview_to_slint
                        .run_if(resource_exists::<crate::slint_support::state_bridge::SlintWindow>)
                        .run_if(in_state(AppState::MainMenu)),
                    handle_show_self_profile
                        .run_if(resource_exists::<crate::slint_support::state_bridge::SlintWindow>)
                        .run_if(in_state(AppState::InGame)),
                    sync_profile_to_slint
                        .run_if(resource_exists::<crate::slint_support::state_bridge::SlintWindow>)
                        .run_if(in_state(AppState::InGame)),
                    sync_group_to_slint
                        .run_if(resource_exists::<crate::slint_support::state_bridge::SlintWindow>)
                        .run_if(in_state(AppState::InGame)),
                    sync_skill_cooldowns_to_slint
                        .run_if(resource_exists::<crate::slint_support::state_bridge::SlintWindow>)
                        .run_if(in_state(AppState::InGame)),
                    sync_spell_casting_to_slint
                        .run_if(resource_exists::<crate::slint_support::state_bridge::SlintWindow>)
                        .run_if(in_state(AppState::InGame)),
                ),
            )
            .add_systems(
                PostUpdate,
                (
                    sync_world_labels_to_slint
                        .run_if(resource_exists::<crate::slint_support::state_bridge::SlintWindow>)
                        .run_if(in_state(AppState::InGame)),
                    sync_map_name_to_slint
                        .run_if(resource_exists::<crate::slint_support::state_bridge::SlintWindow>)
                        .run_if(in_state(AppState::InGame)),
                    sync_installer_to_slint
                        .run_if(resource_exists::<crate::slint_support::state_bridge::SlintWindow>)
                        .run_if(in_state(AppState::Installing)),
                ),
            )
            .add_systems(
                OnEnter(AppState::Installing),
                crate::slint_support::state_bridge::show_installer_ui,
            )
            .add_systems(
                OnExit(AppState::Installing),
                crate::slint_support::state_bridge::hide_installer_ui,
            );

    #[cfg(feature = "debug")]
    app.insert_non_send(ConsoleMetricsModel(None));

    #[cfg(feature = "debug")]
    app.add_systems(
        Last,
        update_debug_console
            .run_if(resource_exists::<DebugConsoleWindow>)
            .after(crate::sys_timing::collect_system_timings),
    );
    }
}

fn in_ui_state(
    state: Res<State<AppState>>,
    game_files: Option<Res<crate::game_files::GameFiles>>,
) -> bool {
    let base_state = matches!(*state.get(), AppState::MainMenu | AppState::InGame);
    base_state && game_files.is_some()
}
