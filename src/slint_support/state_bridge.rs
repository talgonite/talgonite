use crate::{
    resources::{PlayerPortraitState, RendererState, ZoomState},
    slint_support::assets::{
        load_item_icon, load_skill_icon, load_spell_icon, load_world_map_image,
    },
};
use bevy::prelude::*;
use game_types::SlotPanelType;
use game_ui::{ActionId, LoginError};
use slint::Model;

pub fn sync_portrait_to_slint(
    portrait: Option<ResMut<PlayerPortraitState>>,
    win: Option<Res<SlintWindow>>,
    mut last_version: Local<u32>,
    renderer: Option<Res<RendererState>>,
) {
    let (Some(mut portrait), Some(win), Some(renderer)) = (portrait, win, renderer) else {
        return;
    };
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
    portraits: Option<Res<crate::resources::LobbyPortraits>>,
    win: Option<Res<SlintWindow>>,
    mut last_version: Local<u32>,
    settings: Option<Res<crate::settings_types::Settings>>,
) {
    let (Some(portraits), Some(win), Some(settings)) = (portraits, win, settings) else {
        return;
    };
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

pub fn apply_core_to_slint(
    mut reader: MessageReader<crate::webui::plugin::UiOutbound>,
    win: Option<Res<SlintWindow>>,
    game_files: Option<Res<crate::game_files::GameFiles>>,
    inventory: Option<Res<crate::webui::plugin::InventoryState>>,
    ability: Option<Res<crate::webui::plugin::AbilityState>>,
    hotbar: Option<Res<crate::ecs::hotbar::HotbarState>>,
    hotbar_panel: Option<Res<crate::ecs::hotbar::HotbarPanelState>>,
    lobby_portraits: Option<Res<crate::resources::LobbyPortraits>>,
    world_list: Option<ResMut<crate::webui::plugin::WorldListState>>,
) {
    let Some(win) = win else {
        return;
    };
    let Some(strong) = win.0.upgrade() else {
        return;
    };

    let mut hotbar_dirty = false;
    if let Some(i) = &inventory {
        if i.is_changed() {
            if let Some(game_files) = &game_files {
                let game_state = slint::ComponentHandle::global::<crate::GameState>(&strong);

                if game_state.get_inventory().row_count() != 60 {
                    game_state.set_inventory(slint::ModelRc::new(slint::VecModel::from(
                        vec![crate::InventoryItem::default(); 60],
                    )));
                }

                let inventory_state = game_state.get_inventory();
                let mut slint_items = vec![crate::InventoryItem::default(); 60];
                for item in &i.0 {
                    let icon = load_item_icon(game_files, item.sprite).unwrap_or_default();
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
        }
    }

    if let Some(a) = &ability {
        if a.is_changed() {
            if let Some(game_files) = &game_files {
                let game_state = slint::ComponentHandle::global::<crate::GameState>(&strong);

                if game_state.get_skills().row_count() != a.skills.len() {
                    game_state.set_skills(slint::ModelRc::new(slint::VecModel::from(
                        vec![crate::Skill::default(); a.skills.len()],
                    )));
                }
                let skills_state = game_state.get_skills();
                let mut si = 0;
                for s in &a.skills {
                    let icon = load_skill_icon(game_files, s.sprite).unwrap_or_default();
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
                if game_state.get_spells().row_count() != a.spells.len() {
                    game_state.set_spells(slint::ModelRc::new(slint::VecModel::from(
                        vec![crate::Spell::default(); a.spells.len()],
                    )));
                }
                let spells_state = game_state.get_spells();
                let mut spi = 0;
                for s in &a.spells {
                    let icon = load_spell_icon(game_files, s.sprite).unwrap_or_default();
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
        }
    }

    if let Some(h) = &hotbar {
        if h.is_changed() {
            hotbar_dirty = true;
        }
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
                        .as_ref()
                        .and_then(|lp| {
                            lp.textures
                                .get(&l.id)
                                .and_then(|t| t.clone().try_into().ok())
                        })
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

                if let Some(ref game_files) = game_files {
                    if let Ok(img) = load_world_map_image(game_files, &field_name) {
                        game_state.set_world_map_image(img);
                    }
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
                entry_type,
                entries,
                ..
            } => {
                let game_state = slint::ComponentHandle::global::<crate::GameState>(&strong);
                game_state.set_menu_title(slint::SharedString::from(title.as_str()));
                game_state.set_menu_text(slint::SharedString::from(text.as_str()));

                let mut slint_entries = Vec::with_capacity(entries.len());
                for entry in entries {
                    let mut icon = slint::Image::default();
                    let has_icon = entry.sprite > 0
                        && *entry_type != crate::webui::ipc::MenuEntryType::TextOptions;

                    if has_icon {
                        if let Some(ref gf) = game_files {
                            let result = match entry_type {
                                crate::webui::ipc::MenuEntryType::Items => {
                                    load_item_icon(gf, entry.sprite)
                                }
                                crate::webui::ipc::MenuEntryType::Spells => {
                                    load_spell_icon(gf, entry.sprite)
                                }
                                crate::webui::ipc::MenuEntryType::Skills => {
                                    load_skill_icon(gf, entry.sprite)
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
                    }

                    slint_entries.push(crate::MenuEntry {
                        text: slint::SharedString::from(entry.text.as_str()),
                        id: entry.id as i32,
                        icon,
                        has_icon,
                        cost: entry.cost,
                    });
                }

                game_state
                    .set_menu_entries(slint::ModelRc::new(slint::VecModel::from(slint_entries)));
                game_state
                    .set_menu_is_shop(*entry_type != crate::webui::ipc::MenuEntryType::TextOptions);
                game_state.set_show_menu(true);
            }
            crate::webui::ipc::CoreToUi::DisplayMenuTextEntry {
                title,
                text,
                args,
                pursuit_id,
            } => {
                let game_state = slint::ComponentHandle::global::<crate::GameState>(&strong);
                game_state.set_menu_title(slint::SharedString::from(title.as_str()));
                game_state.set_menu_text(slint::SharedString::from(text.as_str()));
                game_state.set_text_entry_args(slint::SharedString::from(args.as_str()));
                game_state.set_text_entry_pursuit_id(*pursuit_id as i32);
                game_state.set_show_text_entry(true);
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
                settings_state.set_xray_size(*xray_size as i32);
                settings_state.set_sfx_volume(*sfx_volume);
                settings_state.set_music_volume(*music_volume);
                settings_state.set_scale(*scale);
                settings_state
                    .set_key_move_up(slint::SharedString::from(key_bindings.move_up.as_str()));
                settings_state
                    .set_key_move_down(slint::SharedString::from(key_bindings.move_down.as_str()));
                settings_state
                    .set_key_move_left(slint::SharedString::from(key_bindings.move_left.as_str()));
                settings_state.set_key_move_right(slint::SharedString::from(
                    key_bindings.move_right.as_str(),
                ));
                settings_state
                    .set_key_inventory(slint::SharedString::from(key_bindings.inventory.as_str()));
                settings_state
                    .set_key_skills(slint::SharedString::from(key_bindings.skills.as_str()));
                settings_state
                    .set_key_spells(slint::SharedString::from(key_bindings.spells.as_str()));
                settings_state
                    .set_key_settings(slint::SharedString::from(key_bindings.settings.as_str()));
                settings_state
                    .set_key_refresh(slint::SharedString::from(key_bindings.refresh.as_str()));
                settings_state.set_key_basic_attack(slint::SharedString::from(
                    key_bindings.basic_attack.as_str(),
                ));
                settings_state.set_key_hotbar_slot_1(slint::SharedString::from(
                    key_bindings.hotbar_slot_1.as_str(),
                ));
                settings_state.set_key_hotbar_slot_2(slint::SharedString::from(
                    key_bindings.hotbar_slot_2.as_str(),
                ));
                settings_state.set_key_hotbar_slot_3(slint::SharedString::from(
                    key_bindings.hotbar_slot_3.as_str(),
                ));
                settings_state.set_key_hotbar_slot_4(slint::SharedString::from(
                    key_bindings.hotbar_slot_4.as_str(),
                ));
                settings_state.set_key_hotbar_slot_5(slint::SharedString::from(
                    key_bindings.hotbar_slot_5.as_str(),
                ));
                settings_state.set_key_hotbar_slot_6(slint::SharedString::from(
                    key_bindings.hotbar_slot_6.as_str(),
                ));
                settings_state.set_key_hotbar_slot_7(slint::SharedString::from(
                    key_bindings.hotbar_slot_7.as_str(),
                ));
                settings_state.set_key_hotbar_slot_8(slint::SharedString::from(
                    key_bindings.hotbar_slot_8.as_str(),
                ));
                settings_state.set_key_hotbar_slot_9(slint::SharedString::from(
                    key_bindings.hotbar_slot_9.as_str(),
                ));
                settings_state.set_key_hotbar_slot_10(slint::SharedString::from(
                    key_bindings.hotbar_slot_10.as_str(),
                ));
                settings_state.set_key_hotbar_slot_11(slint::SharedString::from(
                    key_bindings.hotbar_slot_11.as_str(),
                ));
                settings_state.set_key_hotbar_slot_12(slint::SharedString::from(
                    key_bindings.hotbar_slot_12.as_str(),
                ));
                settings_state.set_key_switch_to_inventory(slint::SharedString::from(
                    key_bindings.switch_to_inventory.as_str(),
                ));
                settings_state.set_key_switch_to_skills(slint::SharedString::from(
                    key_bindings.switch_to_skills.as_str(),
                ));
                settings_state.set_key_switch_to_spells(slint::SharedString::from(
                    key_bindings.switch_to_spells.as_str(),
                ));
                settings_state.set_key_switch_to_hotbar_1(slint::SharedString::from(
                    key_bindings.switch_to_hotbar_1.as_str(),
                ));
                settings_state.set_key_switch_to_hotbar_2(slint::SharedString::from(
                    key_bindings.switch_to_hotbar_2.as_str(),
                ));
                settings_state.set_key_switch_to_hotbar_3(slint::SharedString::from(
                    key_bindings.switch_to_hotbar_3.as_str(),
                ));
            }
        }
    }

    if hotbar_dirty {
        if let (Some(h), Some(i), Some(a)) = (hotbar, inventory, ability) {
            let game_state = slint::ComponentHandle::global::<crate::GameState>(&strong);

            if game_state.get_hotbar().row_count() != 36 {
                game_state.set_hotbar(slint::ModelRc::new(slint::VecModel::from(
                    vec![crate::HotbarEntry::default(); 36],
                )));
            }

            let hotbar_state = game_state.get_hotbar();
            let mut entry_idx = 0;

            for bar in &h.config.bars {
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
                                quantity =
                                    i.0.iter()
                                        .filter(|item| item.id == action_id)
                                        .map(|item| item.count)
                                        .sum::<u32>();
                                enabled = quantity > 0;
                                if let Some(item) = i.0.iter().find(|item| item.id == action_id) {
                                    sprite = item.sprite;
                                    name = slint::SharedString::from(item.name.as_str());
                                }
                                cooldown = h.cooldowns.get(&slot.action_id).cloned();
                            }
                            SlotPanelType::Skill => {
                                if let Some(skill) = a.skills.iter().find(|s| s.id == action_id) {
                                    sprite = skill.sprite;
                                    name = slint::SharedString::from(skill.name.as_str());
                                    enabled = true;
                                    cooldown = skill
                                        .on_cooldown
                                        .clone()
                                        .or_else(|| h.cooldowns.get(&slot.action_id).cloned());
                                }
                            }
                            SlotPanelType::Spell => {
                                if let Some(spell) = a.spells.iter().find(|s| s.id == action_id) {
                                    sprite = spell.sprite;
                                    name = slint::SharedString::from(spell.panel_name.as_str());
                                    enabled = true;
                                    cooldown = h.cooldowns.get(&slot.action_id).cloned();
                                }
                            }
                            _ => {}
                        }

                        let icon = if let Some(ref game_files) = game_files {
                            match action_id.panel_type() {
                                SlotPanelType::Item => load_item_icon(game_files, sprite),
                                SlotPanelType::Skill => load_skill_icon(game_files, sprite),
                                SlotPanelType::Spell => load_spell_icon(game_files, sprite),
                                _ => Ok(slint::Image::default()),
                            }
                            .unwrap_or_default()
                        } else {
                            slint::Image::default()
                        };

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
    }

    if let Some(panel) = hotbar_panel {
        if panel.is_changed() {
            let game_state = slint::ComponentHandle::global::<crate::GameState>(&strong);
            game_state.set_current_hotbar_panel(panel.current_panel as i32);
        }
    }

    if let Some(wl) = world_list {
        if wl.is_changed() {
            let game_state = slint::ComponentHandle::global::<crate::GameState>(&strong);
            game_state.set_world_list_loading(false);

            let mut slint_members = Vec::with_capacity(wl.filtered.len());
            for m in &wl.filtered {
                slint_members.push(crate::WorldListMemberUi {
                    name: slint::SharedString::from(m.name.as_str()),
                    title: slint::SharedString::from(m.title.as_str()),
                    class: slint::SharedString::from(m.class.as_str()),
                    color: slint::Color::from_argb_f32(
                        m.color[3], m.color[0], m.color[1], m.color[2],
                    ),
                    is_master: m.is_master,
                });
            }

            game_state
                .set_world_list_members(slint::ModelRc::new(slint::VecModel::from(slint_members)));
            game_state.set_world_list_count(wl.filtered.len() as i32);
            if let Some(raw) = &wl.raw {
                game_state.set_world_list_total_count(raw.world_member_count as i32);
            }
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
    ch: Option<Res<SlintUiChannels>>,
    mut writer: MessageWriter<crate::webui::plugin::UiInbound>,
) {
    let Some(ch) = ch else {
        return;
    };
    while let Ok(msg) = ch.rx.try_recv() {
        writer.write(crate::webui::plugin::UiInbound(msg));
    }
}

/// Syncs world labels and camera state to Slint every frame.
/// This enables Slint to render entity names, speech bubbles, etc. in screen space.
pub fn sync_world_labels_to_slint(
    win: Option<Res<SlintWindow>>,
    camera: Option<Res<crate::Camera>>,
    window_surface: Option<NonSend<crate::WindowSurface>>,
    zoom_state: Option<Res<ZoomState>>,
    player_attrs: Option<Res<crate::resources::PlayerAttributes>>,
    current_session: Option<Res<crate::CurrentSession>>,
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
    let Some(win) = win else {
        return;
    };
    let Some(strong) = win.0.upgrade() else {
        return;
    };
    let Some(camera) = camera else {
        return;
    };
    let Some(surface) = window_surface else {
        return;
    };

    let game_state = slint::ComponentHandle::global::<crate::GameState>(&strong);

    // Update player attributes (HP/MP)
    if let Some(attrs) = player_attrs {
        game_state.set_current_hp(attrs.current_hp as i32);
        game_state.set_max_hp(attrs.max_hp as i32);
        game_state.set_current_mp(attrs.current_mp as i32);
        game_state.set_max_mp(attrs.max_mp as i32);
    }

    // Update server name
    if let Some(session) = current_session {
        game_state.set_server_name(slint::SharedString::from(session.server_url.as_str()));
    }

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
    game_state.set_viewport_width(surface.width as f32);
    game_state.set_viewport_height(surface.height as f32);

    // Calculate display scale: how much the render texture is scaled up for display
    // For pixel-perfect rendering: user_zoom (e.g., 2.0 means render at half size, display at 2x)
    // For non-pixel-perfect: 1.0 (render at display size)
    let display_scale = zoom_state
        .map(|zs| {
            if zs.is_pixel_perfect {
                zs.user_zoom
            } else {
                1.0
            }
        })
        .unwrap_or(1.0);
    game_state.set_display_scale(display_scale);

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
                    entity_id: entity.index() as i32,
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
                entity_id: entity.index() as i32,
                text: slint::SharedString::default(),
                world_x: world_pos.x,
                world_y: world_pos.y,
                y_offset: -80.0,
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
    win: Option<Res<SlintWindow>>,
    map_query: Query<&crate::ecs::components::GameMap, Changed<crate::ecs::components::GameMap>>,
) {
    let Some(win) = win else {
        return;
    };
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
    win: Option<Res<SlintWindow>>,
) {
    let Some(win) = win else {
        return;
    };
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
