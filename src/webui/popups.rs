use bevy::prelude::*;

use game_ui::{CoreToUi, UiToCore};

use crate::ecs::spell_casting::{SpellQueueState, SpellTargetingState};
use crate::network::PacketOutbox;
use crate::slint_support::popups::{PopupId, PopupManager};
use crate::webui::plugin::{UiInbound, UiOutbound};
use crate::webui::state::{
    ActiveMenuContext, ActiveWorldContextMenu, BoardSessionState, ExchangeSessionState,
};

/// Route UI popup open/close requests into the PopupManager. User-initiated
/// closes also run server/client cleanup (NPC dialog, mail board, context menu).
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
                popup_manager.open(PopupId::from_slint(*id));
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

/// Server/client cleanup for user-initiated closes (skipped for
/// server-initiated ones like `DisplayMenuClose`).
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
            // Tell the server we closed the dialog (mirrors `UiToCore::MenuClose`).
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