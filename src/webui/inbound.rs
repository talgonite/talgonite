use bevy::prelude::*;

use game_types::SlotPanelType;
use game_ui::{ActionId, CoreToUi, UiToCore};
use packets::client;
use packets::types::MenuType;
use rendering::scene::utils::screen_to_iso_tile;

use crate::app_state::AppState;
use crate::ecs::components::{EntityId, Hitbox, LocalPlayer, NPC, Player, Position};
use crate::ecs::hotbar::{HotbarPanelState, HotbarState};
use crate::ecs::interaction::ActiveDragState;
use crate::ecs::macros::MacroManager;
use crate::ecs::social_status::LocalSocialStatus;
use crate::events::{
    AbilityEvent, ChatEvent, InteractionIntentAction, InteractionIntentEvent,
    InteractionTargetKind, InventoryEvent,
};
use crate::network::PacketOutbox;
use crate::resources::ZoomState;
use crate::settings_types::Settings as SettingsFile;
use crate::slint_plugin::ShowSelfProfileEvent;
use crate::webui::input::InputBindingResources;
use crate::webui::plugin::{UiInbound, UiOutbound};
use crate::webui::settings::{
    apply_modifier_rows_change, apply_rebind_key, apply_scale_input_change, apply_settings_change,
    apply_unbind_key, apply_volume_change, write_snapshot_and_sync,
};
use crate::webui::state::{
    AbilityState, ActiveMenuContext, ActiveWindowType, ActiveWorldContextMenu, BoardSessionState,
    ExchangeSessionState, GroupState, InventoryState, WorldListState,
};
use crate::{Camera, CurrentSession, WindowSurface};

pub(crate) fn handle_ui_inbound_settings(
    mut inbound: MessageReader<UiInbound>,
    mut outbound: MessageWriter<UiOutbound>,
    mut settings: ResMut<SettingsFile>,
    mut zoom_state: ResMut<ZoomState>,
    bindings: InputBindingResources,
) {
    let mut input_bindings = bindings.input_bindings;
    let mut unified_bindings = bindings.unified_bindings;

    for UiInbound(msg) in inbound.read() {
        match msg {
            UiToCore::RequestSnapshot => {
                write_snapshot_and_sync(&mut outbound, &settings);
            }
            UiToCore::SettingsChange { xray_size } => {
                apply_settings_change(*xray_size, &mut settings);
            }
            UiToCore::VolumeChange { sfx, music } => {
                apply_volume_change(*sfx, *music, &mut settings);
            }
            UiToCore::ScaleInputChange { progress } => {
                let scale = apply_scale_input_change(*progress, &mut settings);
                zoom_state.set_zoom(scale);
            }
            UiToCore::ModifierHotbarRowsTargetCustomOnlyChange { enabled } => {
                apply_modifier_rows_change(*enabled, &mut settings);
            }
            UiToCore::RebindKey {
                action,
                new_key,
                index,
            } => {
                apply_rebind_key(
                    action,
                    new_key,
                    *index,
                    &mut settings,
                    &mut input_bindings,
                    &mut unified_bindings,
                );
            }
            UiToCore::UnbindKey { action, index } => {
                apply_unbind_key(
                    action,
                    *index,
                    &mut settings,
                    &mut input_bindings,
                    &mut unified_bindings,
                );
            }
            UiToCore::ExitApplication => {
                let _ = slint::quit_event_loop();
            }
            _ => {}
        }
    }
}

pub(crate) fn handle_ui_inbound_world(
    mut inbound: MessageReader<UiInbound>,
    mut outbound: MessageWriter<UiOutbound>,
    outbox: Res<PacketOutbox>,
    mut world_context: ResMut<ActiveWorldContextMenu>,
    mut interaction_intents: MessageWriter<InteractionIntentEvent>,
    mut profile_events: MessageWriter<ShowSelfProfileEvent>,
    entity_ids: Query<(&EntityId, Option<&LocalPlayer>)>,
    mut world_list_state: ResMut<WorldListState>,
    mut local_social_status: ResMut<LocalSocialStatus>,
) {
    for UiInbound(msg) in inbound.read() {
        match msg {
            UiToCore::WorldContextMenuSelect { id } => {
                let selected = world_context
                    .entries
                    .iter()
                    .find(|entry| entry.id == *id)
                    .cloned();

                world_context.entries.clear();
                world_context.title.clear();
                outbound.write(UiOutbound(CoreToUi::HideWorldContextMenu));

                let Some(selected) = selected else {
                    continue;
                };

                match selected.action {
                    crate::events::WorldContextAction::WalkToTile { tile_x, tile_y } => {
                        interaction_intents.write(InteractionIntentEvent {
                            source: crate::events::ClickSource::AndroidLongPress,
                            target_kind: InteractionTargetKind::Ground,
                            target_entity: None,
                            tile_x,
                            tile_y,
                            action: InteractionIntentAction::WalkToTile,
                        });
                    }
                    crate::events::WorldContextAction::ApproachActor {
                        entity,
                        tile_x,
                        tile_y,
                    } => {
                        interaction_intents.write(InteractionIntentEvent {
                            source: crate::events::ClickSource::AndroidLongPress,
                            target_kind: InteractionTargetKind::Actor,
                            target_entity: Some(entity),
                            tile_x,
                            tile_y,
                            action: InteractionIntentAction::ApproachAndFace,
                        });
                    }
                    crate::events::WorldContextAction::ViewProfile { entity, is_self } => {
                        if is_self {
                            profile_events.write(ShowSelfProfileEvent::SelfRequested);
                            outbox.send(&packets::client::SelfProfileRequest {});
                        } else if let Ok((entity_id, _)) = entity_ids.get(entity) {
                            profile_events.write(ShowSelfProfileEvent::OtherRequested);
                            outbox.send(&packets::client::Click::TargetEntity(entity_id.id));
                        }
                    }
                    crate::events::WorldContextAction::PickUpItem { tile_x, tile_y } => {
                        outbox.send(&packets::client::Pickup {
                            destination_slot: 0,
                            source_point: (tile_x.max(0) as u16, tile_y.max(0) as u16),
                        });
                    }
                    crate::events::WorldContextAction::SpeakToNpc { entity } => {
                        if let Ok((entity_id, _)) = entity_ids.get(entity) {
                            outbox.send(&packets::client::Click::TargetEntity(entity_id.id));
                        }
                    }
                    crate::events::WorldContextAction::Trade { entity } => {
                        if let Ok((entity_id, _)) = entity_ids.get(entity) {
                            outbox.send(&packets::client::ExchangeInteraction::Start {
                                other_player_id: entity_id.id,
                            });
                        }
                    }
                    crate::events::WorldContextAction::InteractWalls { walls } => {
                        for (tile_x, tile_y, is_right) in walls {
                            outbox.send(&packets::client::Click::TargetWall {
                                x: tile_x.max(0) as u16,
                                y: tile_y.max(0) as u16,
                                is_right,
                            });
                        }
                    }
                }
            }
            UiToCore::WorldContextMenuClose => {
                world_context.entries.clear();
                world_context.title.clear();
                outbound.write(UiOutbound(CoreToUi::HideWorldContextMenu));
            }
            UiToCore::WorldMapClick {
                map_id,
                x,
                y,
                check_sum,
            } => {
                outbox.send(&packets::client::WorldMapClick {
                    check_sum: *check_sum,
                    map_id: *map_id,
                    point: (*x, *y),
                });
            }
            UiToCore::RequestWorldList => {
                outbox.send(&packets::client::WorldListRequest);
            }
            UiToCore::SetWorldListFilter { filter } => {
                world_list_state.filter = filter.clone();
                world_list_state.version = world_list_state.version.wrapping_add(1);
            }
            UiToCore::SetSocialStatus { status } => {
                if let Ok(social_status) = packets::types::SocialStatus::try_from(*status) {
                    local_social_status.set_status(social_status);
                    outbox.send(&packets::client::SocialStatus {
                        social_status: *status,
                    });
                }
            }
            UiToCore::RequestSelfProfile => {
                outbox.send(&packets::client::SelfProfileRequest {});
            }
            _ => {}
        }
    }
}

pub(crate) fn handle_ui_inbound_menus(
    mut inbound: MessageReader<UiInbound>,
    mut outbound: MessageWriter<UiOutbound>,
    outbox: Res<PacketOutbox>,
    mut menu_ctx: ResMut<ActiveMenuContext>,
    mut board_state: ResMut<BoardSessionState>,
    mut chat_events: MessageWriter<ChatEvent>,
    mut next_state: ResMut<NextState<AppState>>,
    time: Res<Time>,
) {
    for UiInbound(msg) in inbound.read() {
        match msg {
            UiToCore::MenuSelect { id, name } => {
                if menu_ctx.window_type == ActiveWindowType::Info {
                    menu_ctx.window_type = ActiveWindowType::None;
                    outbound.write(UiOutbound(CoreToUi::DisplayMenuClose));
                    continue;
                }

                if let Some(dialog_id) = menu_ctx.dialog_id {
                    if let Some(entity_type) = menu_ctx.entity_type {
                        let mut final_dialog_id = *id;
                        let args = if *id == dialog_id as i32 {
                            packets::client::DialogInteractionArgs::TextResponse {
                                args: vec![name.clone()],
                            }
                        } else if *id >= 100_000 {
                            final_dialog_id = dialog_id as i32 + 1;
                            packets::client::DialogInteractionArgs::MenuResponse {
                                option: (*id - 100_000 + 1) as u8,
                            }
                        } else {
                            packets::client::DialogInteractionArgs::None
                        };

                        outbox.send(&packets::client::DialogInteraction {
                            entity_type,
                            entity_id: menu_ctx.entity_id,
                            pursuit_id: menu_ctx.pursuit_id.unwrap_or(0),
                            dialog_id: final_dialog_id as u16,
                            args,
                        });
                    }
                    continue;
                }

                let is_slot_interaction = matches!(
                    menu_ctx.menu_type,
                    Some(MenuType::ShowPlayerItems)
                        | Some(MenuType::ShowPlayerSpells)
                        | Some(MenuType::ShowPlayerSkills)
                );

                let args = if is_slot_interaction {
                    packets::client::MenuInteractionArgs::Slot(*id as u8)
                } else {
                    let mut topics = Vec::new();
                    if !menu_ctx.args.is_empty() {
                        topics.push(menu_ctx.args.clone());
                    }
                    if !name.is_empty() {
                        topics.push(name.clone());
                    }

                    if topics.is_empty() {
                        packets::client::MenuInteractionArgs::Slot(0)
                    } else {
                        packets::client::MenuInteractionArgs::Topics(topics)
                    }
                };

                if let Some(entity_type) = menu_ctx.entity_type {
                    outbox.send(&packets::client::MenuInteraction {
                        entity_type,
                        entity_id: menu_ctx.entity_id,
                        pursuit_id: menu_ctx.pursuit_id.unwrap_or(*id as _),
                        args,
                    });
                } else {
                    tracing::warn!("MenuSelect with no entity_type in context");
                }
            }
            UiToCore::MenuClose => {
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
                // Dialog closed by user - clear menu context
                tracing::debug!("MenuClose requested");
            }
            UiToCore::ChatSubmit { mode, text, target } => {
                let body = text.trim();
                if body.is_empty() {
                    continue;
                }
                if mode == "whisper" {
                    if let Some(t) = target.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()) {
                        chat_events.write(ChatEvent::SendWhisper(t.to_string(), body.to_string()));
                    } else {
                        // Fallback: if no target treat as say
                        chat_events.write(ChatEvent::SendPublicMessage(
                            body.to_string(),
                            client::PublicMessageType::Normal,
                        ));
                    }
                } else {
                    chat_events.write(ChatEvent::SendPublicMessage(
                        body.to_string(),
                        client::PublicMessageType::Normal,
                    ));
                }
            }
            UiToCore::MailBoardOpenPost { index, post_id } => {
                let Ok(post_id) = i16::try_from(*post_id) else {
                    continue;
                };
                let Some(board_id) = board_state.active_board_id else {
                    continue;
                };

                board_state.selected_index = *index;
                tracing::info!(board_id, post_id, index = *index, "board: ui select post");

                if board_state.has_cached_message(*index, i32::from(post_id)) {
                    board_state.loading_post_id = -1;
                    outbound.write(UiOutbound(CoreToUi::DisplayBoard(
                        board_state.to_display_board_ui(false),
                    )));
                    continue;
                }

                board_state.loading_post_id = post_id as i32;
                outbox.send(&client::BoardInteraction::ViewPost {
                    board_id,
                    post_id,
                    navigation: None,
                });
            }
            UiToCore::MailBoardSelectBoard { index } => {
                let Ok(index) = usize::try_from(*index) else {
                    continue;
                };
                let Some(entry) = board_state.boards.get(index) else {
                    continue;
                };
                let Ok(board_id) = u16::try_from(entry.board_id) else {
                    continue;
                };
                let board_name = entry.name.clone();
                tracing::info!(
                    board_id,
                    name = %entry.name,
                    "board: ui select board"
                );

                board_state.select_board(board_id, board_name);
                outbound.write(UiOutbound(CoreToUi::DisplayBoard(
                    board_state.to_display_board_ui(false),
                )));

                outbox.send(&client::BoardInteraction::ViewBoard {
                    board_id,
                    start_post_id: i16::MAX,
                });
            }
            UiToCore::MailBoardComposeNew => {
                let Some(board_id) = board_state.active_board_id else {
                    continue;
                };
                if !board_state.can_post {
                    continue;
                }
                board_state.compose_open = true;
                board_state.compose_reply_to = None;
                board_state.compose_title = if board_id == 0 {
                    "New Mail".to_string()
                } else {
                    "New Post".to_string()
                };
                board_state.compose_subject.clear();
                board_state.compose_waiting = false;
                board_state.compose_result.clear();
                board_state.compose_submitted_at = None;
                tracing::info!(board_id, "board: ui compose new");
                outbound.write(UiOutbound(board_state.to_compose_msg()));
            }
            UiToCore::MailBoardComposeReply { index, post_id } => {
                let Some(board_id) = board_state.active_board_id else {
                    continue;
                };
                let Ok(index) = usize::try_from(*index) else {
                    continue;
                };
                let Some(post) = board_state
                    .posts
                    .get(index)
                    .filter(|entry| entry.post_id == *post_id && entry.can_reply)
                else {
                    continue;
                };
                let author = post.author.clone();
                let title = post.title.clone();
                board_state.compose_open = true;
                board_state.compose_reply_to = Some(author.clone());
                board_state.compose_title = "Reply".to_string();
                board_state.compose_subject = format!("Re: {}", title);
                board_state.compose_waiting = false;
                board_state.compose_result.clear();
                board_state.compose_submitted_at = None;
                tracing::info!(board_id, post_id, author = %author, "board: ui compose reply");
                outbound.write(UiOutbound(board_state.to_compose_msg()));
            }
            UiToCore::MailBoardComposeSend {
                name,
                subject,
                body,
            } => {
                let Some(board_id) = board_state.active_board_id else {
                    continue;
                };
                let reply_to = board_state.compose_reply_to.clone();
                if board_id == 0 && reply_to.is_none() && name.trim().is_empty() {
                    tracing::warn!("board: mail compose send with empty recipient");
                    continue;
                }
                board_state.compose_waiting = true;
                board_state.compose_result.clear();
                board_state.compose_submitted_at = Some(time.elapsed());
                outbound.write(UiOutbound(board_state.to_compose_msg()));

                if let Some(to) = reply_to {
                    tracing::info!(board_id, to = %to, "board: send mail (reply)");
                    outbox.send(&client::BoardInteraction::SendMail {
                        board_id,
                        to,
                        subject: subject.clone(),
                        message: body.clone(),
                    });
                } else if board_id == 0 {
                    let to = name.trim();
                    tracing::info!(board_id, to, "board: send mail");
                    outbox.send(&client::BoardInteraction::SendMail {
                        board_id,
                        to: to.to_string(),
                        subject: subject.clone(),
                        message: body.clone(),
                    });
                } else {
                    tracing::info!(board_id, "board: new post");
                    outbox.send(&client::BoardInteraction::NewPost {
                        board_id,
                        subject: subject.clone(),
                        message: body.clone(),
                    });
                }
            }
            UiToCore::MailBoardComposeCancel => {
                board_state.reset_compose();
                outbound.write(UiOutbound(board_state.to_compose_msg()));
            }
            UiToCore::MailBoardDeletePost { index, post_id } => {
                if board_state.deleting_post_id.is_some() {
                    continue;
                }
                let Some(board_id) = board_state.active_board_id else {
                    continue;
                };
                let Ok(index) = usize::try_from(*index) else {
                    continue;
                };
                let Some(post) = board_state
                    .posts
                    .get(index)
                    .filter(|entry| entry.post_id == *post_id)
                else {
                    continue;
                };
                let Ok(post_id) = i16::try_from(post.post_id) else {
                    continue;
                };

                tracing::info!(board_id, post_id, "board: ui delete post");
                board_state.deleting_post_id = Some(i32::from(post_id));
                board_state.delete_requested_at = Some(time.elapsed());
                outbound.write(UiOutbound(board_state.to_delete_msg("")));
                outbox.send(&client::BoardInteraction::Delete { board_id, post_id });
            }
            UiToCore::MailBoardDeleteDismiss => {
                board_state.deleting_post_id = None;
                board_state.delete_requested_at = None;
                outbound.write(UiOutbound(board_state.to_delete_msg("")));
            }
            UiToCore::MailBoardClose => {
                board_state.invalidate();
                outbound.write(UiOutbound(CoreToUi::DisplayBoard(
                    board_state.to_display_board_ui(false),
                )));
                outbound.write(UiOutbound(board_state.to_compose_msg()));
                outbound.write(UiOutbound(board_state.to_delete_msg("")));
            }
            UiToCore::ReturnToMainMenu => {
                board_state.invalidate();
                next_state.set(AppState::MainMenu);
            }
            _ => {}
        }
    }
}

pub(crate) fn handle_ui_inbound_slots(
    mut inbound: MessageReader<UiInbound>,
    mut inventory_events: MessageWriter<InventoryEvent>,
    mut ability_events: MessageWriter<AbilityEvent>,
    outbox: Res<PacketOutbox>,
    inv_state: Res<InventoryState>,
    ability_state: Res<AbilityState>,
    exchange_state: Res<ExchangeSessionState>,
    mut active_drag: ResMut<ActiveDragState>,
    mut hotbar_state: ResMut<HotbarState>,
    session: Res<CurrentSession>,
    macro_manager: Res<MacroManager>,
    mut settings: ResMut<SettingsFile>,
    camera: Res<Camera>,
    window_surface: NonSend<WindowSurface>,
    zoom_state: Res<ZoomState>,
    entity_query: Query<(
        Entity,
        &Position,
        &Hitbox,
        Option<&EntityId>,
        Option<&NPC>,
        Option<&Player>,
        Option<&LocalPlayer>,
    )>,
) {
    for UiInbound(msg) in inbound.read() {
        match msg {
            UiToCore::ActivateAction { category, index } => match category {
                SlotPanelType::Item => {
                    if exchange_state.is_active && !exchange_state.my_accepted {
                        outbox.send(&packets::client::ExchangeInteraction::AddItem {
                            other_player_id: exchange_state.other_player_id,
                            source_slot: *index as u8,
                        });
                    } else {
                        inventory_events.write(InventoryEvent::Use { slot: *index as u8 });
                    }
                }
                SlotPanelType::Skill => {
                    ability_events.write(AbilityEvent::UseSkill { slot: *index as u8 });
                }
                SlotPanelType::Spell => {
                    ability_events.write(AbilityEvent::UseSpell { slot: *index as u8 });
                }
                SlotPanelType::Hotbar => {
                    let bar = *index / 12;
                    let slot_in_bar = *index % 12;

                    if let Some(config_slot) = hotbar_state.config.get_slot(bar, slot_in_bar) {
                        if !config_slot.action_id.is_empty() {
                            let action_id = ActionId::from_str(&config_slot.action_id);

                            match action_id.panel_type() {
                                SlotPanelType::Item => {
                                    if let Some(item) =
                                        inv_state.0.iter().find(|item| item.id == action_id)
                                    {
                                        inventory_events
                                            .write(InventoryEvent::Use { slot: item.slot });
                                    }
                                }
                                SlotPanelType::Skill => {
                                    if let Some(skill) =
                                        ability_state.skills.iter().find(|s| s.id == action_id)
                                    {
                                        ability_events
                                            .write(AbilityEvent::UseSkill { slot: skill.slot });
                                    }
                                }
                                SlotPanelType::Spell => {
                                    if let Some(spell) =
                                        ability_state.spells.iter().find(|s| s.id == action_id)
                                    {
                                        ability_events
                                            .write(AbilityEvent::UseSpell { slot: spell.slot });
                                    }
                                }
                                SlotPanelType::Macro => {
                                    let macros =
                                        settings.get_macros(session.server_id, &session.username);
                                    if let Some(script) = macros.get(action_id.as_str()) {
                                        macro_manager.execute(script);
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
                SlotPanelType::World
                | SlotPanelType::None
                | SlotPanelType::Macro
                | SlotPanelType::Exchange => {}
            },
            UiToCore::DragDropAction {
                src_category,
                src_index,
                dst_category,
                dst_index,
                x,
                y,
            } => match (src_category, dst_category) {
                (SlotPanelType::Item, SlotPanelType::Item) => {
                    outbox.send(&packets::client::SwapSlot {
                        panel_type: packets::client::SwapSlotPanelType::Inventory,
                        slot1: *src_index as u8,
                        slot2: *dst_index as u8,
                    });
                }
                (SlotPanelType::Skill, SlotPanelType::Skill) => {
                    outbox.send(&packets::client::SwapSlot {
                        panel_type: packets::client::SwapSlotPanelType::Skill,
                        slot1: *src_index as u8,
                        slot2: *dst_index as u8,
                    });
                }
                (SlotPanelType::Spell, SlotPanelType::Spell) => {
                    outbox.send(&packets::client::SwapSlot {
                        panel_type: packets::client::SwapSlotPanelType::Spell,
                        slot1: *src_index as u8,
                        slot2: *dst_index as u8,
                    });
                }
                (SlotPanelType::Item, SlotPanelType::Hotbar) => {
                    if let Some(item) = inv_state.0.iter().find(|i| i.slot == *src_index as u8) {
                        hotbar_state.assign_slot(*dst_index, item.id.as_str().to_string());

                        settings.set_hotbars(
                            session.server_id,
                            &session.username,
                            hotbar_state.config.clone(),
                        );
                    }
                }
                (SlotPanelType::Skill, SlotPanelType::Hotbar) => {
                    if let Some(skill) = ability_state
                        .skills
                        .iter()
                        .find(|s| s.slot == *src_index as u8)
                    {
                        hotbar_state.assign_slot(*dst_index, skill.id.as_str().to_string());

                        settings.set_hotbars(
                            session.server_id,
                            &session.username,
                            hotbar_state.config.clone(),
                        );
                    }
                }
                (SlotPanelType::Spell, SlotPanelType::Hotbar) => {
                    if let Some(spell) = ability_state
                        .spells
                        .iter()
                        .find(|s| s.slot == *src_index as u8)
                    {
                        hotbar_state.assign_slot(*dst_index, spell.id.as_str().to_string());

                        settings.set_hotbars(
                            session.server_id,
                            &session.username,
                            hotbar_state.config.clone(),
                        );
                    }
                }
                (SlotPanelType::Hotbar, SlotPanelType::Hotbar) => {
                    let bar1 = *src_index / 12;
                    let slot1 = *src_index % 12;
                    let bar2 = *dst_index / 12;
                    let slot2 = *dst_index % 12;

                    let slot1_action = hotbar_state.config.bars[bar1][slot1].action_id.clone();
                    let slot2_action = hotbar_state.config.bars[bar2][slot2].action_id.clone();

                    hotbar_state.config.set_slot(bar2, slot2, slot1_action);
                    hotbar_state.config.set_slot(bar1, slot1, slot2_action);

                    settings.set_hotbars(
                        session.server_id,
                        &session.username,
                        hotbar_state.config.clone(),
                    );
                }
                (_, SlotPanelType::World) => {
                    let camera = &camera;
                    let cam_pos = camera.camera.position();
                    let cam_zoom = camera.camera.zoom();
                    let win_size =
                        Vec2::new(window_surface.width as f32, window_surface.height as f32);

                    let cursor_scale = zoom_state.cursor_to_render_scale();
                    let screen = Vec2::new(*x * cursor_scale, *y * cursor_scale);
                    let tile = screen_to_iso_tile(screen, cam_pos, win_size, cam_zoom);
                    let tile_i = (tile.x.floor() as i32, tile.y.floor() as i32);

                    // Manual hit testing
                    let mut hits: Vec<(
                        Entity,
                        Option<&crate::ecs::components::EntityId>,
                        bool, // is_creature
                        bool, // is_local
                        f32,  // Y-sort height
                    )> = Vec::new();
                    for (entity, pos, hitbox, entity_id, npc, player, local) in entity_query.iter()
                    {
                        if hitbox.check_hit(
                            Vec2::new(pos.x, pos.y),
                            tile,
                            screen,
                            cam_pos,
                            win_size,
                            cam_zoom,
                        ) {
                            let is_creature = npc.is_some() || player.is_some();
                            let is_local = local.is_some();
                            hits.push((entity, entity_id, is_creature, is_local, pos.x + pos.y));
                        }
                    }
                    hits.sort_by(|a, b| b.4.partial_cmp(&a.4).unwrap_or(std::cmp::Ordering::Equal));

                    // If dropping an item, ignore hit-testing against yourself
                    if matches!(src_category, SlotPanelType::Item) {
                        hits.retain(|h| !h.3);
                    }

                    let hovered_info = if let Some((entity, entity_id, _, _, _)) = hits.first() {
                        if let Some(eid) = entity_id {
                            format!("entity {} (ID {})", entity.index(), eid.id)
                        } else {
                            format!("entity {}", entity.index())
                        }
                    } else {
                        "nothing".to_string()
                    };

                    tracing::info!(
                        "Dropped {:?} slot {} onto world at tile ({}, {}) over {}",
                        src_category,
                        src_index,
                        tile_i.0,
                        tile_i.1,
                        hovered_info
                    );

                    if matches!(src_category, SlotPanelType::Item) {
                        if let Some(item) = inv_state.0.iter().find(|i| i.slot == *src_index as u8)
                        {
                            if let Some((_, entity_id, is_creature, _, _)) = hits.first() {
                                if *is_creature {
                                    if let Some(eid) = entity_id {
                                        outbox.send(&client::ItemDroppedOnCreature {
                                            source_slot: item.slot,
                                            target_id: eid.id,
                                            count: if item.stackable { 0 } else { 1 },
                                        });
                                    }
                                } else {
                                    outbox.send(&client::ItemDrop {
                                        source_slot: item.slot,
                                        destination_point: (
                                            tile_i.0.max(0) as u16,
                                            tile_i.1.max(0) as u16,
                                        ),
                                        count: 1, // Only drop 1 as requested
                                    });
                                }
                            } else {
                                // Drop on empty tile
                                outbox.send(&client::ItemDrop {
                                    source_slot: item.slot,
                                    destination_point: (
                                        tile_i.0.max(0) as u16,
                                        tile_i.1.max(0) as u16,
                                    ),
                                    count: 1, // Only drop 1 as requested
                                });
                            }
                        }
                    }

                    if matches!(src_category, SlotPanelType::Spell) {
                        if let Some(spell) = ability_state
                            .spells
                            .iter()
                            .find(|s| s.slot == *src_index as u8)
                        {
                            let target_entity =
                                if spell.spell_type == packets::server::SpellType::NoTarget {
                                    None
                                } else {
                                    hits.first()
                                        .filter(|hit| hit.2)
                                        .and_then(|hit| hit.1.map(|eid| eid.id))
                                };

                            ability_events.write(AbilityEvent::UseSpellAt {
                                slot: spell.slot,
                                target_entity,
                                target_x: tile_i.0.max(0) as u16,
                                target_y: tile_i.1.max(0) as u16,
                            });
                        }
                    }

                    if matches!(src_category, SlotPanelType::Hotbar) {
                        let bar = *src_index / 12;
                        let slot = *src_index % 12;

                        let Some(config_slot) = hotbar_state.config.get_slot(bar, slot) else {
                            continue;
                        };

                        if config_slot.action_id.is_empty() {
                            continue;
                        }

                        let action_id = ActionId::from_str(&config_slot.action_id);

                        match action_id.panel_type() {
                            SlotPanelType::Item => {
                                let Some(item) =
                                    inv_state.0.iter().find(|item| item.id == action_id)
                                else {
                                    continue;
                                };

                                let target =
                                    hits.iter().find(|hit| !hit.3).map(|hit| (hit.1, hit.2));

                                match target {
                                    Some((Some(eid), true)) => {
                                        outbox.send(&client::ItemDroppedOnCreature {
                                            source_slot: item.slot,
                                            target_id: eid.id,
                                            count: if item.stackable { 0 } else { 1 },
                                        });
                                    }
                                    _ => {
                                        outbox.send(&client::ItemDrop {
                                            source_slot: item.slot,
                                            destination_point: (
                                                tile_i.0.max(0) as u16,
                                                tile_i.1.max(0) as u16,
                                            ),
                                            count: 1,
                                        });
                                    }
                                }
                            }
                            SlotPanelType::Skill => {
                                if let Some(skill) =
                                    ability_state.skills.iter().find(|s| s.id == action_id)
                                {
                                    ability_events
                                        .write(AbilityEvent::UseSkill { slot: skill.slot });
                                }
                            }
                            SlotPanelType::Spell => {
                                if let Some(spell) =
                                    ability_state.spells.iter().find(|s| s.id == action_id)
                                {
                                    let target_entity = if spell.spell_type
                                        == packets::server::SpellType::NoTarget
                                    {
                                        None
                                    } else {
                                        hits.first()
                                            .filter(|hit| hit.2)
                                            .and_then(|hit| hit.1.map(|eid| eid.id))
                                    };

                                    ability_events.write(AbilityEvent::UseSpellAt {
                                        slot: spell.slot,
                                        target_entity,
                                        target_x: tile_i.0.max(0) as u16,
                                        target_y: tile_i.1.max(0) as u16,
                                    });
                                }
                            }
                            SlotPanelType::Macro => {
                                let macros =
                                    settings.get_macros(session.server_id, &session.username);
                                if let Some(script) = macros.get(action_id.as_str()) {
                                    macro_manager.execute(script);
                                }
                            }
                            _ => {}
                        }
                    }
                }
                (SlotPanelType::Hotbar, SlotPanelType::None) => {
                    let bar = *src_index / 12;
                    let slot = *src_index % 12;
                    hotbar_state.config.set_slot(bar, slot, "".to_string());

                    settings.set_hotbars(
                        session.server_id,
                        &session.username,
                        hotbar_state.config.clone(),
                    );
                    tracing::info!("Deallocated hotbar slot {}", src_index);
                }
                (_, SlotPanelType::None) => {
                    tracing::info!("Drag action cancelled (dropped over safe UI background)");
                }
                _ => {
                    tracing::warn!(
                        "[webui] DragDropAction: unsupported category combination: {:?} -> {:?}",
                        src_category,
                        dst_category
                    );
                }
            },
            // --- Group actions (opcode 46 / ToggleGroup 47) ---
            UiToCore::Unequip { slot } => {
                inventory_events.write(InventoryEvent::Unequip { slot: *slot });
            }
            UiToCore::DragStateChanged {
                is_dragging,
                panel,
                index,
            } => {
                active_drag.is_dragging = *is_dragging;
                active_drag.source_panel = *panel;
                active_drag.source_index = *index;
            }
            _ => {}
        }
    }
}

pub(crate) fn handle_ui_inbound_group_exchange(
    mut inbound: MessageReader<UiInbound>,
    outbox: Res<PacketOutbox>,
    mut group_state: ResMut<GroupState>,
    mut exchange_state: ResMut<ExchangeSessionState>,
) {
    for UiInbound(msg) in inbound.read() {
        match msg {
            UiToCore::ToggleGroupable => {
                outbox.send(&packets::client::ToggleGroup);
                group_state.is_groupable = !group_state.is_groupable;
            }
            UiToCore::SendGroupInvite { name } => {
                outbox.send(&packets::client::GroupInvite::Request { name: name.clone() });
            }
            UiToCore::RespondGroupInvite {
                accept,
                source_name,
            } => {
                if *accept {
                    outbox.send(&packets::client::GroupInvite::Forced {
                        name: source_name.clone(),
                    });
                    outbox.send(&packets::client::SelfProfileRequest {});
                }
                group_state.pending_invite = None;
            }
            UiToCore::KickGroupMember { name } => {
                outbox.send(&packets::client::GroupInvite::Request { name: name.clone() });
            }
            UiToCore::LeaveGroup => {
                outbox.send(&packets::client::ToggleGroup);
                outbox.send(&packets::client::SelfProfileRequest {});
            }
            UiToCore::ExchangeAddItem { slot } => {
                if exchange_state.is_active && !exchange_state.my_accepted {
                    outbox.send(&packets::client::ExchangeInteraction::AddItem {
                        other_player_id: exchange_state.other_player_id,
                        source_slot: *slot,
                    });
                }
            }
            UiToCore::ExchangeAddStackableItem { slot, count } => {
                if exchange_state.is_active && !exchange_state.my_accepted {
                    tracing::info!("Offering {}x from inventory slot {}", count, slot);
                    outbox.send(&packets::client::ExchangeInteraction::AddStackableItem {
                        other_player_id: exchange_state.other_player_id,
                        source_slot: *slot,
                        item_count: *count,
                    });
                    exchange_state.quantity_prompt = None;
                }
            }
            UiToCore::ExchangeSetGold { amount } => {
                if exchange_state.is_active && !exchange_state.my_accepted {
                    let amt = (*amount).max(0) as u32;
                    exchange_state.my_gold = amt;
                    outbox.send(&packets::client::ExchangeInteraction::SetGold {
                        other_player_id: exchange_state.other_player_id,
                        gold_amount: amt,
                    });
                }
            }
            UiToCore::ExchangeCancelQuantity => {
                exchange_state.quantity_prompt = None;
            }
            UiToCore::ExchangeAccept => {
                if exchange_state.is_active && !exchange_state.my_accepted {
                    exchange_state.quantity_prompt = None;
                    tracing::info!(
                        "Sending ExchangeInteraction::Accept for other_player_id: {}",
                        exchange_state.other_player_id
                    );
                    exchange_state.my_accepted = true;
                    outbox.send(&packets::client::ExchangeInteraction::Accept {
                        other_player_id: exchange_state.other_player_id,
                    });
                }
            }
            UiToCore::ExchangeCancel => {
                if exchange_state.is_active {
                    outbox.send(&packets::client::ExchangeInteraction::Cancel {
                        other_player_id: exchange_state.other_player_id,
                    });
                    exchange_state.reset();
                }
            }
            _ => {}
        }
    }
}

pub(crate) fn handle_ui_inbound_hotbar(
    mut inbound: MessageReader<UiInbound>,
    mut hotbar_panel_state: ResMut<HotbarPanelState>,
) {
    for UiInbound(msg) in inbound.read() {
        match msg {
            UiToCore::SetHotbarPanel { panel_num } => {
                hotbar_panel_state.current_panel =
                    crate::ecs::hotbar::HotbarPanel::from_u8(*panel_num);
            }
            UiToCore::ExpandHotbarRows => {
                hotbar_panel_state.rows = hotbar_panel_state.rows.expand();
            }
            UiToCore::CollapseHotbarRows => {
                hotbar_panel_state.rows = hotbar_panel_state.rows.collapse();
            }
            _ => {}
        }
    }
}
