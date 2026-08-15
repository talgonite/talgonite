use bevy::prelude::*;

use game_ui::{
    ActionId, BoardPostUi, BoardStateUi, ChatEntryUi, Cooldown, CoreToUi, InventoryItemUi,
    MenuEntryUi, SkillUi, SpellUi, WorldListMemberUi, WorldMapNodeUi,
};
use packets::client;
use packets::server::display_menu::DisplayMenuPayload;

use crate::events::{AbilityEvent, ChatEvent, InventoryEvent, SessionEvent};
use crate::rich_text::RichText;
use crate::webui::plugin::UiOutbound;
use crate::webui::state::{
    AbilityState, ActiveMenuContext, ActiveWindowType, BoardSessionState, EquipmentState,
    ExchangeSessionState, ExchangeSlotItem, GroupState, InventoryState, PendingGroupInvite,
    PlayerProfileState, SkillCooldowns, WorldListState,
};

use std::time::Instant;

/// Server sends these when group membership changes; we request SelfProfile so the group panel stays in sync.
fn is_group_change_system_message(msg: &str) -> bool {
    let msg = msg.trim();
    msg.eq_ignore_ascii_case("Group disbanded.")
        || msg.contains("is joining this group.")
        || msg.contains("is leaving this group.")
        || msg.contains("has taken command of the group.")
}

pub(crate) fn bridge_chat_events(
    mut chat_events: MessageReader<ChatEvent>,
    mut outbound: MessageWriter<UiOutbound>,
    mut menu_ctx: ResMut<ActiveMenuContext>,
    outbox: Option<Res<crate::network::PacketOutbox>>,
) {
    use packets::server::{PublicMessageType, ServerMessageType};

    let mut to_append: Vec<ChatEntryUi> = Vec::new();
    for evt in chat_events.read() {
        match evt {
            ChatEvent::ServerMessage(pkt) => {
                if let Some(ref out) = outbox {
                    if is_group_change_system_message(&pkt.message) {
                        out.send(&packets::client::SelfProfileRequest {});
                    }
                }
                let (show_in_message_box, show_in_action_bar, color) = match pkt.message_type {
                    ServerMessageType::Whisper => (true, false, Some("#60a5fa".to_string())),
                    ServerMessageType::OrangeBar1
                    | ServerMessageType::OrangeBar2
                    | ServerMessageType::OrangeBar3
                    | ServerMessageType::OrangeBar5 => (true, true, Some("#ff9800".to_string())),
                    ServerMessageType::ActiveMessage | ServerMessageType::AdminMessage => {
                        (true, true, Some("#ff9800".to_string()))
                    }
                    ServerMessageType::GroupChat => (true, false, Some("#9acd32".to_string())),
                    ServerMessageType::GuildChat => (true, false, Some("#808000".to_string())),
                    ServerMessageType::ScrollWindow
                    | ServerMessageType::NonScrollWindow
                    | ServerMessageType::WoodenBoard => {
                        let title = match pkt.message_type {
                            ServerMessageType::WoodenBoard => "Wooden Board",
                            _ => "Information",
                        };
                        menu_ctx.window_type = ActiveWindowType::Info;
                        menu_ctx.dialog_id = None;
                        menu_ctx.menu_type = None;
                        menu_ctx.pursuit_id = None;
                        menu_ctx.entity_type = None;
                        menu_ctx.entity_id = 0;

                        outbound.write(UiOutbound(CoreToUi::DisplayMenu {
                            title: title.to_string(),
                            text: pkt.message.clone(),
                            sprite_id: 0,
                            entry_type: crate::webui::ipc::MenuEntryType::TextOptions,
                            entries: vec![MenuEntryUi::text_option("Close".to_string(), 0)],
                        }));
                        continue;
                    }
                    ServerMessageType::ClosePopup => {
                        if menu_ctx.window_type == ActiveWindowType::Info {
                            menu_ctx.window_type = ActiveWindowType::None;
                            outbound.write(UiOutbound(CoreToUi::DisplayMenuClose));
                        }
                        continue;
                    }
                    ServerMessageType::UserOptions | ServerMessageType::PersistentMessage => {
                        continue;
                    }
                };

                to_append.push(ChatEntryUi {
                    kind: "server".to_string(),
                    message_type: Some(pkt.message_type as u8),
                    text: pkt.message.clone(),
                    show_in_message_box,
                    show_in_action_bar,
                    color,
                });
            }
            ChatEvent::PublicMessage(pkt) => {
                if pkt.message_type == PublicMessageType::Chant {
                    continue;
                }

                let color = match pkt.message_type {
                    PublicMessageType::Normal => Some("#d0d0d0".to_string()),
                    PublicMessageType::Shout => Some("#ffeb3b".to_string()),
                    PublicMessageType::Chant => None,
                };

                to_append.push(ChatEntryUi {
                    kind: "public".to_string(),
                    message_type: Some(pkt.message_type as u8),
                    text: pkt.message.clone(),
                    show_in_message_box: true,
                    show_in_action_bar: false,
                    color,
                });
            }
            _ => {}
        }
    }
    if !to_append.is_empty() {
        outbound.write(UiOutbound(CoreToUi::ChatAppend { entries: to_append }));
    }
}

pub(crate) fn bridge_session_events(
    mut session_events: MessageReader<SessionEvent>,
    mut outbound: MessageWriter<UiOutbound>,
    outbox: Res<crate::network::PacketOutbox>,
    mut menu_ctx: ResMut<ActiveMenuContext>,
    inv_state: Res<InventoryState>,
    ability_state: Res<AbilityState>,
    mut profile_state: ResMut<PlayerProfileState>,
    mut show_profile: MessageWriter<crate::slint_plugin::ShowSelfProfileEvent>,
    mut world_list_state: ResMut<WorldListState>,
    mut board_state: ResMut<BoardSessionState>,
    mut exchange_state: ResMut<ExchangeSessionState>,
    mut group_state: ResMut<GroupState>,
    mut popup_manager: ResMut<crate::slint_support::popups::PopupManager>,
    mut chat_events: MessageWriter<ChatEvent>,
) {
    for evt in session_events.read() {
        match evt {
            SessionEvent::WorldList(pkt) => {
                world_list_state.raw = Some(pkt.clone());
                world_list_state.version = world_list_state.version.wrapping_add(1);
            }
            SessionEvent::DisplayDialog(pkt) => {
                match pkt {
                    packets::server::DisplayDialog::Show { header, payload } => {
                        menu_ctx.window_type = ActiveWindowType::Dialog;
                        menu_ctx.entity_type = Some(header.entity_type);
                        menu_ctx.entity_id = header.source_id;
                        menu_ctx.pursuit_id = Some(header.pursuit_id);
                        menu_ctx.dialog_id = Some(header.dialog_id);
                        menu_ctx.menu_type = None;
                        menu_ctx.args.clear();

                        let mut entries = Vec::new();
                        // Put Previous above Next as requested
                        if header.has_previous_button {
                            entries.push(MenuEntryUi::text_option(
                                "Previous".to_string(),
                                header.dialog_id as i32 - 1,
                            ));
                        }

                        let mut is_text_entry = false;
                        let mut prompt = String::new();
                        match payload {
                            packets::server::DisplayDialogPayload::DialogMenu { options }
                            | packets::server::DisplayDialogPayload::CreatureMenu { options } => {
                                for (idx, option) in options.iter().enumerate() {
                                    // Use a high range for menu options to avoid collisions with Previous/Next/Base IDs
                                    entries.push(MenuEntryUi::text_option(
                                        option.clone(),
                                        100_000 + idx as i32,
                                    ));
                                }
                            }
                            packets::server::DisplayDialogPayload::TextEntry { info } => {
                                is_text_entry = true;
                                prompt = info.prompt.clone();
                            }
                            _ => {}
                        }

                        if header.has_next_button {
                            entries.push(MenuEntryUi::text_option(
                                "Next".to_string(),
                                header.dialog_id as i32 + 1,
                            ));
                        }

                        if is_text_entry {
                            outbound.write(UiOutbound(CoreToUi::DisplayMenuTextEntry {
                                title: header.name.clone(),
                                text: header.text.clone(),
                                prompt,
                                sprite_id: header.sprite,
                                args: String::new(),
                                entries,
                            }));
                        } else {
                            outbound.write(UiOutbound(CoreToUi::DisplayMenu {
                                title: header.name.clone(),
                                text: header.text.clone(),
                                sprite_id: header.sprite,
                                entry_type: crate::webui::ipc::MenuEntryType::TextOptions,
                                entries,
                            }));
                        }
                    }
                    packets::server::DisplayDialog::Close => {
                        menu_ctx.window_type = ActiveWindowType::None;
                        menu_ctx.dialog_id = None;
                        outbound.write(UiOutbound(CoreToUi::DisplayMenuClose));
                    }
                }
            }
            SessionEvent::DisplayBoard(pkt) => match pkt {
                packets::server::DisplayBoard::PublicBoard { board }
                | packets::server::DisplayBoard::MailBoard { board } => {
                    let response_session_token = board_state
                        .pending_request_session_token
                        .unwrap_or(board_state.session_token);

                    let mut posts = board
                        .posts
                        .iter()
                        .map(|post| BoardPostUi {
                            post_id: post.post_id as i32,
                            author: post.author.clone(),
                            month_of_year: post.month_of_year as i32,
                            day_of_month: post.day_of_month as i32,
                            title: post.subject.clone(),
                            message: String::new(),
                            is_unread: post.is_unread,
                            can_reply: true,
                            can_delete: false,
                        })
                        .collect::<Vec<_>>();

                    for post in &mut posts {
                        board_state.merge_cached_post(post);
                    }

                    board_state.visible = true;
                    board_state.active_board_id = Some(board.board_id);
                    board_state.board_name = board.name.clone();
                    board_state.last_post_id = board.posts.last().map(|post| post.post_id);
                    let append = board_state.pending_request_session_token.is_some();
                    let initial_post_id = (!append)
                        .then(|| {
                            posts.first().and_then(|post| {
                                if post.message.is_empty() {
                                    i16::try_from(post.post_id).ok()
                                } else {
                                    None
                                }
                            })
                        })
                        .flatten();

                    if append {
                        board_state.posts.extend(posts.iter().cloned());
                    } else {
                        board_state.posts = posts.clone();
                        if !board_state.posts.is_empty() {
                            board_state.selected_index = 0;
                        } else {
                            board_state.selected_index = -1;
                        }

                        if let Some(post_id) = initial_post_id {
                            board_state.selected_index = 0;
                            board_state.loading_post_id = i32::from(post_id);
                        } else {
                            board_state.loading_post_id = -1;
                        }
                    }

                    outbound.write(UiOutbound(CoreToUi::DisplayBoard(BoardStateUi {
                        visible: true,
                        board_name: board_state.board_name.clone(),
                        selected_index: board_state.selected_index,
                        loading_post_id: board_state.loading_post_id,
                        session_token: response_session_token,
                        append,
                        posts,
                    })));

                    board_state.pending_request_session_token = None;
                    board_state.pending_start_post_id = None;

                    if let Some(post_id) = initial_post_id {
                        outbox.send(&client::BoardInteraction::ViewPost {
                            board_id: board.board_id,
                            post_id,
                            navigation: None,
                        });
                    }

                    if board_state.visible && !board.posts.is_empty() {
                        if let Some(last_post_id) = board_state.last_post_id {
                            let start_post_id = last_post_id.saturating_sub(1);
                            board_state.mark_request(start_post_id);
                            outbox.send(&client::BoardInteraction::ViewBoard {
                                board_id: board.board_id,
                                start_post_id,
                            });
                        }
                    }
                }
                packets::server::DisplayBoard::MailPost {
                    enable_prev_btn: _,
                    post,
                    message,
                }
                | packets::server::DisplayBoard::PublicPost {
                    enable_prev_btn: _,
                    post,
                    message,
                } => {
                    let post_id = post.post_id as i32;
                    if let Some(index) = board_state
                        .posts
                        .iter()
                        .position(|entry| entry.post_id == post_id)
                    {
                        let entry = &mut board_state.posts[index];
                        entry.author = post.author.clone();
                        entry.month_of_year = post.month_of_year as i32;
                        entry.day_of_month = post.day_of_month as i32;
                        entry.title = post.subject.clone();
                        entry.message = message.clone();
                        entry.is_unread = false;

                        board_state.selected_index = index as i32;
                        if board_state.loading_post_id == post_id {
                            board_state.loading_post_id = -1;
                        }

                        outbound.write(UiOutbound(CoreToUi::DisplayBoard(BoardStateUi {
                            visible: board_state.visible,
                            board_name: board_state.board_name.clone(),
                            selected_index: board_state.selected_index,
                            loading_post_id: board_state.loading_post_id,
                            session_token: board_state.session_token,
                            append: false,
                            posts: board_state.posts.clone(),
                        })));
                    }
                }
                packets::server::DisplayBoard::BoardList { .. }
                | packets::server::DisplayBoard::SubmitResponse { .. }
                | packets::server::DisplayBoard::DeleteResponse { .. }
                | packets::server::DisplayBoard::MarkUnreadResponse { .. } => continue,
            },
            SessionEvent::SelfProfile(pkt) => {
                profile_state.is_self = true;
                profile_state.entity_id = None; // Local player
                profile_state.name.clear();
                profile_state.portrait = None;
                profile_state.equipment.clear();
                profile_state.class = pkt.display_class.clone();
                profile_state.guild = pkt.guild_name.clone();
                profile_state.guild_rank = pkt.guild_rank.clone();
                profile_state.title = pkt.title.clone();
                profile_state.nation = pkt.nation;
                profile_state.group_string = RichText::parse(&pkt.group_string);
                profile_state.group_open = pkt.group_open;
                profile_state.profile_text = RichText::parse(&pkt.group_string);
                profile_state.legend_marks = pkt.legend_marks.clone();
                // Parse group_string into member list. Server marks leader with "* " prefix (e.g. "* Tedders").
                group_state.is_groupable = pkt.group_open;
                let lines: Vec<String> = RichText::parse(&pkt.group_string)
                    .to_plain_string()
                    .lines()
                    .map(|l| l.trim().to_string())
                    .filter(|l| !l.is_empty())
                    .collect();
                group_state.members = lines
                    .into_iter()
                    .filter(|l| {
                        let s = l.as_str();
                        if s.eq_ignore_ascii_case("Group members") {
                            return false;
                        }
                        if s.starts_with("Total ")
                            && s["Total ".len()..].trim().parse::<u32>().is_ok()
                        {
                            return false;
                        }
                        if s.starts_with("Spouse:") {
                            return false;
                        }
                        true
                    })
                    .map(|l| {
                        let is_leader = l.trim_start().starts_with("* ");
                        let name = l
                            .trim_start()
                            .strip_prefix("* ")
                            .unwrap_or(l.trim_start())
                            .trim()
                            .to_string();
                        (name, is_leader)
                    })
                    .collect();
                show_profile.write(crate::slint_plugin::ShowSelfProfileEvent::SelfUpdate);
            }
            SessionEvent::OtherProfile(pkt) => {
                profile_state.is_self = false;
                profile_state.entity_id = Some(pkt.id);
                profile_state.name = pkt.name.clone();
                profile_state.class = pkt.display_class.clone();
                profile_state.guild = pkt.guild_name.clone();
                profile_state.guild_rank = pkt.guild_rank.clone();
                profile_state.title = pkt.title.clone();
                profile_state.nation = pkt.nation;
                profile_state.group_open = pkt.group_open;
                profile_state.profile_text =
                    RichText::parse(pkt.profile_text.as_deref().unwrap_or_default());
                profile_state.social_status = pkt.social_status;
                profile_state.legend_marks = pkt.legend_marks.clone();
                profile_state.portrait = pkt.portrait.clone();
                profile_state.equipment = pkt.equipment.clone();
                show_profile.write(crate::slint_plugin::ShowSelfProfileEvent::OtherUpdate);
            }
            SessionEvent::WorldMap(pkt) => {
                let nodes = pkt
                    .nodes
                    .iter()
                    .map(|n| WorldMapNodeUi {
                        text: n.text.clone(),
                        map_id: n.map_id,
                        x: n.screen_position.0 as u16,
                        y: n.screen_position.1 as u16,
                        dest_x: n.destination_point.0 as u16,
                        dest_y: n.destination_point.1 as u16,
                        check_sum: n.check_sum,
                    })
                    .collect();
                outbound.write(UiOutbound(CoreToUi::WorldMapOpen {
                    field_name: pkt.field_name.clone(),
                    nodes,
                }));
            }
            SessionEvent::DisplayMenu(pkt) => {
                menu_ctx.window_type = ActiveWindowType::Menu;
                menu_ctx.entity_type = pkt.header.entity_type.into();
                menu_ctx.entity_id = pkt.header.source_id;
                menu_ctx.menu_type = Some(pkt.menu_type);
                menu_ctx.args.clear();
                menu_ctx.dialog_id = None;

                let mut entries = Vec::new();
                let mut entry_type = crate::webui::ipc::MenuEntryType::TextOptions;
                let mut is_text_entry = false;

                match &pkt.payload {
                    DisplayMenuPayload::Menu { options } => {
                        menu_ctx.pursuit_id = None;
                        entries = options
                            .iter()
                            .map(|(text, id)| MenuEntryUi::text_option(text.clone(), *id as i32))
                            .collect();
                    }
                    DisplayMenuPayload::MenuWithArgs { args, options } => {
                        menu_ctx.pursuit_id = None;
                        menu_ctx.args = args.clone();
                        entries = options
                            .iter()
                            .map(|(text, id)| MenuEntryUi::text_option(text.clone(), *id as i32))
                            .collect();
                    }
                    DisplayMenuPayload::ShowItems { pursuit_id, items } => {
                        menu_ctx.pursuit_id = Some(*pursuit_id);
                        entry_type = crate::webui::ipc::MenuEntryType::Items;
                        entries = items
                            .iter()
                            .enumerate()
                            .map(|(idx, item)| {
                                MenuEntryUi::shop_item(
                                    item.name.clone(),
                                    (idx + 1) as i32,
                                    item.sprite,
                                    item.color,
                                    item.cost,
                                )
                            })
                            .collect();
                    }
                    DisplayMenuPayload::ShowSpells { pursuit_id, spells } => {
                        menu_ctx.pursuit_id = Some(*pursuit_id);
                        entry_type = crate::webui::ipc::MenuEntryType::Spells;
                        entries = spells
                            .iter()
                            .enumerate()
                            .map(|(idx, spell)| {
                                MenuEntryUi::ability(
                                    spell.name.clone(),
                                    (idx + 1) as i32,
                                    spell.sprite,
                                )
                            })
                            .collect();
                    }
                    DisplayMenuPayload::ShowSkills { pursuit_id, skills } => {
                        menu_ctx.pursuit_id = Some(*pursuit_id);
                        entry_type = crate::webui::ipc::MenuEntryType::Skills;
                        entries = skills
                            .iter()
                            .enumerate()
                            .map(|(idx, skill)| {
                                MenuEntryUi::ability(
                                    skill.name.clone(),
                                    (idx + 1) as i32,
                                    skill.sprite,
                                )
                            })
                            .collect();
                    }
                    DisplayMenuPayload::TextEntry { pursuit_id } => {
                        menu_ctx.pursuit_id = Some(*pursuit_id);
                        is_text_entry = true;
                    }
                    DisplayMenuPayload::TextEntryWithArgs { args, pursuit_id } => {
                        menu_ctx.pursuit_id = Some(*pursuit_id);
                        menu_ctx.args = args.clone();
                        is_text_entry = true;
                    }
                    DisplayMenuPayload::ShowPlayerItems { pursuit_id, slots } => {
                        menu_ctx.pursuit_id = Some(*pursuit_id);
                        entry_type = crate::webui::ipc::MenuEntryType::Items;
                        entries = slots
                            .iter()
                            .filter_map(|&slot| {
                                inv_state.0.iter().find(|i| i.slot == slot).map(|item| {
                                    MenuEntryUi::shop_item(
                                        item.name.clone(),
                                        slot as i32,
                                        item.sprite,
                                        item.color,
                                        item.count as i32,
                                    )
                                })
                            })
                            .collect();
                    }
                    DisplayMenuPayload::ShowPlayerSpells { pursuit_id } => {
                        menu_ctx.pursuit_id = Some(*pursuit_id);
                        entry_type = crate::webui::ipc::MenuEntryType::Spells;
                        entries = ability_state
                            .spells
                            .iter()
                            .map(|spell| {
                                MenuEntryUi::ability(
                                    spell.panel_name.clone(),
                                    spell.slot as i32,
                                    spell.sprite,
                                )
                            })
                            .collect();
                    }
                    DisplayMenuPayload::ShowPlayerSkills { pursuit_id } => {
                        menu_ctx.pursuit_id = Some(*pursuit_id);
                        entry_type = crate::webui::ipc::MenuEntryType::Skills;
                        entries = ability_state
                            .skills
                            .iter()
                            .map(|skill| {
                                MenuEntryUi::ability(
                                    skill.name.clone(),
                                    skill.slot as i32,
                                    skill.sprite,
                                )
                            })
                            .collect();
                    }
                }

                if is_text_entry {
                    outbound.write(UiOutbound(CoreToUi::DisplayMenuTextEntry {
                        title: pkt.header.name.clone(),
                        text: pkt.header.text.clone(),
                        prompt: pkt.header.text.clone(),
                        sprite_id: pkt.header.sprite,
                        args: menu_ctx.args.clone(),
                        entries,
                    }));
                } else {
                    outbound.write(UiOutbound(CoreToUi::DisplayMenu {
                        title: pkt.header.name.clone(),
                        text: pkt.header.text.clone(),
                        sprite_id: pkt.header.sprite,
                        entry_type,
                        entries,
                    }));
                }
            }
            SessionEvent::GroupInvite(pkt) => {
                // Server sent group invite (opcode 99); show invite popup.
                match pkt {
                    packets::server::DisplayGroupInvite::Invite {
                        source_name,
                        group_box_info,
                    } => {
                        group_state.pending_invite = Some(PendingGroupInvite {
                            source_name: source_name.clone(),
                            group_name: group_box_info.name.clone(),
                            group_note: group_box_info.note.clone(),
                        });
                    }
                    packets::server::DisplayGroupInvite::ShowGroupBox { source_name } => {
                        group_state.pending_invite = Some(PendingGroupInvite {
                            source_name: source_name.clone(),
                            group_name: String::new(),
                            group_note: String::new(),
                        });
                    }
                }
            }
            SessionEvent::DisplayExchange(pkt) => {
                tracing::info!("Received DisplayExchange: {:?}", pkt);
                match pkt {
                    packets::server::DisplayExchange::Start {
                        other_user_id,
                        other_user_name,
                    } => {
                        exchange_state.reset();
                        exchange_state.is_active = true;
                        exchange_state.other_player_id = *other_user_id;
                        exchange_state.other_player_name = other_user_name.clone();
                        popup_manager.open(crate::slint_support::popups::PopupId::Exchange);
                    }
                    packets::server::DisplayExchange::AddItem {
                        right_side,
                        exchange_index: _,
                        item_sprite,
                        item_color,
                        item_name,
                    } => {
                        let item = ExchangeSlotItem {
                            sprite: *item_sprite,
                            color: *item_color,
                            name: item_name.clone(),
                        };
                        let list = if *right_side {
                            &mut exchange_state.other_items
                        } else {
                            &mut exchange_state.my_items
                        };
                        // The wire index is not a reliable placement key; items append in arrival order.
                        list.push(item);
                        if *right_side {
                            exchange_state.other_accepted = false;
                        } else {
                            exchange_state.my_accepted = false;
                        }
                    }
                    packets::server::DisplayExchange::SetGold {
                        right_side,
                        gold_amount,
                    } => {
                        if *right_side {
                            exchange_state.other_gold = *gold_amount;
                            exchange_state.other_accepted = false;
                        } else {
                            exchange_state.my_gold = *gold_amount;
                            exchange_state.my_accepted = false;
                        }
                    }
                    packets::server::DisplayExchange::RequestAmount { from_slot } => {
                        if exchange_state.is_active {
                            exchange_state.quantity_prompt = Some(*from_slot);
                        }
                    }
                    packets::server::DisplayExchange::Accept {
                        right_side,
                        message,
                    } => {
                        if *right_side {
                            exchange_state.other_accepted = true;
                        } else {
                            exchange_state.my_accepted = true;
                        }

                        if exchange_state.my_accepted && exchange_state.other_accepted {
                            popup_manager.close(crate::slint_support::popups::PopupId::Exchange);
                            exchange_state.reset();
                            if !message.is_empty() {
                                chat_events.write(ChatEvent::ServerMessage(
                                    packets::server::ServerMessage {
                                        message_type:
                                            packets::server::ServerMessageType::ActiveMessage,
                                        message: message.clone(),
                                    },
                                ));
                            }
                        }
                    }
                    packets::server::DisplayExchange::Cancel {
                        right_side: _,
                        message,
                    } => {
                        popup_manager.close(crate::slint_support::popups::PopupId::Exchange);
                        exchange_state.reset();
                        if !message.is_empty() {
                            chat_events.write(ChatEvent::ServerMessage(
                                packets::server::ServerMessage {
                                    message_type: packets::server::ServerMessageType::ActiveMessage,
                                    message: message.clone(),
                                },
                            ));
                        }
                    }
                }
            }
            SessionEvent::NetworkDisconnected => {
                outbound.write(UiOutbound(CoreToUi::NetworkDisconnected));
            }
            _ => {}
        }
    }
}

// Bridge inventory GameEvents to UI CoreToUi messages
pub(crate) fn bridge_inventory_events(
    mut inventory_events: MessageReader<InventoryEvent>,
    mut inv_state: ResMut<InventoryState>,
    mut eq_state: ResMut<EquipmentState>,
    mut show_profile: MessageWriter<crate::slint_plugin::ShowSelfProfileEvent>,
) {
    let mut equipment_changed = false;
    for evt in inventory_events.read() {
        match evt {
            InventoryEvent::Add(pkt) => {
                let mut replaced = false;
                for item in inv_state.0.iter_mut() {
                    if item.slot == pkt.slot {
                        *item = InventoryItemUi {
                            id: ActionId::from_item(pkt.sprite, &pkt.name),
                            slot: pkt.slot,
                            name: pkt.name.clone(),
                            count: pkt.count,
                            sprite: pkt.sprite,
                            color: pkt.color,
                            stackable: pkt.stackable,
                            max_durability: pkt.max_durability,
                            current_durability: pkt.current_durability,
                        };
                        replaced = true;
                        break;
                    }
                }
                if !replaced {
                    inv_state.0.push(InventoryItemUi {
                        id: ActionId::from_item(pkt.sprite, &pkt.name),
                        slot: pkt.slot,
                        name: pkt.name.clone(),
                        count: pkt.count,
                        sprite: pkt.sprite,
                        color: pkt.color,
                        stackable: pkt.stackable,
                        max_durability: pkt.max_durability,
                        current_durability: pkt.current_durability,
                    });
                }
            }
            InventoryEvent::Remove(pkt) => {
                inv_state.0.retain(|i| i.slot != pkt.slot);
            }
            InventoryEvent::Equipment(pkt) => {
                eq_state.0.insert(pkt.slot, pkt.clone());
                equipment_changed = true;
            }
            InventoryEvent::DisplayUnequip(pkt) => {
                eq_state.0.remove(&pkt.equipment_slot);
                equipment_changed = true;
            }
            _ => {
                continue;
            }
        }
    }

    if equipment_changed {
        show_profile.write(crate::slint_plugin::ShowSelfProfileEvent::SelfUpdate);
    }
}

pub(crate) fn update_world_list_filtered(mut state: ResMut<WorldListState>, mut last_version: Local<u32>) {
    if state.version == *last_version {
        return;
    }

    *last_version = state.version;
    let Some(raw) = &state.raw else {
        return;
    };

    let filter = state.filter.clone();
    let search = filter.search.to_lowercase();

    state.filtered = raw
        .country_list
        .iter()
        .filter(|m| {
            if filter.master_only && !m.is_master {
                return false;
            }

            if let Some(class_filter) = &filter.class {
                let m_class = format!("{:?}", m.base_class);
                if !m_class.eq_ignore_ascii_case(class_filter) {
                    return false;
                }
            }

            if !search.is_empty() {
                if !m.name.to_lowercase().contains(&search)
                    && !m.title.to_lowercase().contains(&search)
                {
                    return false;
                }
            }

            true
        })
        .map(|m| WorldListMemberUi {
            name: m.name.clone(),
            title: m.title.clone(),
            class: format!("{:?}", m.base_class),
            is_master: m.is_master,
            color: match m.color {
                packets::server::WorldListColor::Guilded => [1.0, 0.75, 0.25, 1.0], // Gold-ish
                packets::server::WorldListColor::Unknown => [1.0, 0.596, 0.0, 1.0], // Orange
                packets::server::WorldListColor::WithinLevelRange => [0.6, 0.6, 1.0, 1.0], // Blue-ish
                packets::server::WorldListColor::White => [1.0, 1.0, 1.0, 1.0],
                packets::server::WorldListColor::NotSure => [0.5, 0.5, 0.5, 1.0],
            },
            social_status: m.social_status.into(),
        })
        .collect();
}

pub(crate) fn update_skill_cooldowns(
    time: Res<Time>,
    mut timer: Local<Timer>,
    mut cooldowns: ResMut<SkillCooldowns>,
) {
    if timer.duration().is_zero() {
        *timer = Timer::from_seconds(0.1, TimerMode::Repeating);
    }

    if !timer.tick(time.delta()).just_finished() {
        return;
    }

    if cooldowns.cooldowns.is_empty() {
        return;
    }

    let now = Instant::now();
    let mut expired = Vec::new();
    for (slot, cd) in cooldowns.cooldowns.iter_mut() {
        let time_left = cd
            .start_time
            .checked_add(cd.duration)
            .and_then(|end| end.checked_duration_since(now))
            .unwrap_or_default();

        if time_left.is_zero() {
            expired.push(*slot);
        } else {
            cd.time_left = time_left;
        }
    }
    for slot in expired {
        cooldowns.cooldowns.remove(&slot);
    }
}

// Bridge skill/spell GameEvents to UI
pub(crate) fn bridge_ability_events(
    mut ability_events: MessageReader<AbilityEvent>,
    mut state: ResMut<AbilityState>,
    mut cooldowns: ResMut<SkillCooldowns>,
) {
    for evt in ability_events.read() {
        match evt {
            AbilityEvent::SkillCooldown {
                slot,
                cooldown_secs,
            } => {
                let Some(skill) = state.skills.iter_mut().find(|s| s.slot == *slot) else {
                    continue;
                };

                if Some(*cooldown_secs) == skill.cooldown_secs {
                    continue;
                }

                skill.cooldown_secs = Some(*cooldown_secs);
                cooldowns
                    .cooldowns
                    .insert(*slot, Cooldown::new(*cooldown_secs));
            }
            AbilityEvent::UseSkill { slot } => {
                let Some(skill) = state.skills.iter().find(|s| s.slot == *slot) else {
                    continue;
                };

                let Some(cd) = skill.cooldown_secs else {
                    continue;
                };

                cooldowns.cooldowns.insert(*slot, Cooldown::new(cd));
            }
            AbilityEvent::AddSkill(pkt) => {
                let parsed = game_ui::parse_ability_name(&pkt.name);
                if let Some(existing) = state.skills.iter_mut().find(|s| s.slot == pkt.slot) {
                    existing.name = parsed.full_name.clone();
                    existing.sprite = pkt.sprite;

                    let new_id = ActionId::from_skill(pkt.sprite, parsed.chant_name());

                    if new_id != existing.id {
                        existing.id = new_id;
                        existing.cooldown_secs = None;
                        cooldowns.cooldowns.remove(&pkt.slot);
                    }
                } else {
                    state.skills.push(SkillUi {
                        slot: pkt.slot,
                        id: ActionId::from_skill(pkt.sprite, parsed.chant_name()),
                        name: parsed.full_name.clone(),
                        sprite: pkt.sprite,
                        cooldown_secs: None,
                    });
                }
            }
            AbilityEvent::RemoveSkill(pkt) => {
                state.skills.retain(|s| s.slot != pkt.slot);
                cooldowns.cooldowns.remove(&pkt.slot);
            }
            AbilityEvent::AddSpell(pkt) => {
                let parsed = game_ui::parse_ability_name(&pkt.panel_name);
                if let Some(existing) = state.spells.iter_mut().find(|s| s.slot == pkt.slot) {
                    existing.sprite = pkt.sprite;
                    existing.panel_name = parsed.full_name.clone();
                    existing.prompt = pkt.prompt.clone();
                    existing.cast_lines = pkt.cast_lines;
                    existing.id = ActionId::from_spell(pkt.sprite, parsed.chant_name());
                    existing.spell_type = pkt.spell_type;
                } else {
                    state.spells.push(SpellUi {
                        slot: pkt.slot,
                        sprite: pkt.sprite,
                        id: ActionId::from_spell(pkt.sprite, parsed.chant_name()),
                        panel_name: parsed.full_name.clone(),
                        prompt: pkt.prompt.clone(),
                        cast_lines: pkt.cast_lines,
                        spell_type: pkt.spell_type,
                    });
                }
            }
            AbilityEvent::RemoveSpell(pkt) => {
                state.spells.retain(|s| s.slot != pkt.slot);
            }
            AbilityEvent::UseSpell { .. } | AbilityEvent::UseSpellAt { .. } => {}
        }
    }
}