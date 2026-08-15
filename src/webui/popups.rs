use bevy::prelude::*;

use game_ui::{CoreToUi, UiToCore};

use crate::ecs::spell_casting::{SpellQueueState, SpellTargetingState};
use crate::network::PacketOutbox;
use crate::slint_support::popups::{PopupId, PopupManager};
use crate::webui::plugin::{UiInbound, UiOutbound};
use crate::webui::state::{
    ActiveMenuContext, ActiveWorldContextMenu, BoardSessionState, ExchangeSessionState,
};

pub fn handle_popup_requests(
    mut inbound: MessageReader<UiInbound>,
    mut popup_manager: ResMut<PopupManager>,
    mut targeting_state: ResMut<SpellTargetingState>,
    mut queue_state: ResMut<SpellQueueState>,
    outbox: Res<PacketOutbox>,
    mut menu_ctx: ResMut<ActiveMenuContext>,
    mut board_state: ResMut<BoardSessionState>,
    mut exchange_state: ResMut<ExchangeSessionState>,
    mut world_context: ResMut<ActiveWorldContextMenu>,
    mut outbound: MessageWriter<UiOutbound>,
) {
    for UiInbound(msg) in inbound.read() {
        let closed = match msg {
            UiToCore::PopupOpenRequest { id } => {
                let id = PopupId::from_slint(*id);
                if id == PopupId::MailBoard {
                    board_state.open_board_list();
                    tracing::info!("board: ui open list");
                    let mut ui = board_state.to_display_board_ui(false);
                    ui.visible = true;
                    outbound.write(UiOutbound(CoreToUi::DisplayBoard(ui)));
                    outbox.send(&packets::client::BoardInteraction::ListBoards);
                }
                popup_manager.open(id);
                None
            }
            UiToCore::PopupCloseRequest { id } => {
                let id = PopupId::from_slint(*id);
                popup_manager.close(id).then_some(id)
            }
            UiToCore::PopupCloseTop => popup_manager.close_top(),
            UiToCore::CancelSpellTargeting => {
                targeting_state.pending_target = None;
                queue_state.queued_spell = None;
                None
            }
            _ => None,
        };
        if let Some(id) = closed {
            popup_close_coordination(
                id,
                &outbox,
                &mut menu_ctx,
                &mut board_state,
                &mut exchange_state,
                &mut world_context,
                &mut outbound,
            );
        }
    }
}

fn popup_close_coordination(
    id: PopupId,
    outbox: &Res<PacketOutbox>,
    menu_ctx: &mut ActiveMenuContext,
    board_state: &mut BoardSessionState,
    exchange_state: &mut ExchangeSessionState,
    world_context: &mut ActiveWorldContextMenu,
    outbound: &mut MessageWriter<UiOutbound>,
) {
    match id {
        PopupId::NpcDialog => {
            if let Some(dialog_id) = menu_ctx.dialog_id {
                if let Some(entity_type) = menu_ctx.entity_type {
                    outbox.send(&packets::client::DialogInteraction {
                        entity_type,
                        entity_id: menu_ctx.entity_id,
                        pursuit_id: menu_ctx.pursuit_id.unwrap_or(0),
                        dialog_id,
                        args: packets::client::DialogInteractionArgs::None,
                    });
                }
            }
        }
        PopupId::MailBoard => {
            board_state.invalidate();
            outbound.write(UiOutbound(CoreToUi::DisplayBoard(
                board_state.to_display_board_ui(false),
            )));
            outbound.write(UiOutbound(board_state.to_compose_msg()));
            outbound.write(UiOutbound(board_state.to_delete_msg("")));
        }
        PopupId::Exchange => {
            if exchange_state.is_active {
                outbox.send(&packets::client::ExchangeInteraction::Cancel {
                    other_player_id: exchange_state.other_player_id,
                });
                exchange_state.reset();
            }
        }
        PopupId::ContextMenu => {
            world_context.entries.clear();
            world_context.title.clear();
            outbound.write(UiOutbound(CoreToUi::HideWorldContextMenu));
        }
        _ => {}
    }
}
