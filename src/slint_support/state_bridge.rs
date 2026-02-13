use crate::{
    resources::{PlayerPortraitState, RendererState, ZoomState},
    slint_support::assets::SlintAssetLoader,
};
use bevy::prelude::*;
use game_types::SlotPanelType;
use game_ui::{ActionId, LoginError};
use game_ui::slint_types::GoldDropState;
use slint::Model;

const GOLD_SPRITE_ID: u16 = 140;

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

fn reset_game_state_for_main_menu(window: &crate::MainWindow) {
    let game_state = slint::ComponentHandle::global::<crate::GameState>(window);

    game_state.set_map_name(slint::SharedString::from(""));
    game_state.set_player_x(0.0);
    game_state.set_player_y(0.0);
    game_state.set_current_hp(0);
    game_state.set_max_hp(0);
    game_state.set_current_mp(0);
    game_state.set_max_mp(0);
    game_state.set_player_gold(0);
    game_state.set_player_id(-1);
    game_state.set_server_name(slint::SharedString::from(""));
    game_state.set_ping_ms(0);
    game_state.set_player_name(slint::SharedString::from(""));
    game_state.set_player_portrait(slint::Image::default());
    game_state.set_gold_icon(slint::Image::default());

    game_state.set_camera_x(0.0);
    game_state.set_camera_y(0.0);
    game_state.set_camera_zoom(1.0);
    game_state.set_viewport_width(0.0);
    game_state.set_viewport_height(0.0);
    game_state.set_display_scale(1.0);

    game_state.set_world_labels(empty_model());
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

    let gold_drop = slint::ComponentHandle::global::<GoldDropState>(window);
    gold_drop.set_visible(false);
    gold_drop.set_max_gold(0);
    gold_drop.set_error_text(slint::SharedString::from(""));

    game_state.set_inventory(empty_model());
    game_state.set_skills(empty_model());
    game_state.set_spells(empty_model());
    game_state.set_hotbar(empty_model());
    game_state.set_show_inventory(false);
    game_state.set_show_skills(false);
    game_state.set_show_spells(false);

    game_state.set_show_world_list(false);
    game_state.set_world_list_loading(false);
    game_state.set_world_list_members(empty_model());
    game_state.set_world_list_count(0);
    game_state.set_world_list_total_count(0);

    let mut profile = crate::ProfileData::default();
    profile.visible = false;
    game_state.set_profile(profile);

    game_state.set_current_hotbar_panel(0);
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
    mut gold_icon_loaded: Local<bool>,
) {
    let Some(strong) = win.0.upgrade() else {
        return;
    };
    let asset_loader = &asset_loader.0;
    let game_state = slint::ComponentHandle::global::<crate::GameState>(&strong);

    let player_id = game_state.get_player_id();
    if player_id < 0 {
        *gold_icon_loaded = false;
    }

    if !*gold_icon_loaded && player_id >= 0 {
        let icon = asset_loader
            .load_item_icon(&game_files, GOLD_SPRITE_ID)
            .unwrap_or_default();
        game_state.set_gold_icon(icon);
        *gold_icon_loaded = true;
    }

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

        if game_state.get_skills().row_count() != ability.skills.len() {
            game_state.set_skills(slint::ModelRc::new(slint::VecModel::from(
                vec![crate::Skill::default(); ability.skills.len()],
            )));
        }
        let skills_state = game_state.get_skills();
        let mut si = 0;
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

            if let Some(m) = skills_state.row_data(si) {
                if m != skill {
                    skills_state.set_row_data(si, skill);
                    hotbar_dirty = true;
                }
            }
            si += 1;
        }

        // Update Spells
        if game_state.get_spells().row_count() != ability.spells.len() {
            game_state.set_spells(slint::ModelRc::new(slint::VecModel::from(
                vec![crate::Spell::default(); ability.spells.len()],
            )));
        }
        let spells_state = game_state.get_spells();
        let mut spi = 0;
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

            if let Some(m) = spells_state.row_data(spi) {
                if m != spell {
                    spells_state.set_row_data(spi, spell);
                    hotbar_dirty = true;
                }
            }
            spi += 1;
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
                    LoginError::Response(r) => r as i32,
                    _ => 0i32,
                }));
                if login_error.is_some() {
                    login_state.set_is_submitting(false);
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

                        chat_messages.push(crate::ChatMessage {
                            text: slint::SharedString::from(entry.text.as_str()),
                            color,
                        });
                    }

                    if entry.show_in_action_bar {
                        action_bar_messages.push(slint::SharedString::from(entry.text.as_str()));
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
            crate::webui::ipc::CoreToUi::GoldDropPrompt { max_gold, error } => {
                let gold_drop = slint::ComponentHandle::global::<GoldDropState>(&strong);
                gold_drop.set_max_gold(*max_gold as i32);
                gold_drop.set_error_text(slint::SharedString::from(
                    error.as_deref().unwrap_or(""),
                ));
                gold_drop.set_visible(true);
            }
            crate::webui::ipc::CoreToUi::GoldDropClose => {
                let gold_drop = slint::ComponentHandle::global::<GoldDropState>(&strong);
                gold_drop.set_visible(false);
                gold_drop.set_error_text(slint::SharedString::from(""));
            }
            crate::webui::ipc::CoreToUi::SettingsSync {
                xray_size,
                sfx_volume,
                music_volume,
                scale,
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

        if game_state.get_hotbar().row_count() != 36 {
            game_state.set_hotbar(slint::ModelRc::new(slint::VecModel::from(
                vec![crate::HotbarEntry::default(); 36],
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
                        _ => {}
                    }

                    let icon = match action_id.panel_type() {
                        SlotPanelType::Item => asset_loader.load_item_icon(&game_files, sprite),
                        SlotPanelType::Skill => asset_loader.load_skill_icon(&game_files, sprite),
                        SlotPanelType::Spell => asset_loader.load_spell_icon(&game_files, sprite),
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
    game_state.set_player_gold(player_attrs.gold as i32);

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

    // Collect all label types from all entities
    let mut slint_labels: Vec<crate::WorldLabel> = Vec::new();
    for (entity, pos, hover_label, speech_bubble, chant_label, health_bar) in entities_query.iter()
    {
        let world_pos = rendering::scene::get_isometric_coordinate(pos.x, pos.y);
        let hp = health_bar.map(|h| h.percent as i32).unwrap_or(-1);
        let mut hp_assigned = false;

        // Helper to push a label and assign HP once per entity
        let mut push_v_label =
            |label: crate::ecs::components::WorldLabel,
             slint_labels: &mut Vec<crate::WorldLabel>| {
                let mut final_hp = -1;
                if !hp_assigned && hp >= 0 {
                    final_hp = hp;
                    hp_assigned = true;
                }

                slint_labels.push(crate::WorldLabel {
                    entity_id: entity.index().index() as i32,
                    text: slint::SharedString::from(label.text.as_str()),
                    world_x: world_pos.x,
                    world_y: world_pos.y,
                    y_offset: label.y_offset,
                    color_r: label.color.x,
                    color_g: label.color.y,
                    color_b: label.color.z,
                    color_a: label.color.w,
                    is_speech: label.is_speech,
                    health_percent: final_hp,
                });
            };

        if let Some(hover) = hover_label {
            push_v_label(hover.to_world_label(), &mut slint_labels);
        }

        if let Some(bubble) = speech_bubble {
            push_v_label(bubble.to_world_label(), &mut slint_labels);
        }

        if let Some(chant) = chant_label {
            push_v_label(chant.to_world_label(), &mut slint_labels);
        }

        if !hp_assigned && hp >= 0 {
            slint_labels.push(crate::WorldLabel {
                entity_id: entity.index().index() as i32,
                text: slint::SharedString::default(),
                world_x: world_pos.x,
                world_y: world_pos.y,
                y_offset: -40.0,
                color_r: 1.0,
                color_g: 1.0,
                color_b: 1.0,
                color_a: 1.0,
                is_speech: false,
                health_percent: hp,
            });
        }
    }

    let model = slint::VecModel::from(slint_labels);
    game_state.set_world_labels(slint::ModelRc::new(model));
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

        // Auto-show/hide based on progress
        if evt.percent < 1.0 {
            installer_state.set_is_installing(true);
        } else {
            // Give a little time to see the 100% or just hide it
            installer_state.set_is_installing(false);
        }
    }
}
