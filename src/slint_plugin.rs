use bevy::prelude::*;
use packets::server::EquipmentSlot;
use rendering::scene::Scene;
use std::sync::Arc;

use crate::resources::ZoomState;
use crate::slint_support::frame_exchange::{BackBufferPool, ControlMessage, FrameChannels};
use crate::slint_support::input_bridge::{
    self, QueuedKeyAction, QueuedKeyEvent, QueuedPointerEvent, SharedDoubleClickQueue,
    SharedKeyEventQueue, SharedPointerEventQueue, SharedScrollEventQueue,
};
use crate::slint_support::state_bridge::{
    SlintAssetLoaderRes, SlintUiChannels, SlintWindow, apply_core_to_slint, drain_slint_inbound,
    sync_installer_to_slint, sync_map_name_to_slint, sync_world_labels_to_slint,
};
use crate::{RendererState, WindowSurface};
use slint::ComponentHandle;
use slint::wgpu_27::{WGPUConfiguration, WGPUSettings};
use wgpu::{self};

fn slint_to_game_panel(panel: crate::SlotPanelType) -> game_types::SlotPanelType {
    match panel {
        crate::SlotPanelType::Item => game_types::SlotPanelType::Item,
        crate::SlotPanelType::Skill => game_types::SlotPanelType::Skill,
        crate::SlotPanelType::Spell => game_types::SlotPanelType::Spell,
        crate::SlotPanelType::Hotbar => game_types::SlotPanelType::Hotbar,
        crate::SlotPanelType::World => game_types::SlotPanelType::World,
        crate::SlotPanelType::None => game_types::SlotPanelType::None,
    }
}

// Marker that GPU + surface + scene/camera are ready for systems.
#[derive(Resource, Default)]
pub struct SlintGpuReady(pub bool);

#[derive(Resource, Debug, Clone, Message)]
pub struct SlintDoubleClickEvent(pub f32, pub f32);

/// Event emitted when the player wants to show a profile panel
#[derive(Debug, Clone, Message)]
pub enum ShowSelfProfileEvent {
    SelfRequested,  // User double-clicked self
    SelfUpdate,     // Server sent SelfProfile packet
    OtherRequested, // User double-clicked other (optimistic UI)
    OtherUpdate,    // Server sent OtherProfile packet
}

pub struct SlintBridgePlugin;

impl Plugin for SlintBridgePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SlintGpuReady>()
            .insert_resource(SlintUiChannels::default())
            .add_message::<SlintDoubleClickEvent>()
            .add_message::<ShowSelfProfileEvent>()
            .add_systems(PreUpdate, drain_slint_inbound)
            .add_systems(
                Update,
                (
                    apply_core_to_slint,
                    crate::slint_support::state_bridge::sync_portrait_to_slint,
                    crate::slint_support::state_bridge::sync_lobby_portraits_to_slint,
                    handle_show_self_profile,
                    sync_profile_to_slint,
                ),
            )
            .add_systems(
                PostUpdate,
                (
                    sync_world_labels_to_slint,
                    sync_map_name_to_slint,
                    sync_installer_to_slint,
                ),
            );
    }
}

/// System that syncs PlayerProfileState to Slint whenever it changes
fn sync_profile_to_slint(
    win: Option<Res<SlintWindow>>,
    asset_loader: Option<Res<SlintAssetLoaderRes>>,
    game_files: Option<Res<crate::game_files::GameFiles>>,
    eq_state: Option<Res<crate::webui::plugin::EquipmentState>>,
    profile_state: Option<Res<crate::webui::plugin::PlayerProfileState>>,
    portrait_state: Option<ResMut<crate::resources::ProfilePortraitState>>,
    renderer: Option<Res<RendererState>>,
    mut last_portrait_version: Local<u32>,
) {
    let Some(win) = win else {
        return;
    };
    let Some(strong) = win.0.upgrade() else {
        return;
    };
    let Some(ps) = profile_state else {
        return;
    };
    let Some(asset_loader) = asset_loader else {
        return;
    };
    let asset_loader = &asset_loader.0;

    let mut portrait_image = None;
    if let (Some(mut portrait), Some(renderer)) = (portrait_state, renderer) {
        if portrait.version != *last_portrait_version {
            let profile_size = 128;
            let next_texture = rendering::texture::Texture::create_render_texture(
                &renderer.device,
                "profile_portrait",
                profile_size,
                profile_size,
                wgpu::TextureFormat::Rgba8Unorm,
            );

            let old_texture = std::mem::replace(&mut portrait.texture, next_texture.texture);
            portrait.view = next_texture.view;

            if let Ok(image) = old_texture.try_into() {
                portrait_image = Some(image);
            }
            *last_portrait_version = portrait.version;
        }
    }

    if ps.is_changed() || portrait_image.is_some() {
        let game_state = slint::ComponentHandle::global::<crate::GameState>(&strong);
        let mut profile = game_state.get_profile();

        if let Some(img) = portrait_image {
            profile.preview = img;
        }

        if !ps.name.is_empty() {
            profile.name = slint::SharedString::from(ps.name.as_str());
        }
        profile.class = slint::SharedString::from(ps.class.as_str());
        profile.guild = slint::SharedString::from(ps.guild.as_str());
        profile.guild_rank = slint::SharedString::from(ps.guild_rank.as_str());
        profile.title = slint::SharedString::from(ps.title.as_str());
        profile.town = slint::SharedString::from(format!("{:?}", ps.nation));
        profile.group_requests_enabled = ps.group_open;
        profile.profile_text = slint::SharedString::from(ps.profile_text.as_str());

        let legend_marks: Vec<crate::LegendMarkData> = ps
            .legend_marks
            .iter()
            .map(|m| crate::LegendMarkData {
                icon_name: slint::SharedString::from(format!("{:?}", m.icon)),
                color: match format!("{:?}", m.color) {
                    c if c.contains("Red") => slint::Color::from_rgb_u8(255, 100, 100),
                    c if c.contains("Blue") => slint::Color::from_rgb_u8(100, 100, 255),
                    c if c.contains("Green") => slint::Color::from_rgb_u8(100, 255, 100),
                    c if c.contains("Yellow") => slint::Color::from_rgb_u8(255, 255, 100),
                    c if c.contains("Orange") => slint::Color::from_rgb_u8(255, 165, 0),
                    c if c.contains("Purple") => slint::Color::from_rgb_u8(160, 32, 240),
                    c if c.contains("Cyan") => slint::Color::from_rgb_u8(0, 255, 255),
                    c if c.contains("White") => slint::Color::from_rgb_u8(255, 255, 255),
                    _ => slint::Color::from_rgb_u8(200, 200, 200),
                },
                text: slint::SharedString::from(m.text.as_str()),
            })
            .collect();
        profile.legend_marks = slint::ModelRc::new(slint::VecModel::from(legend_marks));

        // Sync equipment as well if changed
        if let Some(gf) = game_files.as_ref() {
            let is_other_player = !ps.name.is_empty();

            let make_slot = |slot_type: EquipmentSlot| {
                if is_other_player {
                    if let Some(item) = ps.equipment.get(&slot_type) {
                        return crate::EquipmentSlotData {
                            name: slint::SharedString::default(),
                            icon: asset_loader
                                .load_item_icon(gf, item.sprite)
                                .unwrap_or_default(),
                            has_item: true,
                            durability_percent: 1.0,
                            current_durability: 0,
                            max_durability: 0,
                        };
                    }
                } else if let Some(eq) = eq_state.as_ref() {
                    if let Some(item) = eq.0.get(&slot_type) {
                        let durability_percent = if item.max_durability > 0 {
                            item.current_durability as f32 / item.max_durability as f32
                        } else {
                            1.0
                        };
                        return crate::EquipmentSlotData {
                            name: slint::SharedString::from(item.name.as_str()),
                            icon: asset_loader
                                .load_item_icon(gf, item.sprite)
                                .unwrap_or_default(),
                            has_item: true,
                            durability_percent,
                            current_durability: item.current_durability as i32,
                            max_durability: item.max_durability as i32,
                        };
                    }
                }
                crate::EquipmentSlotData::default()
            };

            profile.eq_weapon = make_slot(EquipmentSlot::Weapon);
            profile.eq_armor = make_slot(EquipmentSlot::Armor);
            profile.eq_shield = make_slot(EquipmentSlot::Shield);
            profile.eq_helmet = make_slot(EquipmentSlot::Helmet);
            profile.eq_earrings = make_slot(EquipmentSlot::Earrings);
            profile.eq_necklace = make_slot(EquipmentSlot::Necklace);
            profile.eq_left_ring = make_slot(EquipmentSlot::LeftRing);
            profile.eq_right_ring = make_slot(EquipmentSlot::RightRing);
            profile.eq_left_gauntlet = make_slot(EquipmentSlot::LeftGaunt);
            profile.eq_right_gauntlet = make_slot(EquipmentSlot::RightGaunt);
            profile.eq_belt = make_slot(EquipmentSlot::Belt);
            profile.eq_greaves = make_slot(EquipmentSlot::Greaves);
            profile.eq_boots = make_slot(EquipmentSlot::Boots);
            profile.eq_accessory1 = make_slot(EquipmentSlot::Accessory1);
            profile.eq_accessory2 = make_slot(EquipmentSlot::Accessory2);
            profile.eq_overcoat = make_slot(EquipmentSlot::Overcoat);
            profile.eq_over_helmet = make_slot(EquipmentSlot::OverHelm);
            profile.eq_over_armor = make_slot(EquipmentSlot::Accessory3);
        }

        game_state.set_profile(profile);
    }
}

/// System that handles ShowSelfProfileEvent to display the profile panel
fn handle_show_self_profile(
    mut reader: MessageReader<ShowSelfProfileEvent>,
    win: Option<Res<SlintWindow>>,
    asset_loader: Option<Res<SlintAssetLoaderRes>>,
    game_files: Option<Res<crate::game_files::GameFiles>>,
    eq_state: Option<Res<crate::webui::plugin::EquipmentState>>,
    mut profile_state: Option<ResMut<crate::webui::plugin::PlayerProfileState>>,
    mut portrait_state: Option<ResMut<crate::resources::ProfilePortraitState>>,
) {
    let Some(win) = win else {
        return;
    };
    let Some(strong) = win.0.upgrade() else {
        return;
    };
    let Some(asset_loader) = asset_loader else {
        return;
    };
    let asset_loader = &asset_loader.0;

    for event in reader.read() {
        let game_state = slint::ComponentHandle::global::<crate::GameState>(&strong);

        match event {
            ShowSelfProfileEvent::OtherRequested => {
                // When requesting another player, clear stale state and HIDE the panel
                // until we get the actual data from the server.
                if let Some(ps) = profile_state.as_mut() {
                    ps.clear();
                }
                let mut profile = game_state.get_profile();
                profile.visible = false;
                game_state.set_profile(profile);

                if let Some(ps_portrait) = portrait_state.as_mut() {
                    ps_portrait.dirty = true;
                }
                continue;
            }
            ShowSelfProfileEvent::SelfRequested => {
                // When requesting our own profile, clear the "other player" state so we use
                // our own local EquipmentState and Name optimistically.
                if let Some(ps) = profile_state.as_mut() {
                    ps.clear();
                }

                if let Some(ps_portrait) = portrait_state.as_mut() {
                    ps_portrait.dirty = true;
                }
            }
            ShowSelfProfileEvent::SelfUpdate => {
                // If this is a response from the server but the user closed the panel already, don't reopen it
                let current = game_state.get_profile();
                if !current.visible {
                    continue;
                }

                if let Some(ps_portrait) = portrait_state.as_mut() {
                    ps_portrait.dirty = true;
                }
            }
            ShowSelfProfileEvent::OtherUpdate => {
                // Server sent detail for another player - update and ensure it's handled below
                if let Some(ps_portrait) = portrait_state.as_mut() {
                    ps_portrait.dirty = true;
                }
            }
        }

        // Get current player name to use in profile
        let player_name = game_state.get_player_name();

        let mut profile = crate::ProfileData {
            visible: true,
            is_self: true,
            name: player_name,
            preview: portrait_state
                .as_ref()
                .and_then(|p| p.texture.clone().try_into().ok())
                .unwrap_or_default(),
            ..Default::default()
        };

        // Populate profile fields from state
        if let Some(ps) = profile_state.as_ref() {
            if !ps.name.is_empty() {
                profile.name = slint::SharedString::from(ps.name.as_str());
            }
            profile.is_self = ps.is_self;
            profile.class = slint::SharedString::from(ps.class.as_str());
            profile.guild = slint::SharedString::from(ps.guild.as_str());
            profile.guild_rank = slint::SharedString::from(ps.guild_rank.as_str());
            profile.title = slint::SharedString::from(ps.title.as_str());
            profile.town = slint::SharedString::from(format!("{:?}", ps.nation));
            profile.group_requests_enabled = ps.group_open;
            profile.profile_text = slint::SharedString::from(ps.profile_text.as_str());

            let legend_marks: Vec<crate::LegendMarkData> = ps
                .legend_marks
                .iter()
                .map(|m| crate::LegendMarkData {
                    icon_name: slint::SharedString::from(format!("{:?}", m.icon)),
                    color: match format!("{:?}", m.color) {
                        c if c.contains("Red") => slint::Color::from_rgb_u8(255, 100, 100),
                        c if c.contains("Blue") => slint::Color::from_rgb_u8(100, 100, 255),
                        c if c.contains("Green") => slint::Color::from_rgb_u8(100, 255, 100),
                        c if c.contains("Yellow") => slint::Color::from_rgb_u8(255, 255, 100),
                        c if c.contains("Orange") => slint::Color::from_rgb_u8(255, 165, 0),
                        c if c.contains("Purple") => slint::Color::from_rgb_u8(160, 32, 240),
                        c if c.contains("Cyan") => slint::Color::from_rgb_u8(0, 255, 255),
                        c if c.contains("White") => slint::Color::from_rgb_u8(255, 255, 255),
                        _ => slint::Color::from_rgb_u8(200, 200, 200),
                    },
                    text: slint::SharedString::from(m.text.as_str()),
                })
                .collect();
            profile.legend_marks = slint::ModelRc::new(slint::VecModel::from(legend_marks));
        }

        // Populate equipment if available
        if let Some(gf) = game_files.as_ref() {
            let is_other_player = profile_state
                .as_ref()
                .map_or(false, |ps| !ps.name.is_empty());

            let make_slot = |slot_type: EquipmentSlot| {
                // Try to get from profile_state first (set for other players' profiles)
                if is_other_player {
                    if let Some(ps) = profile_state.as_ref() {
                        if let Some(item) = ps.equipment.get(&slot_type) {
                            return crate::EquipmentSlotData {
                                name: slint::SharedString::default(),
                                icon: asset_loader
                                    .load_item_icon(gf, item.sprite)
                                    .unwrap_or_default(),
                                has_item: true,
                                durability_percent: 1.0,
                                current_durability: 0,
                                max_durability: 0,
                            };
                        }
                    }
                    return crate::EquipmentSlotData::default();
                }

                // Fall back to local player's equipment state (only for self profile)
                if let Some(eq) = eq_state.as_ref() {
                    if let Some(item) = eq.0.get(&slot_type) {
                        let durability_percent = if item.max_durability > 0 {
                            item.current_durability as f32 / item.max_durability as f32
                        } else {
                            1.0
                        };
                        return crate::EquipmentSlotData {
                            name: slint::SharedString::from(item.name.as_str()),
                            icon: asset_loader
                                .load_item_icon(gf, item.sprite)
                                .unwrap_or_default(),
                            has_item: true,
                            durability_percent,
                            current_durability: item.current_durability as i32,
                            max_durability: item.max_durability as i32,
                        };
                    }
                }

                crate::EquipmentSlotData::default()
            };

            profile.eq_weapon = make_slot(EquipmentSlot::Weapon);
            profile.eq_armor = make_slot(EquipmentSlot::Armor);
            profile.eq_shield = make_slot(EquipmentSlot::Shield);
            profile.eq_helmet = make_slot(EquipmentSlot::Helmet);
            profile.eq_earrings = make_slot(EquipmentSlot::Earrings);
            profile.eq_necklace = make_slot(EquipmentSlot::Necklace);
            profile.eq_left_ring = make_slot(EquipmentSlot::LeftRing);
            profile.eq_right_ring = make_slot(EquipmentSlot::RightRing);
            profile.eq_left_gauntlet = make_slot(EquipmentSlot::LeftGaunt);
            profile.eq_right_gauntlet = make_slot(EquipmentSlot::RightGaunt);
            profile.eq_belt = make_slot(EquipmentSlot::Belt);
            profile.eq_greaves = make_slot(EquipmentSlot::Greaves);
            profile.eq_boots = make_slot(EquipmentSlot::Boots);
            profile.eq_accessory1 = make_slot(EquipmentSlot::Accessory1);
            profile.eq_accessory2 = make_slot(EquipmentSlot::Accessory2);
            profile.eq_overcoat = make_slot(EquipmentSlot::Overcoat);
            profile.eq_over_helmet = make_slot(EquipmentSlot::OverHelm);
            // Accessory3 -> eq_over_armor (best guess for now)
            profile.eq_over_armor = make_slot(EquipmentSlot::Accessory3);
        }

        game_state.set_profile(profile);
        tracing::info!("Showing self profile panel");
    }
}

// Attach Slint UI to the provided Bevy `App` and return the created `MainWindow`.
// This consumes the App so the returned Slint notifier closure can own it and
// drive updates from Slint's rendering callbacks (same pattern as original main.rs).
pub fn attach_slint_ui(mut app: App) -> crate::MainWindow {
    // Configure WGPU for Slint backend (mirrors previous main.rs behavior).
    let mut wgpu_settings = WGPUSettings::default();
    wgpu_settings.device_required_features = wgpu::Features::PUSH_CONSTANTS;
    wgpu_settings.device_required_limits.max_push_constant_size = 16;

    slint::BackendSelector::new()
        .require_wgpu_27(WGPUConfiguration::Automatic(wgpu_settings))
        .select()
        .expect("Unable to create Slint backend with WGPU based renderer");

    // Finish building schedules so systems are ready before Slint takes control.
    app.finish();
    app.cleanup();

    let slint_app = crate::MainWindow::new().unwrap();
    let key_event_queue: SharedKeyEventQueue = input_bridge::new_shared_queue();
    let pointer_event_queue: SharedPointerEventQueue = input_bridge::new_shared_pointer_queue();
    let scroll_event_queue: SharedScrollEventQueue = input_bridge::new_shared_scroll_queue();
    let double_click_queue: SharedDoubleClickQueue = input_bridge::new_shared_double_click_queue();

    app.insert_resource(input_bridge::SlintKeyEventQueue(key_event_queue.clone()));
    app.insert_resource(input_bridge::SlintPointerEventQueue(
        pointer_event_queue.clone(),
    ));
    app.insert_resource(input_bridge::SlintScrollEventQueue(
        scroll_event_queue.clone(),
    ));
    app.insert_resource(input_bridge::SlintDoubleClickQueue(
        double_click_queue.clone(),
    ));

    {
        let queue = Arc::clone(&key_event_queue);
        let input_bridge = slint::ComponentHandle::global::<crate::InputBridge>(&slint_app);
        input_bridge.on_key_pressed(move |key| {
            if let Some(code) = input_bridge::slint_key_to_keycode(&key) {
                if let Ok(mut guard) = queue.lock() {
                    guard.push_back(QueuedKeyEvent {
                        code,
                        action: QueuedKeyAction::Press,
                    });
                }
            }
            i_slint_core::items::EventResult::Accept
        });
    }

    {
        let slint_app_weak = slint_app.as_weak();
        let input_bridge = slint::ComponentHandle::global::<crate::InputBridge>(&slint_app);
        input_bridge.on_rebind_key_event(move |event| {
            let Some(strong) = slint_app_weak.upgrade() else {
                return false;
            };

            let settings_global = slint::ComponentHandle::global::<crate::SettingsState>(&strong);

            if event.text.as_str() == "\u{1b}" {
                settings_global.set_is_rebinding(false);
                settings_global.set_rebinding_action(slint::SharedString::from(""));
                return true;
            }

            if let Some(code) = input_bridge::slint_key_to_keycode(&event) {
                let mut key_string = String::new();

                if event.modifiers.control {
                    key_string.push_str("Ctrl+");
                }
                if event.modifiers.shift {
                    key_string.push_str("Shift+");
                }
                if event.modifiers.alt {
                    key_string.push_str("Alt+");
                }

                key_string.push_str(&input_bridge::keycode_to_string(code));
                settings_global.invoke_rebind_key(slint::SharedString::from(key_string));
                return true;
            }

            false
        });
    }

    {
        let queue = Arc::clone(&key_event_queue);
        let input_bridge = slint::ComponentHandle::global::<crate::InputBridge>(&slint_app);
        input_bridge.on_key_released(move |key| {
            if let Some(code) = input_bridge::slint_key_to_keycode(&key) {
                if let Ok(mut guard) = queue.lock() {
                    guard.push_back(QueuedKeyEvent {
                        code,
                        action: QueuedKeyAction::Release,
                    });
                }
            }
            i_slint_core::items::EventResult::Accept
        });
    }

    {
        let queue = Arc::clone(&pointer_event_queue);
        let input_bridge = slint::ComponentHandle::global::<crate::InputBridge>(&slint_app);
        input_bridge.on_pointer_event(move |event, x, y| {
            if let Ok(mut guard) = queue.lock() {
                guard.push_back(QueuedPointerEvent {
                    kind: event.kind,
                    button: event.button,
                    position: (x, y),
                });
            }
            i_slint_core::items::EventResult::Accept
        });
    }

    {
        let queue = Arc::clone(&double_click_queue);
        let input_bridge = slint::ComponentHandle::global::<crate::InputBridge>(&slint_app);
        input_bridge.on_double_click(move |x, y| {
            if let Ok(mut guard) = queue.lock() {
                guard.push_back((x, y));
            }
        });
    }

    {
        let queue = Arc::clone(&scroll_event_queue);
        let input_bridge = slint::ComponentHandle::global::<crate::InputBridge>(&slint_app);
        input_bridge.on_scroll_event(move |x, y, dx, dy| {
            if let Ok(mut guard) = queue.lock() {
                guard.push_back(input_bridge::QueuedScrollEvent {
                    position: (x, y),
                    delta: (dx, dy),
                });
            }
            i_slint_core::items::EventResult::Accept
        });
    }

    // Install weak handle so Bevy systems can mutate properties
    {
        let weak = slint_app.as_weak();
        app.world_mut().insert_resource(SlintWindow(weak));
    }

    // Wire Slint callbacks -> crossbeam channel -> UiInbound messages
    if let Some(ch) = app.world().get_resource::<SlintUiChannels>() {
        let login_bridge = slint::ComponentHandle::global::<crate::LoginBridge>(&slint_app);
        {
            let tx_login = ch.tx.clone();
            login_bridge.on_attempt_login(move |server_id, username, password, remember| {
                let _ = tx_login.send(crate::webui::ipc::UiToCore::LoginSubmit {
                    server_id: server_id as u32,
                    username: username.to_string(),
                    password: password.to_string(),
                    remember,
                });
            });
        }
        {
            let tx_use_saved = ch.tx.clone();
            login_bridge.on_use_saved(move |id| {
                let _ = tx_use_saved
                    .send(crate::webui::ipc::UiToCore::LoginUseSaved { id: id.to_string() });
            });
        }
        {
            let tx_remove_saved = ch.tx.clone();
            login_bridge.on_remove_saved(move |id| {
                let _ = tx_remove_saved
                    .send(crate::webui::ipc::UiToCore::LoginRemoveSaved { id: id.to_string() });
            });
        }
        {
            let tx_change_server = ch.tx.clone();
            login_bridge.on_change_current_server(move |id| {
                let _ = tx_change_server
                    .send(crate::webui::ipc::UiToCore::ServersChangeCurrent { id: id as u32 });
            });
        }
        {
            let tx_add_server = ch.tx.clone();
            login_bridge.on_add_server(move |name, address| {
                let server = crate::webui::ipc::ServerNoId {
                    name: name.to_string(),
                    address: address.to_string(),
                };
                let _ = tx_add_server.send(crate::webui::ipc::UiToCore::ServersAdd { server });
            });
        }
        {
            let tx_edit_server = ch.tx.clone();
            login_bridge.on_edit_server(move |id, name, address| {
                let server = crate::webui::ipc::ServerWithId {
                    id: id as u32,
                    name: name.to_string(),
                    address: address.to_string(),
                };
                let _ = tx_edit_server.send(crate::webui::ipc::UiToCore::ServersEdit { server });
            });
        }
        {
            let tx_remove_server = ch.tx.clone();
            login_bridge.on_remove_server(move |id| {
                let _ = tx_remove_server
                    .send(crate::webui::ipc::UiToCore::ServersRemove { id: id as u32 });
            });
        }
        let tx_snapshot = ch.tx.clone();
        slint_app.on_request_snapshot(move || {
            let _ = tx_snapshot.send(crate::webui::ipc::UiToCore::RequestSnapshot);
        });

        let game_state = slint::ComponentHandle::global::<crate::GameState>(&slint_app);
        {
            let tx_world_map = ch.tx.clone();
            game_state.on_world_map_click(move |map_id, x, y, check_sum| {
                let _ = tx_world_map.send(crate::webui::ipc::UiToCore::WorldMapClick {
                    map_id: map_id as u16,
                    x: x as u16,
                    y: y as u16,
                    check_sum: check_sum as u16,
                });
            });
        }
        {
            let tx_menu = ch.tx.clone();
            game_state.on_menu_select(move |id, name| {
                let _ = tx_menu.send(crate::webui::ipc::UiToCore::MenuSelect {
                    id: id as u16,
                    name: name.to_string(),
                });
            });
        }
        {
            let tx = ch.tx.clone();
            game_state.on_unequip(move |slot| {
                if tx
                    .send(crate::webui::ipc::UiToCore::Unequip { slot: slot as u8 })
                    .is_err()
                {
                    tracing::error!("Failed to send Unequip message");
                }
            });
        }
        {
            let tx = ch.tx.clone();
            game_state.on_use_action(move |panel, slot| {
                if tx
                    .send(crate::webui::ipc::UiToCore::ActivateAction {
                        category: slint_to_game_panel(panel),
                        index: slot as usize,
                    })
                    .is_err()
                {
                    tracing::error!("Failed to send ActivateAction message");
                }
            });
        }
        {
            let tx = ch.tx.clone();
            game_state.on_set_hotbar_panel(move |panel_num| {
                if tx
                    .send(crate::webui::ipc::UiToCore::SetHotbarPanel {
                        panel_num: panel_num as u8,
                    })
                    .is_err()
                {
                    tracing::error!("Failed to send SetHotbarPanel message");
                }
            });
        }
        {
            let tx = ch.tx.clone();
            game_state.on_refresh_world_list(move || {
                let _ = tx.send(crate::webui::ipc::UiToCore::RequestWorldList);
            });
        }
        {
            let tx = ch.tx.clone();
            game_state.on_set_world_list_filter(move |class, master_only, search| {
                let _ = tx.send(crate::webui::ipc::UiToCore::SetWorldListFilter {
                    filter: crate::webui::ipc::WorldListFilter {
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
        {
            let tx = ch.tx.clone();
            game_state.on_send_chat(move |text| {
                if tx
                    .send(crate::webui::ipc::UiToCore::ChatSubmit {
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
        {
            let tx = ch.tx.clone();
            let slint_app_weak = slint_app.as_weak();
            game_state.on_send_whisper(move |target, text| {
                if let Some(app) = slint_app_weak.upgrade() {
                    let gs = app.global::<crate::GameState>();
                    gs.set_last_whisper_target(target.clone());
                }

                if tx
                    .send(crate::webui::ipc::UiToCore::ChatSubmit {
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
        let dragdrop_state = slint::ComponentHandle::global::<crate::DragDropState>(&slint_app);
        {
            let tx = ch.tx.clone();
            dragdrop_state.on_action_drag_drop(
                move |src_panel, src_slot, dst_panel, dst_slot, x, y| {
                    tracing::info!(
                        "DragDropAction from {:?} slot {} to {:?} slot {} at ({}, {})",
                        src_panel,
                        src_slot,
                        dst_panel,
                        dst_slot,
                        x,
                        y
                    );

                    if tx
                        .send(crate::webui::ipc::UiToCore::DragDropAction {
                            src_category: slint_to_game_panel(src_panel),
                            src_index: src_slot as usize,
                            dst_category: slint_to_game_panel(dst_panel),
                            dst_index: dst_slot as usize,
                            x,
                            y,
                        })
                        .is_err()
                    {
                        tracing::error!("Failed to send DragDropAction message");
                    }
                },
            );
        }

        let settings_state = slint::ComponentHandle::global::<crate::SettingsState>(&slint_app);
        {
            let tx_xray = ch.tx.clone();
            settings_state.on_xray_size_changed(move |size| {
                let _ = tx_xray.send(crate::webui::ipc::UiToCore::SettingsChange {
                    xray_size: size as u8,
                });
            });
        }
        {
            let tx_sfx = ch.tx.clone();
            settings_state.on_sfx_volume_changed(move |vol| {
                let _ = tx_sfx.send(crate::webui::ipc::UiToCore::VolumeChange {
                    sfx: Some(vol),
                    music: None,
                });
            });
        }
        {
            let tx_music = ch.tx.clone();
            settings_state.on_music_volume_changed(move |vol| {
                let _ = tx_music.send(crate::webui::ipc::UiToCore::VolumeChange {
                    sfx: None,
                    music: Some(vol),
                });
            });
        }
        {
            let tx_scale = ch.tx.clone();
            settings_state.on_scale_changed(move |scale| {
                let _ = tx_scale.send(crate::webui::ipc::UiToCore::ScaleChange { scale });
            });
        }
        {
            let slint_app_weak = slint_app.as_weak();
            settings_state.on_start_rebind(move |action| {
                if let Some(strong) = slint_app_weak.upgrade() {
                    let settings_global =
                        slint::ComponentHandle::global::<crate::SettingsState>(&strong);
                    settings_global.set_rebinding_action(action.clone());
                    settings_global.set_is_rebinding(true);
                }
            });
        }
        {
            let slint_app_weak = slint_app.as_weak();
            let tx_rebind = ch.tx.clone();
            settings_state.on_rebind_key(move |key_code| {
                if let Some(strong) = slint_app_weak.upgrade() {
                    let settings_global =
                        slint::ComponentHandle::global::<crate::SettingsState>(&strong);
                    let action = settings_global.get_rebinding_action().to_string();
                    settings_global.set_is_rebinding(false);
                    settings_global.set_rebinding_action(slint::SharedString::from(""));
                    let _ = tx_rebind.send(crate::webui::ipc::UiToCore::RebindKey {
                        action,
                        new_key: key_code.to_string(),
                    });
                }
            });
        }
        {
            let slint_app_weak = slint_app.as_weak();
            settings_state.on_cancel_rebind(move || {
                if let Some(strong) = slint_app_weak.upgrade() {
                    let settings_global =
                        slint::ComponentHandle::global::<crate::SettingsState>(&strong);
                    settings_global.set_is_rebinding(false);
                    settings_global.set_rebinding_action(slint::SharedString::from(""));
                }
            });
        }
    }

    // Capture mutable Bevy app to drive frames from Slint's render callbacks.
    let slint_app_handle = slint_app.as_weak();

    slint_app
        .window()
        .set_rendering_notifier(move |rendering_state, graphics_api| match rendering_state {
            slint::RenderingState::RenderingSetup => {
                if let slint::GraphicsAPI::WGPU27 { device, queue, .. } = graphics_api {
                    let Some(strong) = slint_app_handle.upgrade() else {
                        return;
                    };

                    let window = strong.window();
                    initialize_gpu_world(
                        &mut app.world_mut(),
                        &device,
                        &queue,
                        window,
                        wgpu::TextureFormat::Rgba8Unorm,
                    );
                    let size = window.size();

                    // Grab control sender clone without holding a mutable borrow to the World
                    let ctrl_sender = app
                        .world()
                        .get_resource::<FrameChannels>()
                        .map(|c| c.control_tx.clone());

                    if let Some(mut pool) = app.world_mut().get_resource_mut::<BackBufferPool>() {
                        if pool.0.is_empty() {
                            let mut seeded = Vec::new();
                            for label in ["Front Buffer", "Back Buffer", "Inflight Buffer"] {
                                let tex = device.create_texture(&wgpu::TextureDescriptor {
                                    label: Some(label),
                                    size: wgpu::Extent3d {
                                        width: size.width,
                                        height: size.height,
                                        depth_or_array_layers: 1,
                                    },
                                    mip_level_count: 1,
                                    sample_count: 1,
                                    dimension: wgpu::TextureDimension::D2,
                                    format: wgpu::TextureFormat::Rgba8Unorm,
                                    usage: wgpu::TextureUsages::TEXTURE_BINDING
                                        | wgpu::TextureUsages::COPY_DST
                                        | wgpu::TextureUsages::COPY_SRC
                                        | wgpu::TextureUsages::RENDER_ATTACHMENT,
                                    view_formats: &[],
                                });
                                seeded.push(tex);
                            }
                            if let Some(tx) = ctrl_sender {
                                for tex in seeded.into_iter() {
                                    let _ =
                                        tx.try_send(ControlMessage::ReleaseFrontBufferTexture {
                                            texture: tex,
                                        });
                                }
                            } else {
                                pool.0.extend(seeded.into_iter());
                            }
                        }
                    }
                    tracing::info!("WGPU Rendering setup complete (Slint -> Bevy bridge)");

                    // One update so startup systems that depend on GPU can initialize.
                    app.update();
                }
            }
            slint::RenderingState::BeforeRendering => {
                app.update();
                let Some(strong) = slint_app_handle.upgrade() else {
                    return;
                };
                strong.window().request_redraw();

                let display_width = strong.get_requested_texture_width() as u32;
                let display_height = strong.get_requested_texture_height() as u32;
                let dpi_scale = strong.get_texture_scale();

                if display_width > 0 && display_height > 0 {
                    if let Some(mut zoom_state) = app.world_mut().get_resource_mut::<ZoomState>() {
                        if zoom_state.display_size != (display_width, display_height) {
                            zoom_state.set_display_size(display_width, display_height);
                        }
                        if (zoom_state.dpi_scale - dpi_scale).abs() > 0.001 {
                            zoom_state.set_dpi_scale(dpi_scale);
                        }
                    }
                }

                let (render_size, is_pixel_perfect, camera_zoom) = app
                    .world()
                    .get_resource::<ZoomState>()
                    .map(|zs| (zs.render_size, zs.is_pixel_perfect, zs.camera_zoom))
                    .unwrap_or(((display_width, display_height), true, 1.0));

                strong.set_use_pixelated_filtering(is_pixel_perfect);

                if let Some(ch) = app.world().get_resource::<FrameChannels>() {
                    if render_size.0 > 0 && render_size.1 > 0 {
                        let needs_resize = app
                            .world()
                            .get_non_send_resource::<WindowSurface>()
                            .map(|surface| {
                                surface.width != render_size.0
                                    || surface.height != render_size.1
                                    || (surface.scale_factor - camera_zoom).abs() > 0.001
                            })
                            .unwrap_or(true);

                        if needs_resize {
                            tracing::info!(
                                "Resizing render target: {}x{} (display: {}x{}, zoom: {:.2}x, camera_zoom: {:.2})",
                                render_size.0,
                                render_size.1,
                                display_width,
                                display_height,
                                app.world().get_resource::<ZoomState>().map(|zs| zs.user_zoom).unwrap_or(1.0),
                                camera_zoom
                            );

                            let _ = ch.control_tx.try_send(ControlMessage::ResizeBuffers {
                                width: render_size.0,
                                height: render_size.1,
                                scale: camera_zoom,
                            });
                        }
                    }

                    if let Ok(new_texture) = ch.front_buffer_rx.try_recv() {
                        if let Some(old) = strong.get_texture().to_wgpu_27_texture() {
                            let _ =
                                ch.control_tx
                                    .try_send(ControlMessage::ReleaseFrontBufferTexture {
                                        texture: old,
                                    });
                        }
                        if let Ok(image) = new_texture.try_into() {
                            strong.set_texture(image);
                        }
                    }
                }
            }
            _ => {}
        })
        .expect("Failed to set rendering notifier - WGPU integration may not be available");

    slint_app
}

// Initialize full renderer stack (surface, scene, camera) using Slint's provided wgpu context + window.
pub fn initialize_gpu_world(
    world: &mut World,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    window: &slint::Window,
    texture_format: wgpu::TextureFormat,
) {
    // Avoid double init if already present.
    if world.contains_resource::<RendererState>() {
        return;
    }

    let size = window.size();

    tracing::info!(
        "Initializing Slint GPU world with size {}x{} (scale factor {})",
        size.width,
        size.height,
        window.scale_factor()
    );

    let mut scene = Scene::new(device, size.width, size.height, texture_format);
    scene.resize_depth_texture(device, size.width, size.height);
    let camera = rendering::scene::CameraState::new(
        (size.width, size.height).into(),
        device,
        window.scale_factor(),
    );

    world.insert_resource(RendererState {
        device: device.clone(),
        queue: queue.clone(),
        scene,
    });
    // Safety: surface tied to window lifetime which matches app lifetime.
    world.insert_non_send_resource(WindowSurface {
        width: size.width,
        height: size.height,
        scale_factor: window.scale_factor(),
    });
    world.insert_resource(crate::Camera { camera });
    let initial_zoom = world
        .get_resource::<crate::settings_types::Settings>()
        .map(|s| s.graphics.scale)
        .unwrap_or(1.0);
    world.insert_resource(ZoomState::new(
        size.width,
        size.height,
        window.scale_factor(),
        initial_zoom,
    ));
    // Initialize frame channels & an empty pool (textures allocated lazily after notifier knows desired size)
    if !world.contains_resource::<FrameChannels>() {
        world.insert_resource(FrameChannels::new());
    }
    world.init_resource::<BackBufferPool>();
    // Seed frame buffers and channels will be handled from main.rs after this call.
    if let Some(mut ready) = world.get_resource_mut::<SlintGpuReady>() {
        ready.0 = true;
    } else {
        world.insert_resource(SlintGpuReady(true));
    }
    tracing::info!("Slint GPU world initialized (surface + scene + camera)");
}

#[allow(dead_code)]
pub fn create_color_target(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("GameFrame"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    })
}
