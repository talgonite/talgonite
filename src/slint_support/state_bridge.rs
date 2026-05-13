use crate::rich_text::RichText;
use crate::{
    resources::{PlayerPortraitState, RendererState, ZoomState},
    slint_support::assets::SlintAssetLoader,
};
use bevy::prelude::*;
use game_types::SlotPanelType;
use game_ui::{ActionId, LoginError};
use slint::Model;

fn macro_display_name(action_id: &str) -> String {
    let raw = action_id.get(6..).unwrap_or(action_id);
    let mut spaced = String::with_capacity(raw.len() + 4);
    let mut prev_was_lower_or_digit = false;

    for ch in raw.chars() {
        if ch == '_' || ch == '-' {
            if !spaced.ends_with(' ') {
                spaced.push(' ');
            }
            prev_was_lower_or_digit = false;
            continue;
        }

        if ch.is_ascii_uppercase() && prev_was_lower_or_digit && !spaced.ends_with(' ') {
            spaced.push(' ');
        }

        prev_was_lower_or_digit = ch.is_ascii_lowercase() || ch.is_ascii_digit();
        spaced.push(ch);
    }

    let mut titled = String::with_capacity(spaced.len());
    for (idx, word) in spaced.split_whitespace().enumerate() {
        if idx > 0 {
            titled.push(' ');
        }

        let mut chars = word.chars();
        if let Some(first) = chars.next() {
            titled.push(first.to_ascii_uppercase());
            for ch in chars {
                titled.push(ch.to_ascii_lowercase());
            }
        }
    }

    if titled.is_empty() {
        "Macro".to_string()
    } else {
        titled
    }
}

pub fn sync_portrait_to_slint(
    mut portrait: ResMut<PlayerPortraitState>,
    win: Res<SlintWindow>,
    mut last_version: Local<u32>,
    renderer: Res<RendererState>,
) {
    if portrait.version == *last_version {
        return;
    }

    let Some(strong) = win.0.upgrade() else {
        return;
    };
    let game_state = slint::ComponentHandle::global::<crate::GameState>(&strong);

    let portrait_size = 64;
    let next_texture = rendering::texture::Texture::create_render_texture(
        &renderer.device,
        "player_portrait",
        portrait_size,
        portrait_size,
        wgpu::TextureFormat::Rgba8Unorm,
    );

    let old_texture = std::mem::replace(&mut portrait.texture, next_texture.texture);
    portrait.view = next_texture.view;

    if let Ok(image) = old_texture.try_into() {
        game_state.set_player_portrait(image);
    }

    *last_version = portrait.version;
}

pub fn sync_lobby_portraits_to_slint(
    portraits: Res<crate::resources::LobbyPortraits>,
    win: Res<SlintWindow>,
    mut last_version: Local<u32>,
    settings: Res<crate::settings_types::Settings>,
) {
    if portraits.version == *last_version {
        return;
    }

    let Some(strong) = win.0.upgrade() else {
        return;
    };
    let lobby_state = slint::ComponentHandle::global::<crate::LobbyState>(&strong);

    let current_server_id = settings.gameplay.current_server_id;
    // Default to first server if none selected
    let effective_server_id = current_server_id.or_else(|| settings.servers.first().map(|s| s.id));

    // Update the saved logins with portraits
    let logins = &settings.saved_credentials;
    let mut li: Vec<crate::SavedLoginItem> = Vec::with_capacity(logins.len());
    for l in logins.iter() {
        // Filter by current server
        if effective_server_id
            .map(|id| id != l.server_id)
            .unwrap_or(false)
        {
            continue;
        }

        let preview = portraits
            .textures
            .get(&l.id)
            .and_then(|t| t.clone().try_into().ok())
            .unwrap_or_default();
        li.push(crate::SavedLoginItem {
            id: slint::SharedString::from(l.id.as_str()),
            server_id: l.server_id as i32,
            username: slint::SharedString::from(l.username.as_str()),
            last_used: l.last_used as i32,
            preview,
        });
    }

    let logins_model = slint::VecModel::<crate::SavedLoginItem>::default();
    for l in li {
        logins_model.push(l);
    }
    lobby_state.set_saved_logins(slint::ModelRc::new(logins_model));

    *last_version = portraits.version;
}

pub fn sync_character_creator_preview_to_slint(
    preview: Res<crate::resources::CharacterCreatorPreviewState>,
    win: Res<SlintWindow>,
    mut last_version: Local<u32>,
) {
    if preview.version == *last_version {
        return;
    }

    let Some(strong) = win.0.upgrade() else {
        return;
    };
    let state = slint::ComponentHandle::global::<game_ui::slint_types::CharacterCreationState>(&strong);

    if let Some(texture) = &preview.texture {
        if let Ok(image) = texture.clone().try_into() {
            state.set_preview(image);
        }
    }

    *last_version = preview.version;
}


fn parse_color_hex(hex: &str) -> slint::Brush {
    let hex = hex.trim_start_matches('#');
    let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(208);
    let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(208);
    let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(208);
    slint::Color::from_rgb_u8(r, g, b).into()
}

#[derive(Resource, Clone)]
pub struct SlintWindow(pub slint::Weak<crate::MainWindow>);

#[derive(Resource)]
pub struct SlintAssetLoaderRes(pub SlintAssetLoader);

pub fn show_prelogin_ui(win: Res<SlintWindow>) {
    let Some(strong) = win.0.upgrade() else {
        return;
    };
    reset_game_state_for_main_menu(&strong);
    strong.set_show_prelogin(true);
    let settings_state = slint::ComponentHandle::global::<crate::SettingsState>(&strong);
    settings_state.set_show_settings(false);
    strong.invoke_request_snapshot();
}

fn empty_model<T: Clone + 'static>() -> slint::ModelRc<T> {
    slint::ModelRc::new(slint::VecModel::from(Vec::<T>::new()))
}

fn responsive_mode_for(render_size: (u32, u32)) -> &'static str {
    let (width, height) = render_size;

    if width <= 1024 || height <= 640 {
        "compact"
    } else if width >= 1600 && height >= 900 {
        "wide"
    } else {
        "normal"
    }
}

fn sync_responsive_state(game_state: &crate::GameState, render_size: (u32, u32)) {
    let mode = responsive_mode_for(render_size);

    game_state.set_responsive_mode(slint::SharedString::from(mode));
    game_state.set_responsive_compact(mode == "compact");
    game_state.set_responsive_wide(mode == "wide");
}

fn reset_game_state_for_main_menu(window: &crate::MainWindow) {
    let game_state = slint::ComponentHandle::global::<crate::GameState>(window);

    game_state.set_map_name(slint::SharedString::from(""));
    game_state.set_player_x(0.0);
    game_state.set_player_y(0.0);
    game_state.set_current_hp(0);
    game_state.set_max_hp(0);
    game_state.set_current_mp(0);
    game_state.set_max_mp(0);
    game_state.set_player_id(-1);
    game_state.set_server_name(slint::SharedString::from(""));
    game_state.set_ping_ms(0);
    game_state.set_player_name(slint::SharedString::from(""));
    game_state.set_player_portrait(slint::Image::default());

    game_state.set_camera_x(0.0);
    game_state.set_camera_y(0.0);
    game_state.set_camera_zoom(1.0);
    game_state.set_viewport_width(0.0);
    game_state.set_viewport_height(0.0);
    game_state.set_display_scale(1.0);
    sync_responsive_state(&game_state, (0, 0));

    game_state.set_world_labels(empty_model());
    game_state.set_speech_bubbles(empty_model());
    game_state.set_chat_messages(empty_model());
    game_state.set_action_bar_messages(empty_model());
    game_state.set_action_bar_update_counter(0);
    game_state.set_last_whisper_target(slint::SharedString::from(""));

    game_state.set_world_map_nodes(empty_model());
    game_state.set_world_map_image(slint::Image::default());
    game_state.set_world_map_name(slint::SharedString::from(""));
    game_state.set_show_world_map(false);

    // Reset NPC dialog state
    slint::ComponentHandle::global::<crate::NpcDialogState>(window).invoke_reset();

    game_state.set_inventory(empty_model());
    game_state.set_skills(empty_model());
    game_state.set_spells(empty_model());
    game_state.set_hotbar(empty_model());
    game_state.set_show_inventory(false);
    game_state.set_show_skills(false);
    game_state.set_show_spells(false);
    game_state.set_hotbar_row_count(1);

    game_state.set_show_world_list(false);
    game_state.set_world_list_loading(false);
    game_state.set_world_list_members(empty_model());
    game_state.set_world_list_count(0);
    game_state.set_world_list_total_count(0);

    let mail_board = slint::ComponentHandle::global::<crate::MailBoardState>(window);
    mail_board.set_visible(false);
    mail_board.set_board_name(slint::SharedString::from(""));
    mail_board.set_selected_index(-1);
    mail_board.set_loading_post_id(-1);
    mail_board.set_posts(empty_model());

    let mut profile = crate::ProfileData::default();
    profile.visible = false;
    game_state.set_profile(profile);

    game_state.set_current_hotbar_panel(0);

    // Reset group state
    game_state.set_show_group(false);
    game_state.set_is_groupable(false);
    game_state.set_is_group_leader(false);
    game_state.set_group_members(empty_model());
    game_state.set_group_invite(crate::GroupInviteNotification::default());
}

pub fn apply_core_to_slint(
    mut reader: MessageReader<crate::webui::plugin::UiOutbound>,
    win: Res<SlintWindow>,
    asset_loader: Res<SlintAssetLoaderRes>,
    game_files: Res<crate::game_files::GameFiles>,
    metafile_store: Res<crate::metafile_store::MetafileStore>,
    inventory: Res<crate::webui::plugin::InventoryState>,
    ability: Res<crate::webui::plugin::AbilityState>,
    hotbar: Res<crate::ecs::hotbar::HotbarState>,
    hotbar_panel: Res<crate::ecs::hotbar::HotbarPanelState>,
    lobby_portraits: Res<crate::resources::LobbyPortraits>,
    world_list: Res<crate::webui::plugin::WorldListState>,
) {
    let Some(strong) = win.0.upgrade() else {
        return;
    };
    let asset_loader = &asset_loader.0;

    let mut hotbar_dirty = false;
    if inventory.is_changed() {
        let game_state = slint::ComponentHandle::global::<crate::GameState>(&strong);

        if game_state.get_inventory().row_count() != 60 {
            game_state.set_inventory(slint::ModelRc::new(slint::VecModel::from(
                vec![crate::InventoryItem::default(); 60],
            )));
        }

        let inventory_state = game_state.get_inventory();
        let mut slint_items: Vec<crate::InventoryItem> = (1..=60)
            .map(|i| crate::InventoryItem {
                slot: i,
                ..Default::default()
            })
            .collect();
        for item in &inventory.0 {
            let icon = asset_loader
                .load_item_icon(&game_files, item.sprite)
                .unwrap_or_default();
            slint_items[(item.slot - 1) as usize] = crate::InventoryItem {
                slot: item.slot as i32,
                name: slint::SharedString::from(item.name.as_str()),
                icon,
                quantity: item.count as i32,
            };
        }

        for (idx, item) in slint_items.into_iter().enumerate() {
            if let Some(m) = inventory_state.row_data(idx) {
                if m != item {
                    inventory_state.set_row_data(idx, item);
                    hotbar_dirty = true;
                }
            }
        }
    }

    if ability.is_changed() {
        let game_state = slint::ComponentHandle::global::<crate::GameState>(&strong);

        if game_state.get_skills().row_count() != 60 {
            game_state.set_skills(slint::ModelRc::new(slint::VecModel::from(
                vec![crate::Skill::default(); 60],
            )));
        }
        let skills_state = game_state.get_skills();
        let mut slint_skills = vec![crate::Skill::default(); 60];
        for s in &ability.skills {
            let icon = asset_loader
                .load_skill_icon(&game_files, s.sprite)
                .unwrap_or_default();
            let skill = crate::Skill {
                name: slint::SharedString::from(s.name.as_str()),
                icon,
                slot: s.slot as i32,
                cooldown: match &s.on_cooldown {
                    Some(cd) => crate::Cooldown {
                        time_left: cd.time_left.as_millis() as i64,
                        total: cd.duration.as_millis() as i64,
                    },
                    None => crate::Cooldown::default(),
                },
            };

            let index = s.slot.saturating_sub(1) as usize;
            if index < slint_skills.len() {
                slint_skills[index] = skill;
            }
        }

        for (idx, skill) in slint_skills.into_iter().enumerate() {
            if let Some(m) = skills_state.row_data(idx) {
                if m != skill {
                    skills_state.set_row_data(idx, skill);
                    hotbar_dirty = true;
                }
            }
        }

        // Update Spells
        if game_state.get_spells().row_count() != 60 {
            game_state.set_spells(slint::ModelRc::new(slint::VecModel::from(
                vec![crate::Spell::default(); 60],
            )));
        }
        let spells_state = game_state.get_spells();
        let mut slint_spells = vec![crate::Spell::default(); 60];
        for s in &ability.spells {
            let icon = asset_loader
                .load_spell_icon(&game_files, s.sprite)
                .unwrap_or_default();
            let spell = crate::Spell {
                name: slint::SharedString::from(s.panel_name.as_str()),
                icon,
                slot: s.slot as i32,
                prompt: slint::SharedString::from(s.prompt.as_str()),
            };

            let index = s.slot.saturating_sub(1) as usize;
            if index < slint_spells.len() {
                slint_spells[index] = spell;
            }
        }

        for (idx, spell) in slint_spells.into_iter().enumerate() {
            if let Some(m) = spells_state.row_data(idx) {
                if m != spell {
                    spells_state.set_row_data(idx, spell);
                    hotbar_dirty = true;
                }
            }
        }
    }

    if hotbar.is_changed() {
        hotbar_dirty = true;
    }

    for crate::webui::plugin::UiOutbound(payload) in reader.read() {
        match payload {
            crate::webui::ipc::CoreToUi::Snapshot {
                servers,
                current_server_id,
                logins,
                login_error,
            } => {
                let mut si: Vec<crate::ServerItem> = Vec::with_capacity(servers.len());
                let mut _selected_index: i32 = -1;
                for (idx, s) in servers.iter().enumerate() {
                    if current_server_id
                        .as_ref()
                        .map(|v| *v == s.id)
                        .unwrap_or(false)
                    {
                        _selected_index = idx as i32;
                    }
                    si.push(crate::ServerItem {
                        id: s.id as i32,
                        name: slint::SharedString::from(s.name.as_str()),
                        address: slint::SharedString::from(s.address.as_str()),
                    });
                }
                // Default to first server if none selected
                let effective_server_id =
                    current_server_id.or_else(|| servers.first().map(|s| s.id));

                let mut li: Vec<crate::SavedLoginItem> = Vec::with_capacity(logins.len());
                for l in logins.iter() {
                    // Filter by current server
                    if effective_server_id
                        .map(|id| id != l.server_id)
                        .unwrap_or(false)
                    {
                        continue;
                    }
                    let preview = lobby_portraits
                        .textures
                        .get(&l.id)
                        .and_then(|t| t.clone().try_into().ok())
                        .unwrap_or_default();
                    li.push(crate::SavedLoginItem {
                        id: slint::SharedString::from(l.id.as_str()),
                        server_id: l.server_id as i32,
                        username: slint::SharedString::from(l.username.as_str()),
                        last_used: l.last_used as i32,
                        preview,
                    });
                }
                let servers_model = slint::VecModel::<crate::ServerItem>::default();
                for s in si {
                    servers_model.push(s);
                }
                let lobby_state = slint::ComponentHandle::global::<crate::LobbyState>(&strong);
                lobby_state.set_servers(slint::ModelRc::new(servers_model));
                lobby_state
                    .set_current_server_id(effective_server_id.map(|id| id as i32).unwrap_or(-1));
                let current_server_name = servers
                    .iter()
                    .find(|s| effective_server_id.map(|id| id == s.id).unwrap_or(false))
                    .map(|s| s.name.as_str())
                    .unwrap_or("Unknown");
                lobby_state.set_current_server_name(slint::SharedString::from(current_server_name));
                let logins_model = slint::VecModel::<crate::SavedLoginItem>::default();
                for l in li {
                    logins_model.push(l);
                }
                lobby_state.set_saved_logins(slint::ModelRc::new(logins_model));
                let login_state = slint::ComponentHandle::global::<crate::LoginState>(&strong);
                login_state.set_login_error_code(login_error.clone().map_or(-1i32, |c| match c {
                    LoginError::Response(r) => r.code() as i32,
                    _ => 0i32,
                }));
                if login_error.is_some() {
                    login_state.set_is_submitting(false);
                    let char_state = slint::ComponentHandle::global::<game_ui::slint_types::CharacterCreationState>(&strong);
                    char_state.set_is_submitting(false);
                    let error_message = match login_error.as_ref() {
                        Some(LoginError::Network(message)) => message.clone(),
                        Some(LoginError::Response(packets::server::LoginMessageType::ClearNameMessage)) => {
                            "That name is unavailable".to_string()
                        }
                        Some(LoginError::Response(packets::server::LoginMessageType::NameExistsOrReserved)) => {
                            "That name already exists or contains a reserved string".to_string()
                        }
                        Some(LoginError::Response(packets::server::LoginMessageType::ClearPswdMessage)) => {
                            "That password is invalid".to_string()
                        }
                        Some(LoginError::Response(packets::server::LoginMessageType::CharacterDoesntExist)) => {
                            "Character does not exist".to_string()
                        }
                        Some(LoginError::Response(packets::server::LoginMessageType::WrongPassword)) => {
                            "Wrong password".to_string()
                        }
                        Some(LoginError::Response(packets::server::LoginMessageType::Confirm)) => {
                            "Creation failed".to_string()
                        }
                        Some(LoginError::Response(packets::server::LoginMessageType::Other(code))) => {
                            format!("Creation failed (code: {})", code)
                        }
                        Some(LoginError::Unknown) => "Creation failed".to_string(),
                        None => String::new(),
                    };
                    char_state.set_error_message(slint::SharedString::from(error_message));
                } else {
                    let char_state = slint::ComponentHandle::global::<game_ui::slint_types::CharacterCreationState>(&strong);
                    char_state.set_is_submitting(false);
                    char_state.set_error_message(slint::SharedString::from(""));
                    // Also close the character creation window if we succeeded
                    let login_state = slint::ComponentHandle::global::<crate::LoginState>(&strong);
                    login_state.set_show_character_creation(false);
                }
            }
            crate::webui::ipc::CoreToUi::EnteredGame => {
                let login_state = slint::ComponentHandle::global::<crate::LoginState>(&strong);
                login_state.set_login_error_code(-1);
                strong.set_show_prelogin(false);
                login_state.set_is_submitting(false);
            }
            crate::webui::ipc::CoreToUi::ChatAppend { entries } => {
                let game_state = slint::ComponentHandle::global::<crate::GameState>(&strong);

                let existing_chat = game_state.get_chat_messages();
                let mut chat_messages: Vec<crate::ChatMessage> = existing_chat.iter().collect();

                let existing_action = game_state.get_action_bar_messages();
                let mut action_bar_messages: Vec<slint::SharedString> =
                    existing_action.iter().collect();

                let mut action_bar_updated = false;
                for entry in entries.iter() {
                    if entry.show_in_message_box {
                        let color_str = entry
                            .color
                            .as_ref()
                            .map(|s| s.as_str())
                            .unwrap_or("#d0d0d0");
                        let color = parse_color_hex(color_str);

                        let rich_text = crate::rich_text::RichText::parse(entry.text.as_str());
                        chat_messages.push(crate::ChatMessage {
                            text: rich_text.to_slint_styled_text(),
                            color,
                        });
                    }

                    if entry.show_in_action_bar {
                        let rich_text = crate::rich_text::RichText::parse(entry.text.as_str());
                        action_bar_messages.push(slint::SharedString::from(
                            rich_text.to_plain_string().as_str(),
                        ));
                        while action_bar_messages.len() > 4 {
                            action_bar_messages.remove(0);
                        }
                        action_bar_updated = true;
                    }
                }

                let chat_model = std::rc::Rc::new(slint::VecModel::from(chat_messages));
                game_state.set_chat_messages(chat_model.clone().into());

                let action_model = std::rc::Rc::new(slint::VecModel::from(action_bar_messages));
                game_state.set_action_bar_messages(action_model.clone().into());

                if action_bar_updated {
                    let counter = game_state.get_action_bar_update_counter();
                    game_state.set_action_bar_update_counter(counter.wrapping_add(1));
                }
            }
            crate::webui::ipc::CoreToUi::WorldMapOpen { field_name, nodes } => {
                let game_state = slint::ComponentHandle::global::<crate::GameState>(&strong);

                if let Ok(img) = asset_loader.load_world_map_image(&game_files, &field_name) {
                    game_state.set_world_map_image(img);
                }
                game_state.set_world_map_name(slint::SharedString::from(field_name.as_str()));

                let mut slint_nodes = Vec::with_capacity(nodes.len());
                for n in nodes {
                    slint_nodes.push(crate::WorldMapNode {
                        text: slint::SharedString::from(n.text.as_str()),
                        map_id: n.map_id as i32,
                        x: n.x as i32,
                        y: n.y as i32,
                        dest_x: n.dest_x as i32,
                        dest_y: n.dest_y as i32,
                        check_sum: n.check_sum as i32,
                    });
                }
                let model = std::rc::Rc::new(slint::VecModel::from(slint_nodes));
                game_state.set_world_map_nodes(model.clone().into());
                game_state.set_show_world_map(true);
            }
            crate::webui::ipc::CoreToUi::DisplayMenu {
                title,
                text,
                sprite_id,
                entry_type,
                entries,
            } => {
                let npc_dialog = slint::ComponentHandle::global::<crate::NpcDialogState>(&strong);

                let npc_portrait = asset_loader
                    .load_npc_portrait(
                        &game_files,
                        &metafile_store,
                        *sprite_id,
                        Some(title.as_str()),
                    )
                    .unwrap_or_default();

                let mut slint_entries = Vec::with_capacity(entries.len());
                for entry in entries {
                    let mut icon = slint::Image::default();
                    let has_icon = entry.sprite > 0
                        && *entry_type != crate::webui::ipc::MenuEntryType::TextOptions;

                    if has_icon {
                        let result = match entry_type {
                            crate::webui::ipc::MenuEntryType::Items => {
                                asset_loader.load_item_icon(&game_files, entry.sprite)
                            }
                            crate::webui::ipc::MenuEntryType::Spells => {
                                asset_loader.load_spell_icon(&game_files, entry.sprite)
                            }
                            crate::webui::ipc::MenuEntryType::Skills => {
                                asset_loader.load_skill_icon(&game_files, entry.sprite)
                            }
                            _ => Ok(slint::Image::default()),
                        };
                        icon = result.unwrap_or_else(|e| {
                            tracing::warn!(
                                "Failed to load menu icon sprite {}: {}",
                                entry.sprite,
                                e
                            );
                            slint::Image::default()
                        });
                    }

                    slint_entries.push(crate::MenuEntry {
                        text: slint::SharedString::from(entry.text.as_str()),
                        id: entry.id as i32,
                        icon,
                        cost: entry.cost,
                    });
                }

                npc_dialog.set_data(crate::NpcDialogData {
                    visible: true,
                    text_entry_visible: false,
                    is_shop: *entry_type != crate::webui::ipc::MenuEntryType::TextOptions,
                    interaction_enabled: true,
                    npc_name: slint::SharedString::from(title.as_str()),
                    npc_portrait,
                    dialog_text: slint::SharedString::from(text.as_str()),
                    menu_entries: slint::ModelRc::new(slint::VecModel::from(slint_entries)),
                    text_entry_prompt: slint::SharedString::default(),
                    text_entry_args: slint::SharedString::default(),
                });
            }
            crate::webui::ipc::CoreToUi::ShowWorldContextMenu {
                title,
                x,
                y,
                anchor_width,
                anchor_height,
                entries,
            } => {
                let context_menu =
                    slint::ComponentHandle::global::<crate::ContextMenuState>(&strong);
                let slint_entries: Vec<crate::ContextMenuEntry> = entries
                    .iter()
                    .map(|entry| crate::ContextMenuEntry {
                        id: entry.id,
                        text: slint::SharedString::from(entry.text.as_str()),
                    })
                    .collect();

                context_menu.invoke_show(
                    slint::SharedString::from(title.as_str()),
                    slint::ModelRc::new(slint::VecModel::from(slint_entries)),
                    *x,
                    *y,
                    *anchor_width,
                    *anchor_height,
                );
            }
            crate::webui::ipc::CoreToUi::HideWorldContextMenu => {
                slint::ComponentHandle::global::<crate::ContextMenuState>(&strong).invoke_hide();
            }
            crate::webui::ipc::CoreToUi::DisplayMenuClose => {
                slint::ComponentHandle::global::<crate::NpcDialogState>(&strong).invoke_reset();
            }
            crate::webui::ipc::CoreToUi::DisplayMenuTextEntry {
                title,
                text,
                prompt,
                sprite_id,
                args,
                entries,
            } => {
                let npc_dialog = slint::ComponentHandle::global::<crate::NpcDialogState>(&strong);

                let npc_portrait = asset_loader
                    .load_npc_portrait(
                        &game_files,
                        &metafile_store,
                        *sprite_id,
                        Some(title.as_str()),
                    )
                    .unwrap_or_default();

                let mut slint_entries = Vec::with_capacity(entries.len());
                for entry in entries {
                    slint_entries.push(crate::MenuEntry {
                        text: slint::SharedString::from(entry.text.as_str()),
                        id: entry.id as i32,
                        icon: slint::Image::default(),
                        cost: entry.cost,
                    });
                }

                npc_dialog.set_data(crate::NpcDialogData {
                    visible: true,
                    text_entry_visible: true,
                    is_shop: false,
                    interaction_enabled: true,
                    npc_name: slint::SharedString::from(title.as_str()),
                    npc_portrait,
                    dialog_text: slint::SharedString::from(text.as_str()),
                    menu_entries: slint::ModelRc::new(slint::VecModel::from(slint_entries)),
                    text_entry_prompt: slint::SharedString::from(prompt.as_str()),
                    text_entry_args: slint::SharedString::from(args.as_str()),
                });
            }
            crate::webui::ipc::CoreToUi::DisplayBoard(board_state) => {
                let board = slint::ComponentHandle::global::<crate::MailBoardState>(&strong);

                if board_state.session_token != board.get_session_token() {
                    continue;
                }

                let mut slint_posts = Vec::with_capacity(board_state.posts.len());
                for post in &board_state.posts {
                    slint_posts.push(crate::MailBoardPost {
                        post_id: post.post_id,
                        author: slint::SharedString::from(post.author.as_str()),
                        month_of_year: post.month_of_year,
                        day_of_month: post.day_of_month,
                        title: slint::SharedString::from(post.title.as_str()),
                        message: RichText::parse(post.message.as_str()).to_slint_styled_text(),
                        is_unread: post.is_unread,
                        can_reply: post.can_reply,
                        can_delete: post.can_delete,
                    });
                }

                board.set_session_token(board_state.session_token);
                board.set_visible(board_state.visible);
                board.set_board_name(slint::SharedString::from(board_state.board_name.as_str()));

                if board_state.append {
                    let existing_posts = board.get_posts();
                    let mut combined_posts =
                        Vec::with_capacity(existing_posts.row_count() + slint_posts.len());
                    for index in 0..existing_posts.row_count() {
                        if let Some(post) = existing_posts.row_data(index) {
                            combined_posts.push(post);
                        }
                    }
                    combined_posts.extend(slint_posts);
                    board.set_posts(slint::ModelRc::new(slint::VecModel::from(combined_posts)));
                    board.set_loading_post_id(board_state.loading_post_id);
                    board.set_selected_index(board_state.selected_index);
                } else {
                    board.set_selected_index(-1);
                    board.set_posts(slint::ModelRc::new(slint::VecModel::from(slint_posts)));
                    board.set_loading_post_id(board_state.loading_post_id);
                    board.set_selected_index(board_state.selected_index);
                }
            }
            crate::webui::ipc::CoreToUi::SettingsSync {
                xray_size,
                sfx_volume,
                music_volume,
                scale,
                modifier_hotbar_rows_target_custom_only,
                key_bindings,
            } => {
                let settings_state =
                    slint::ComponentHandle::global::<crate::SettingsState>(&strong);
                macro_rules! set_keys {
                   ($field:ident) => {
                       paste::paste! {
                           settings_state.[<set_key_ $field>](slint::SharedString::from(key_bindings.$field[0].as_str()));
                           settings_state.[<set_key_ $field _2>](slint::SharedString::from(key_bindings.$field[1].as_str()));
                       }
                   };
                }

                settings_state.set_xray_size(*xray_size as i32);
                settings_state.set_sfx_volume(*sfx_volume);
                settings_state.set_music_volume(*music_volume);
                settings_state.set_scale(*scale);
                settings_state.set_modifier_hotbar_rows_target_custom_only(
                    *modifier_hotbar_rows_target_custom_only,
                );

                set_keys!(move_up);
                set_keys!(move_down);
                set_keys!(move_left);
                set_keys!(move_right);
                set_keys!(inventory);
                set_keys!(skills);
                set_keys!(spells);
                set_keys!(settings);
                set_keys!(refresh);
                set_keys!(basic_attack);
                set_keys!(auto_attack_toggle);
                set_keys!(hotbar_slot_1);
                set_keys!(hotbar_slot_2);
                set_keys!(hotbar_slot_3);
                set_keys!(hotbar_slot_4);
                set_keys!(hotbar_slot_5);
                set_keys!(hotbar_slot_6);
                set_keys!(hotbar_slot_7);
                set_keys!(hotbar_slot_8);
                set_keys!(hotbar_slot_9);
                set_keys!(hotbar_slot_10);
                set_keys!(hotbar_slot_11);
                set_keys!(hotbar_slot_12);
                set_keys!(hotbar_slot_13);
                set_keys!(hotbar_slot_14);
                set_keys!(hotbar_slot_15);
                set_keys!(hotbar_slot_16);
                set_keys!(hotbar_slot_17);
                set_keys!(hotbar_slot_18);
                set_keys!(hotbar_slot_19);
                set_keys!(hotbar_slot_20);
                set_keys!(hotbar_slot_21);
                set_keys!(hotbar_slot_22);
                set_keys!(hotbar_slot_23);
                set_keys!(hotbar_slot_24);
                set_keys!(hotbar_slot_25);
                set_keys!(hotbar_slot_26);
                set_keys!(hotbar_slot_27);
                set_keys!(hotbar_slot_28);
                set_keys!(hotbar_slot_29);
                set_keys!(hotbar_slot_30);
                set_keys!(hotbar_slot_31);
                set_keys!(hotbar_slot_32);
                set_keys!(hotbar_slot_33);
                set_keys!(hotbar_slot_34);
                set_keys!(hotbar_slot_35);
                set_keys!(hotbar_slot_36);
                set_keys!(hotbar_slot_37);
                set_keys!(hotbar_slot_38);
                set_keys!(hotbar_slot_39);
                set_keys!(hotbar_slot_40);
                set_keys!(hotbar_slot_41);
                set_keys!(hotbar_slot_42);
                set_keys!(hotbar_slot_43);
                set_keys!(hotbar_slot_44);
                set_keys!(hotbar_slot_45);
                set_keys!(hotbar_slot_46);
                set_keys!(hotbar_slot_47);
                set_keys!(hotbar_slot_48);
                set_keys!(switch_to_inventory);
                set_keys!(switch_to_skills);
                set_keys!(switch_to_spells);
                set_keys!(switch_to_hotbar_1);
                set_keys!(switch_to_hotbar_2);
                set_keys!(switch_to_hotbar_3);
            }
        }
    }

    if hotbar_dirty {
        let game_state = slint::ComponentHandle::global::<crate::GameState>(&strong);

        let expected_hotbar_entries = hotbar.config.bars.len() * 12;

        if game_state.get_hotbar().row_count() != expected_hotbar_entries {
            game_state.set_hotbar(slint::ModelRc::new(slint::VecModel::from(
                vec![crate::HotbarEntry::default(); expected_hotbar_entries],
            )));
        }

        let hotbar_state = game_state.get_hotbar();
        let mut entry_idx = 0;

        for bar in &hotbar.config.bars {
            for slot in bar {
                let entry = if slot.action_id.is_empty() {
                    crate::HotbarEntry::default()
                } else {
                    let action_id = ActionId::from_str(&slot.action_id);
                    let mut quantity = 0;
                    let mut enabled = false;
                    let mut sprite = action_id.sprite();
                    let mut cooldown = None;

                    let mut name = slint::SharedString::default();

                    match action_id.panel_type() {
                        SlotPanelType::Item => {
                            quantity = inventory
                                .0
                                .iter()
                                .filter(|item| item.id == action_id)
                                .map(|item| item.count)
                                .sum::<u32>();
                            enabled = quantity > 0;
                            if let Some(item) = inventory.0.iter().find(|item| item.id == action_id)
                            {
                                sprite = item.sprite;
                                name = slint::SharedString::from(item.name.as_str());
                            }
                            cooldown = hotbar.cooldowns.get(&slot.action_id).cloned();
                        }
                        SlotPanelType::Skill => {
                            if let Some(skill) = ability.skills.iter().find(|s| s.id == action_id) {
                                sprite = skill.sprite;
                                name = slint::SharedString::from(skill.name.as_str());
                                enabled = true;
                                cooldown = skill
                                    .on_cooldown
                                    .clone()
                                    .or_else(|| hotbar.cooldowns.get(&slot.action_id).cloned());
                            }
                        }
                        SlotPanelType::Spell => {
                            if let Some(spell) = ability.spells.iter().find(|s| s.id == action_id) {
                                sprite = spell.sprite;
                                name = slint::SharedString::from(spell.panel_name.as_str());
                                enabled = true;
                                cooldown = hotbar.cooldowns.get(&slot.action_id).cloned();
                            }
                        }
                        SlotPanelType::Macro => {
                            name =
                                slint::SharedString::from(macro_display_name(action_id.as_str()));
                            enabled = true;
                        }
                        _ => {}
                    }

                    let icon = match action_id.panel_type() {
                        SlotPanelType::Item => asset_loader.load_item_icon(&game_files, sprite),
                        SlotPanelType::Skill => asset_loader.load_skill_icon(&game_files, sprite),
                        SlotPanelType::Spell => asset_loader.load_spell_icon(&game_files, sprite),
                        SlotPanelType::Macro => asset_loader.load_skill_icon(&game_files, 104),
                        _ => Ok(slint::Image::default()),
                    }
                    .unwrap_or_default();

                    crate::HotbarEntry {
                        name,
                        icon,
                        quantity: quantity as i32,
                        enabled,
                        cooldown: match cooldown {
                            Some(cd) => crate::Cooldown {
                                time_left: cd.time_left.as_millis() as i64,
                                total: cd.duration.as_millis() as i64,
                            },
                            None => crate::Cooldown::default(),
                        },
                    }
                };

                if let Some(m) = hotbar_state.row_data(entry_idx) {
                    if m != entry {
                        hotbar_state.set_row_data(entry_idx, entry);
                    }
                }
                entry_idx += 1;
            }
        }
    }

    if hotbar_panel.is_changed() {
        let game_state = slint::ComponentHandle::global::<crate::GameState>(&strong);
        game_state.set_current_hotbar_panel(hotbar_panel.current_panel as i32);
        game_state.set_hotbar_row_count(hotbar_panel.rows.as_i32());
    }

    if world_list.is_changed() {
        let game_state = slint::ComponentHandle::global::<crate::GameState>(&strong);
        game_state.set_world_list_loading(false);

        let mut slint_members = Vec::with_capacity(world_list.filtered.len());
        for m in &world_list.filtered {
            slint_members.push(crate::WorldListMemberUi {
                name: slint::SharedString::from(m.name.as_str()),
                title: slint::SharedString::from(m.title.as_str()),
                class: slint::SharedString::from(m.class.as_str()),
                color: slint::Color::from_argb_f32(m.color[3], m.color[0], m.color[1], m.color[2]),
                is_master: m.is_master,
                social_status: u8_to_social_status(m.social_status),
            });
        }

        game_state
            .set_world_list_members(slint::ModelRc::new(slint::VecModel::from(slint_members)));
        game_state.set_world_list_count(world_list.filtered.len() as i32);
        if let Some(raw) = &world_list.raw {
            game_state.set_world_list_total_count(raw.world_member_count as i32);
        }
    }
}

#[derive(Resource, Clone)]
pub struct SlintUiChannels {
    pub tx: crossbeam_channel::Sender<crate::webui::ipc::UiToCore>,
    pub rx: crossbeam_channel::Receiver<crate::webui::ipc::UiToCore>,
}

impl Default for SlintUiChannels {
    fn default() -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        Self { tx, rx }
    }
}

pub fn drain_slint_inbound(
    ch: Res<SlintUiChannels>,
    mut writer: MessageWriter<crate::webui::plugin::UiInbound>,
) {
    while let Ok(msg) = ch.rx.try_recv() {
        writer.write(crate::webui::plugin::UiInbound(msg));
    }
}

/// Syncs world labels and camera state to Slint every frame.
/// This enables Slint to render entity names, speech bubbles, etc. in screen space.
pub fn sync_world_labels_to_slint(
    win: Res<SlintWindow>,
    camera: Res<crate::Camera>,
    zoom_state: Res<ZoomState>,
    player_attrs: Res<crate::resources::PlayerAttributes>,
    current_session: Res<crate::CurrentSession>,
    local_player_query: Query<
        (
            &crate::ecs::components::Player,
            &crate::ecs::components::EntityId,
        ),
        With<crate::ecs::components::LocalPlayer>,
    >,
    entities_query: Query<(
        Entity,
        &crate::ecs::components::Position,
        Option<&crate::ecs::components::HoverLabel>,
        Option<&crate::ecs::components::SpeechBubble>,
        Option<&crate::ecs::components::ChantLabel>,
        Option<&crate::ecs::components::HealthBar>,
    )>,
) {
    let Some(strong) = win.0.upgrade() else {
        return;
    };

    let game_state = slint::ComponentHandle::global::<crate::GameState>(&strong);

    // Update player attributes (HP/MP)
    game_state.set_current_hp(player_attrs.current_hp as i32);
    game_state.set_max_hp(player_attrs.max_hp as i32);
    game_state.set_current_mp(player_attrs.current_mp as i32);
    game_state.set_max_mp(player_attrs.max_mp as i32);

    // Update server name
    game_state.set_server_name(slint::SharedString::from(
        current_session.server_url.as_str(),
    ));

    // Update player name and ID
    if let Some((player, entity_id)) = local_player_query.iter().next() {
        game_state.set_player_name(slint::SharedString::from(player.name.as_str()));
        game_state.set_player_id(entity_id.id as i32);
    }

    // Update camera state
    let cam = &camera.camera.camera;
    game_state.set_camera_x(cam.position.x);
    game_state.set_camera_y(cam.position.y);
    game_state.set_camera_zoom(cam.zoom);
    game_state.set_viewport_width(zoom_state.render_size.0 as f32);
    game_state.set_viewport_height(zoom_state.render_size.1 as f32);

    game_state.set_display_scale(zoom_state.display_scale());
    sync_responsive_state(&game_state, zoom_state.render_size);

    // Collect all label types from all entities
    let mut slint_labels: Vec<crate::WorldLabel> = Vec::new();
    let mut slint_speech_bubbles: Vec<crate::SpeechBubble> = Vec::new();
    for (entity, pos, hover_label, speech_bubble, chant_label, health_bar) in entities_query.iter()
    {
        let world_pos = rendering::scene::get_isometric_coordinate(pos.x, pos.y);
        let hp = health_bar.map(|h| h.percent as i32).unwrap_or(-1);
        let mut hp_assigned = false;

        // Helper to push a label and assign HP once per entity
        let mut push_v_label = |label: crate::ecs::components::WorldLabel| {
            let mut final_hp = -1;
            if !hp_assigned && hp >= 0 {
                final_hp = hp;
                hp_assigned = true;
            }

            if label.is_speech {
                slint_speech_bubbles.push(crate::SpeechBubble {
                    entity_id: entity.index().index() as i32,
                    text: crate::rich_text::RichText::parse(label.text.as_str())
                        .to_slint_styled_text(),
                    world_x: world_pos.x,
                    world_y: world_pos.y,
                    y_offset: label.y_offset,
                });
            } else {
                slint_labels.push(crate::WorldLabel {
                    entity_id: entity.index().index() as i32,
                    text: slint::SharedString::from(label.text.as_str()),
                    world_x: world_pos.x,
                    world_y: world_pos.y,
                    y_offset: label.y_offset,
                    color: slint::Color::from_argb_f32(
                        label.color.w,
                        label.color.x,
                        label.color.y,
                        label.color.z,
                    )
                    .into(),
                    health_percent: final_hp,
                });
            }
        };

        if let Some(hover) = hover_label {
            push_v_label(hover.to_world_label());
        }

        if let Some(bubble) = speech_bubble {
            push_v_label(bubble.to_world_label());
        }

        if let Some(chant) = chant_label {
            push_v_label(chant.to_world_label());
        }

        if !hp_assigned && hp >= 0 {
            slint_labels.push(crate::WorldLabel {
                entity_id: entity.index().index() as i32,
                text: Default::default(),
                world_x: world_pos.x,
                world_y: world_pos.y,
                y_offset: -40.0,
                color: slint::Color::from_argb_f32(1.0, 1.0, 1.0, 1.0).into(),
                health_percent: hp,
            });
        }
    }

    let model = slint::VecModel::from(slint_labels);
    game_state.set_world_labels(slint::ModelRc::new(model));
    let bubble_model = slint::VecModel::from(slint_speech_bubbles);
    game_state.set_speech_bubbles(slint::ModelRc::new(bubble_model));
}

pub fn sync_map_name_to_slint(
    win: Res<SlintWindow>,
    map_query: Query<&crate::ecs::components::GameMap, Changed<crate::ecs::components::GameMap>>,
) {
    let Some(strong) = win.0.upgrade() else {
        return;
    };

    if let Some(map) = map_query.iter().next() {
        let game_state = slint::ComponentHandle::global::<crate::GameState>(&strong);
        game_state.set_map_name(slint::SharedString::from(map.name.as_str()));
    }
}

// ---------------------------------------------------------------------------
// Group state → Slint
// ---------------------------------------------------------------------------

/// Sync GroupState to Slint: local player first (position 1 = Leave), and whether we are the server-designated leader (can Kick).
pub fn sync_group_to_slint(
    group_state: Res<crate::webui::plugin::GroupState>,
    local_player_query: Query<
        &crate::ecs::components::Player,
        With<crate::ecs::components::LocalPlayer>,
    >,
    win: Res<SlintWindow>,
) {
    if !group_state.is_changed() {
        return;
    }
    let Some(strong) = win.0.upgrade() else {
        return;
    };
    let game_state = slint::ComponentHandle::global::<crate::GameState>(&strong);

    game_state.set_is_groupable(group_state.is_groupable);

    let local_name = local_player_query.iter().next().map(|p| p.name.as_str());
    let mut ordered: Vec<(String, bool)> = group_state.members.clone();
    if let Some(local) = local_name {
        if let Some(idx) = ordered.iter().position(|(name, _)| name == local) {
            let entry = ordered.remove(idx);
            ordered.insert(0, entry);
        }
    }

    let is_group_leader = local_name
        .and_then(|local| {
            group_state
                .members
                .iter()
                .find(|(_, is_leader)| *is_leader)
                .filter(|(name, _)| name == local)
                .map(|_| ())
        })
        .is_some();
    game_state.set_is_group_leader(is_group_leader);

    let members: Vec<crate::GroupMember> = ordered
        .iter()
        .enumerate()
        .map(|(i, (name, _))| crate::GroupMember {
            name: slint::SharedString::from(name.as_str()),
            is_leader: i == 0,
        })
        .collect();
    game_state.set_group_members(slint::ModelRc::new(slint::VecModel::from(members)));

    if let Some(invite) = &group_state.pending_invite {
        game_state.set_group_invite(crate::GroupInviteNotification {
            visible: true,
            source_name: slint::SharedString::from(invite.source_name.as_str()),
            group_name: slint::SharedString::from(invite.group_name.as_str()),
            group_note: slint::SharedString::from(invite.group_note.as_str()),
        });
    } else {
        let mut gi = game_state.get_group_invite();
        if gi.visible {
            gi.visible = false;
            game_state.set_group_invite(gi);
        }
    }
}

pub fn sync_settings_to_slint(
    win: Res<SlintWindow>,
    settings: Res<crate::settings_types::Settings>,
) {
    if !settings.is_changed() {
        return;
    }

    let Some(strong) = win.0.upgrade() else {
        return;
    };

    let settings_state = slint::ComponentHandle::global::<crate::SettingsState>(&strong);

    // Sync settings to slint
    settings_state.set_music_volume(settings.audio.music_volume);
    settings_state.set_sfx_volume(settings.audio.sfx_volume);
    settings_state.set_scale(settings.graphics.scale);
}

pub fn show_installer_ui(win: Res<SlintWindow>) {
    let Some(strong) = win.0.upgrade() else {
        return;
    };
    let installer_state = slint::ComponentHandle::global::<crate::InstallerState>(&strong);
    installer_state.set_is_installing(true);
}

pub fn hide_installer_ui(win: Res<SlintWindow>) {
    let Some(strong) = win.0.upgrade() else {
        return;
    };
    let installer_state = slint::ComponentHandle::global::<crate::InstallerState>(&strong);
    installer_state.set_is_installing(false);
}

pub fn sync_installer_to_slint(
    mut events: MessageReader<crate::plugins::installer::InstallerProgressEvent>,
    win: Res<SlintWindow>,
) {
    let Some(strong) = win.0.upgrade() else {
        return;
    };

    let installer_state = slint::ComponentHandle::global::<crate::InstallerState>(&strong);
    for evt in events.read() {
        installer_state.set_progress(evt.percent);
        if let Some(msg) = &evt.message {
            installer_state.set_message(slint::SharedString::from(msg.as_str()));
        }
    }
}

pub fn sync_social_status_to_slint(
    local_status: Res<crate::ecs::social_status::LocalSocialStatus>,
    win: Res<SlintWindow>,
    mut last_version: Local<u32>,
) {
    if local_status.version == *last_version {
        return;
    }

    let Some(strong) = win.0.upgrade() else {
        return;
    };
    let social_status_state = slint::ComponentHandle::global::<crate::SocialStatusState>(&strong);
    social_status_state.set_current_status(u8_to_social_status(local_status.status as u8));

    *last_version = local_status.version;
}

/// Convert u8 from network protocol to SocialStatus enum
pub fn u8_to_social_status(value: u8) -> crate::SocialStatus {
    match value {
        0 => crate::SocialStatus::Online,
        1 => crate::SocialStatus::DoNotDisturb,
        2 => crate::SocialStatus::DayDreaming,
        3 => crate::SocialStatus::NeedGroup,
        4 => crate::SocialStatus::Grouped,
        5 => crate::SocialStatus::LoneHunter,
        6 => crate::SocialStatus::GroupHunting,
        7 => crate::SocialStatus::NeedHelp,
        _ => crate::SocialStatus::Online,
    }
}
