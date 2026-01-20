//! Slint UI bridge plugin for Bevy.
//!
//! This module provides the `SlintBridgePlugin` that integrates Slint UI with Bevy ECS,
//! handling profile syncing, input events, and UI state management.

use bevy::prelude::*;

use crate::app_state::AppState;
use crate::slint_support::state_bridge::{
    SlintUiChannels, apply_core_to_slint, drain_slint_inbound, sync_installer_to_slint,
    sync_map_name_to_slint, sync_world_labels_to_slint,
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
            .insert_resource(SlintUiChannels::default())
            .add_message::<SlintDoubleClickEvent>()
            .add_message::<ShowSelfProfileEvent>()
            .add_systems(PreUpdate, drain_slint_inbound)
            .add_systems(
                OnEnter(AppState::MainMenu),
                crate::slint_support::state_bridge::show_prelogin_ui
                    .run_if(resource_exists::<crate::slint_support::state_bridge::SlintWindow>),
            )
            .add_systems(
                Update,
                (
                    apply_core_to_slint
                        .run_if(resource_exists::<crate::slint_support::state_bridge::SlintWindow>)
                        .run_if(in_ui_state),
                    crate::slint_support::state_bridge::sync_portrait_to_slint
                        .run_if(resource_exists::<crate::slint_support::state_bridge::SlintWindow>)
                        .run_if(in_state(AppState::InGame)),
                    crate::slint_support::state_bridge::sync_lobby_portraits_to_slint
                        .run_if(resource_exists::<crate::slint_support::state_bridge::SlintWindow>)
                        .run_if(in_state(AppState::MainMenu)),
                    handle_show_self_profile
                        .run_if(resource_exists::<crate::slint_support::state_bridge::SlintWindow>)
                        .run_if(in_state(AppState::InGame)),
                    sync_profile_to_slint
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
            );
    }
}

fn in_ui_state(state: Res<State<AppState>>) -> bool {
    matches!(*state.get(), AppState::MainMenu | AppState::InGame)
}
