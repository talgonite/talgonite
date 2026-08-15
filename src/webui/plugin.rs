use bevy::input::ButtonInput;
use bevy::input::mouse::MouseButton;
use bevy::prelude::*;
use game_ui::{CoreToUi, KeyboardEdges, UiToCore};
use std::time::Duration;

use crate::app_state::AppState;
use crate::render_plugin::game::WebUi;
use crate::settings_types::Settings as SettingsFile;
use crate::webui::bridge::{
    bridge_ability_events, bridge_chat_events, bridge_inventory_events, bridge_session_events,
    update_skill_cooldowns, update_world_list_filtered,
};
use crate::webui::inbound::{
    handle_ui_inbound_group_exchange, handle_ui_inbound_hotbar, handle_ui_inbound_menus,
    handle_ui_inbound_settings, handle_ui_inbound_slots, handle_ui_inbound_world,
};
use crate::webui::input::{clear_input_edges, clear_just_input, handle_input_bridge};
use crate::webui::login::{
    PreLoginConnectionState, handle_character_creation_results, handle_character_creation_tasks,
    handle_login_results, handle_login_tasks, handle_prelogin_connect_tasks,
    handle_ui_inbound_login,
};
use crate::webui::settings::sync_settings_to_ui;

pub use super::popups::handle_popup_requests;
pub use super::state::*;
pub use game_ui::CursorPosition;

#[derive(Message)]
pub struct UiInbound(pub UiToCore);

#[derive(Message)]
pub struct UiOutbound(pub CoreToUi);

pub struct UiBridgePlugin;

impl Plugin for UiBridgePlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<UiInbound>()
            .add_message::<UiOutbound>()
            .init_resource::<InventoryState>()
            .init_resource::<AbilityState>()
            .init_resource::<SkillCooldowns>()
            .init_resource::<WorldListState>()
            .init_resource::<EquipmentState>()
            .init_resource::<PlayerProfileState>()
            .init_resource::<GroupState>()
            .init_resource::<BoardSessionState>()
            .init_resource::<ExchangeSessionState>()
            .init_resource::<crate::ecs::hotbar::HotbarState>()
            .init_resource::<crate::ecs::hotbar::HotbarPanelState>()
            .init_resource::<ActiveMenuContext>()
            .init_resource::<ActiveWorldContextMenu>()
            .init_resource::<PreLoginConnectionState>()
            .init_resource::<CursorPosition>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<KeyboardEdges>()
            .add_systems(PreUpdate, handle_input_bridge)
            .add_systems(
                Update,
                (
                    bridge_inventory_events,
                    bridge_ability_events,
                    bridge_chat_events,
                    bridge_session_events,
                    update_world_list_filtered,
                    forward_outbound_to_webview,
                    handle_ui_inbound_login.run_if(not(in_state(AppState::InGame))),
                    (
                        handle_ui_inbound_settings,
                        handle_ui_inbound_world,
                        handle_ui_inbound_menus,
                        handle_ui_inbound_slots,
                        handle_ui_inbound_group_exchange,
                        handle_ui_inbound_hotbar,
                    )
                        .run_if(in_state(AppState::InGame)),
                    handle_popup_requests,
                    handle_prelogin_connect_tasks,
                    handle_login_tasks,
                    handle_login_results,
                    handle_character_creation_tasks,
                    handle_character_creation_results,
                    update_skill_cooldowns,
                    sync_settings_to_ui,
                    check_board_delete_timeout,
                    check_board_compose_timeout,
                ),
            )
            .add_systems(Last, (clear_input_edges, clear_just_input))
            .add_systems(Update, emit_snapshot_on_state_change);
    }
}

fn check_board_compose_timeout(
    time: Res<Time>,
    mut board_state: ResMut<BoardSessionState>,
    mut outbound: MessageWriter<UiOutbound>,
) {
    let Some(submitted_at) = board_state.compose_submitted_at else {
        return;
    };
    if time.elapsed().saturating_sub(submitted_at) < Duration::from_secs(5) {
        return;
    }
    board_state.compose_waiting = false;
    board_state.compose_submitted_at = None;
    board_state.compose_result =
        "The server did not respond. The message may not have been sent.".to_string();
    outbound.write(UiOutbound(board_state.to_compose_msg()));
}

fn check_board_delete_timeout(
    time: Res<Time>,
    mut board_state: ResMut<BoardSessionState>,
    mut outbound: MessageWriter<UiOutbound>,
) {
    let Some(requested_at) = board_state.delete_requested_at else {
        return;
    };
    if time.elapsed().saturating_sub(requested_at) < Duration::from_secs(5) {
        return;
    }
    board_state.delete_requested_at = None;
    outbound.write(UiOutbound(board_state.to_delete_msg(
        "The server did not respond. The post may still exist.",
    )));
}

fn forward_outbound_to_webview(
    mut reader: MessageReader<UiOutbound>,
    _web_ui: Option<NonSendMut<WebUi>>,
    _settings: Res<SettingsFile>,
) {
    // Slint mode: this becomes a no-op; Slint bridge reads UiOutbound directly.
    for _ in reader.read() {}
}

fn emit_snapshot_on_state_change(
    app_state: Res<State<AppState>>,
    mut prev: Local<Option<AppState>>,
    mut writer: MessageWriter<UiOutbound>,
    settings: Res<SettingsFile>,
) {
    let current = *app_state.get();
    if prev.map(|p| p != current).unwrap_or(true) {
        writer.write(UiOutbound(settings.to_snapshot_message(None)));
        writer.write(UiOutbound(settings.to_sync_message()));
        *prev = Some(current);
    }
}
