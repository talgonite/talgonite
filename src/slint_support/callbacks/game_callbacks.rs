//! Game-related callback wiring for Slint UI.

use crossbeam_channel::Sender;
use slint::ComponentHandle;
use slint::Model;

use crate::webui::ipc::{UiToCore, WorldListFilter};
use crate::{
    ContextMenuState, DragDropState, GameState, MailBoardState, MainWindow, NpcDialogState,
    PopupManagerState, SlotPanelType, SocialStatus, SocialStatusState,
};

/// Convert Slint SlotPanelType to game types.
fn slint_to_game_panel(panel: SlotPanelType) -> game_types::SlotPanelType {
    match panel {
        SlotPanelType::Item => game_types::SlotPanelType::Item,
        SlotPanelType::Skill => game_types::SlotPanelType::Skill,
        SlotPanelType::Spell => game_types::SlotPanelType::Spell,
        SlotPanelType::Hotbar => game_types::SlotPanelType::Hotbar,
        SlotPanelType::World => game_types::SlotPanelType::World,
        SlotPanelType::None => game_types::SlotPanelType::None,
        SlotPanelType::Exchange => game_types::SlotPanelType::Exchange,
    }
}

/// Payload attached to an in-app drag; read by the drop target's handler.
struct DragSource {
    panel: SlotPanelType,
    index: i32,
}

/// Wire all game-related callbacks: world map, menu, chat, equipment, hotbar, drag-drop.
pub fn wire_game_callbacks(slint_app: &MainWindow, tx: Sender<UiToCore>) {
    let game_state = slint_app.global::<GameState>();

    // World map click
    {
        let tx = tx.clone();
        game_state.on_world_map_click(move |map_id, x, y, check_sum| {
            let _ = tx.send(UiToCore::WorldMapClick {
                map_id: map_id as u16,
                x: x as u16,
                y: y as u16,
                check_sum: check_sum as u16,
            });
        });
    }

    // NPC Dialog callbacks
    let npc_dialog = slint_app.global::<NpcDialogState>();
    let context_menu = slint_app.global::<ContextMenuState>();

    // Menu select (option selection)
    {
        let tx = tx.clone();
        npc_dialog.on_select_option_request(move |id, name: slint::SharedString| {
            let _ = tx.send(UiToCore::MenuSelect {
                id,
                name: name.to_string(),
            });
        });
    }

    // Close dialog: the close is routed through `PopupManagerState` and its
    // server coordination happens in `handle_popup_requests`.

    // Text entry submission
    {
        let tx = tx.clone();
        npc_dialog.on_submit_text_request(move |text: slint::SharedString| {
            let _ = tx.send(UiToCore::MenuSelect {
                id: 0,
                name: text.to_string(),
            });
        });
    }

    // Unequip
    {
        let tx = tx.clone();
        game_state.on_unequip(move |slot| {
            if tx.send(UiToCore::Unequip { slot: slot as u8 }).is_err() {
                tracing::error!("Failed to send Unequip message");
            }
        });
    }

    {
        let tx = tx.clone();
        context_menu.on_item_selected_request(move |id| {
            let _ = tx.send(UiToCore::WorldContextMenuSelect { id });
        });
    }

    // Use action
    {
        let tx = tx.clone();
        game_state.on_use_action(move |panel, slot| {
            if tx
                .send(UiToCore::ActivateAction {
                    category: slint_to_game_panel(panel),
                    index: slot as usize,
                })
                .is_err()
            {
                tracing::error!("Failed to send ActivateAction message");
            }
        });
    }

    // Set hotbar panel
    {
        let tx = tx.clone();
        game_state.on_set_hotbar_panel(move |panel_num| {
            if tx
                .send(UiToCore::SetHotbarPanel {
                    panel_num: panel_num as u8,
                })
                .is_err()
            {
                tracing::error!("Failed to send SetHotbarPanel message");
            }
        });
    }

    // Expand hotbar rows
    {
        let tx = tx.clone();
        game_state.on_expand_hotbar(move || {
            if tx.send(UiToCore::ExpandHotbarRows).is_err() {
                tracing::error!("Failed to send ExpandHotbarRows message");
            }
        });
    }

    // Collapse hotbar rows
    {
        let tx = tx.clone();
        game_state.on_collapse_hotbar(move || {
            if tx.send(UiToCore::CollapseHotbarRows).is_err() {
                tracing::error!("Failed to send CollapseHotbarRows message");
            }
        });
    }

    // Refresh world list
    {
        let tx = tx.clone();
        game_state.on_refresh_world_list(move || {
            let _ = tx.send(UiToCore::RequestWorldList);
        });
    }

    // Set world list filter
    {
        let tx = tx.clone();
        game_state.on_set_world_list_filter(move |class, master_only, search| {
            let _ = tx.send(UiToCore::SetWorldListFilter {
                filter: WorldListFilter {
                    class: if class == "All" {
                        None
                    } else {
                        Some(class.to_string())
                    },
                    master_only,
                    search: search.to_string(),
                },
            });
        });
    }

    // Send chat
    {
        let tx = tx.clone();
        game_state.on_send_chat(move |text| {
            if tx
                .send(UiToCore::ChatSubmit {
                    mode: "all".to_string(),
                    text: text.to_string(),
                    target: None,
                })
                .is_err()
            {
                tracing::error!("Failed to send ChatSubmit message");
            }
        });
    }

    // Send whisper
    {
        let tx = tx.clone();
        let slint_app_weak = slint_app.as_weak();
        game_state.on_send_whisper(move |target, text| {
            if let Some(app) = slint_app_weak.upgrade() {
                let gs = app.global::<GameState>();
                gs.set_last_whisper_target(target.clone());
            }

            if tx
                .send(UiToCore::ChatSubmit {
                    mode: "whisper".to_string(),
                    text: text.to_string(),
                    target: Some(target.to_string()),
                })
                .is_err()
            {
                tracing::error!("Failed to send ChatSubmit (whisper) message");
            }
        });
    }

    // Toggle groupable
    {
        let tx = tx.clone();
        game_state.on_toggle_groupable(move || {
            let _ = tx.send(UiToCore::ToggleGroupable);
        });
    }

    let mail_board = slint_app.global::<MailBoardState>();
    {
        let tx = tx.clone();
        mail_board.on_post_open_request(move |index, post_id| {
            let _ = tx.send(UiToCore::MailBoardOpenPost { index, post_id });
        });
    }
    {
        let tx = tx.clone();
        mail_board.on_delete_request(move |index, post_id| {
            let _ = tx.send(UiToCore::MailBoardDeletePost { index, post_id });
        });
    }
    {
        let tx = tx.clone();
        mail_board.on_delete_dismiss(move || {
            let _ = tx.send(UiToCore::MailBoardDeleteDismiss);
        });
    }
    {
        let tx = tx.clone();
        mail_board.on_board_selected(move |index| {
            let _ = tx.send(UiToCore::MailBoardSelectBoard { index });
        });
    }
    {
        let tx = tx.clone();
        mail_board.on_new_post_request(move || {
            let _ = tx.send(UiToCore::MailBoardComposeNew);
        });
    }
    {
        let tx = tx.clone();
        let slint_app_weak = slint_app.as_weak();
        mail_board.on_reply_request(move || {
            if let Some(app) = slint_app_weak.upgrade() {
                let mail_board = app.global::<MailBoardState>();
                let index = mail_board.get_selected_index();
                if index >= 0 {
                    if let Some(post) = mail_board.get_posts().row_data(index as usize) {
                        let _ = tx.send(UiToCore::MailBoardComposeReply {
                            index,
                            post_id: post.post_id,
                        });
                    }
                }
            }
        });
    }
    {
        let tx = tx.clone();
        let slint_app_weak = slint_app.as_weak();
        mail_board.on_compose_send(move || {
            if let Some(app) = slint_app_weak.upgrade() {
                let mail_board = app.global::<MailBoardState>();
                let name = mail_board.get_compose_name().to_string();
                let subject = mail_board.get_compose_subject().to_string();
                let body = mail_board.get_compose_body().to_string();
                let _ = tx.send(UiToCore::MailBoardComposeSend {
                    name,
                    subject,
                    body,
                });
            }
        });
    }
    {
        let tx = tx.clone();
        mail_board.on_compose_cancel(move || {
            let _ = tx.send(UiToCore::MailBoardComposeCancel);
        });
    }

    // === Group callbacks ===
    {
        let tx = tx.clone();
        game_state.on_send_group_invite(move |name: slint::SharedString| {
            let _ = tx.send(UiToCore::SendGroupInvite {
                name: name.to_string(),
            });
        });
    }
    {
        let tx = tx.clone();
        game_state.on_respond_group_invite(move |accept, source_name: slint::SharedString| {
            let _ = tx.send(UiToCore::RespondGroupInvite {
                accept,
                source_name: source_name.to_string(),
            });
        });
    }
    {
        let tx = tx.clone();
        game_state.on_kick_group_member(move |name: slint::SharedString| {
            let _ = tx.send(UiToCore::KickGroupMember {
                name: name.to_string(),
            });
        });
    }
    {
        let tx = tx.clone();
        game_state.on_leave_group(move || {
            let _ = tx.send(UiToCore::LeaveGroup);
        });
    }
    {
        let tx = tx.clone();
        game_state.on_request_self_profile(move || {
            let _ = tx.send(UiToCore::RequestSelfProfile);
        });
    }
    {
        let tx = tx.clone();
        game_state.on_cancel_spell_targeting(move || {
            if tx.send(UiToCore::CancelSpellTargeting).is_err() {
                tracing::error!("Failed to send CancelSpellTargeting message");
            }
        });
    }

    // Drag-drop: the UI builds an opaque payload per drag source; the drop target
    // reports it back here together with the target slot and position.
    let dragdrop_state = slint_app.global::<DragDropState>();
    {
        dragdrop_state.on_make_transfer(move |panel, index| {
            let mut data = slint::DataTransfer::default();
            data.set_user_data(std::rc::Rc::new(DragSource { panel, index }));
            data
        });
    }
    {
        let tx = tx.clone();
        dragdrop_state.on_dropped(move |data, dst_panel, dst_slot, x, y| {
            let Some(src) = data
                .user_data()
                .and_then(|rc| rc.downcast::<DragSource>().ok())
            else {
                tracing::warn!("Drag drop payload is missing the drag source");
                return;
            };

            tracing::info!(
                "DragDropAction from {:?} slot {} to {:?} slot {} at ({}, {})",
                src.panel,
                src.index,
                dst_panel,
                dst_slot,
                x,
                y
            );

            if tx
                .send(UiToCore::DragDropAction {
                    src_category: slint_to_game_panel(src.panel),
                    src_index: src.index as usize,
                    dst_category: slint_to_game_panel(dst_panel),
                    dst_index: dst_slot as usize,
                    x,
                    y,
                })
                .is_err()
            {
                tracing::error!("Failed to send DragDropAction message");
            }
        });
    }
    {
        let tx = tx.clone();
        dragdrop_state.on_drag_state_changed(move |is_dragging, panel, index| {
            let _ = tx.send(UiToCore::DragStateChanged {
                is_dragging,
                panel: slint_to_game_panel(panel),
                index,
            });
        });
    }

    // Social status callbacks
    let social_status_state = slint_app.global::<SocialStatusState>();
    {
        let tx = tx.clone();
        social_status_state.on_status_changed(move |status| {
            if tx
                .send(UiToCore::SetSocialStatus {
                    status: social_status_to_u8(status),
                })
                .is_err()
            {
                tracing::error!("Failed to send SetSocialStatus message");
            }
        });
    }

    // Popup manager requests (open/close/close-top)
    let popup_state = slint_app.global::<PopupManagerState>();
    {
        let tx = tx.clone();
        popup_state.on_open_request(move |id| {
            if tx.send(UiToCore::PopupOpenRequest { id }).is_err() {
                tracing::error!("Failed to send PopupOpenRequest message");
            }
        });
    }
    {
        let tx = tx.clone();
        popup_state.on_close_request(move |id| {
            if tx.send(UiToCore::PopupCloseRequest { id }).is_err() {
                tracing::error!("Failed to send PopupCloseRequest message");
            }
        });
    }
    {
        let tx = tx.clone();
        popup_state.on_close_top_request(move || {
            if tx.send(UiToCore::PopupCloseTop).is_err() {
                tracing::error!("Failed to send PopupCloseTop message");
            }
        });
    }

    // Exchange callbacks
    {
        let tx = tx.clone();
        game_state.on_exchange_add_item(move |slot| {
            let _ = tx.send(UiToCore::ExchangeAddItem { slot: slot as u8 });
        });
    }
    {
        let tx = tx.clone();
        game_state.on_exchange_add_stackable_item(move |slot, count| {
            let _ = tx.send(UiToCore::ExchangeAddStackableItem {
                slot: slot as u8,
                count: count.max(0).min(255) as u8,
            });
        });
    }
    {
        let tx = tx.clone();
        game_state.on_exchange_set_gold(move |amount| {
            let _ = tx.send(UiToCore::ExchangeSetGold {
                amount: amount.max(0) as u32,
            });
        });
    }
    {
        let tx = tx.clone();
        game_state.on_exchange_cancel_quantity(move || {
            let _ = tx.send(UiToCore::ExchangeCancelQuantity);
        });
    }
    {
        let tx = tx.clone();
        game_state.on_exchange_accept(move || {
            let _ = tx.send(UiToCore::ExchangeAccept);
        });
    }
    {
        let tx = tx.clone();
        game_state.on_exchange_cancel(move || {
            let _ = tx.send(UiToCore::ExchangeCancel);
        });
    }
}

/// Convert SocialStatus enum to u8 for network protocol
fn social_status_to_u8(status: SocialStatus) -> u8 {
    match status {
        SocialStatus::Online => 0,
        SocialStatus::DoNotDisturb => 1,
        SocialStatus::DayDreaming => 2,
        SocialStatus::NeedGroup => 3,
        SocialStatus::Grouped => 4,
        SocialStatus::LoneHunter => 5,
        SocialStatus::GroupHunting => 6,
        SocialStatus::NeedHelp => 7,
    }
}
