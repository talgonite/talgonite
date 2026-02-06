use std::time::Instant;

use bevy::input::ButtonInput;
use bevy::input::mouse::MouseButton;
use bevy::prelude::*;
use bevy::tasks::{IoTaskPool, Task};
use futures_lite::future;
use game_types::SlotPanelType;
pub use game_ui::CursorPosition;
use game_ui::{
    ActionId, ChatEntryUi, Cooldown, CoreToUi, InventoryItemUi, KeyboardEdges, LoginError,
    MenuEntryUi, SkillUi, SpellUi, UiToCore, WorldListFilter, WorldListMemberUi, WorldMapNodeUi,
};
use packets::client;
use packets::server::display_menu::DisplayMenuPayload;
use packets::types::{EntityType, MenuType};

use crate::app_state::AppState;
use crate::events::{AbilityEvent, ChatEvent, InventoryEvent, SessionEvent};
use crate::render_plugin::game::WebUi;
use rendering::scene::utils::screen_to_iso_tile;

use super::keyring;
use super::settings::{SavedCredential, SavedCredentialPublic, ServerEntry, SettingsFile};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ActiveWindowType {
    #[default]
    None,
    Dialog,
    Menu,
    Info,
}

#[derive(Resource, Default)]
pub struct ActiveMenuContext {
    pub window_type: ActiveWindowType,
    pub entity_type: Option<EntityType>,
    pub entity_id: u32,
    /// For shop menus and text entry, this is the pursuit_id to send back
    /// For text (list) menus, this is 0 (pursuit_id comes from the selected option)
    pub pursuit_id: u16,
    pub menu_type: Option<MenuType>,
    pub args: String,
    pub dialog_id: Option<u16>,
}

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
            .init_resource::<WorldListState>()
            .init_resource::<EquipmentState>()
            .init_resource::<PlayerProfileState>()
            .init_resource::<crate::ecs::hotbar::HotbarState>()
            .init_resource::<crate::ecs::hotbar::HotbarPanelState>()
            .init_resource::<ActiveMenuContext>()
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
                    handle_ui_inbound_ingame.run_if(in_state(AppState::InGame)),
                    handle_login_tasks,
                    handle_login_results,
                    update_skill_cooldowns,
                    sync_settings_to_ui,
                ),
            )
            .add_systems(Last, (clear_input_edges, clear_just_input))
            .add_systems(Update, emit_snapshot_on_state_change);
    }
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

// No longer converts AppState to a UI screen; UI manages its own screens.

#[derive(bevy::ecs::system::SystemParam)]
struct HotbarResources<'w> {
    hotbar_state: ResMut<'w, crate::ecs::hotbar::HotbarState>,
    hotbar_panel_state: ResMut<'w, crate::ecs::hotbar::HotbarPanelState>,
    session: Res<'w, crate::CurrentSession>,
}

#[derive(bevy::ecs::system::SystemParam)]
struct InteractionResources<'w, 's> {
    camera: Res<'w, crate::Camera>,
    window_surface: NonSend<'w, crate::WindowSurface>,
    zoom_state: ResMut<'w, crate::resources::ZoomState>,
    entity_query: Query<
        'w,
        's,
        (
            Entity,
            &'static crate::ecs::components::Position,
            &'static crate::ecs::components::Hitbox,
            Option<&'static crate::ecs::components::EntityId>,
            Option<&'static crate::ecs::components::NPC>,
            Option<&'static crate::ecs::components::Player>,
            Option<&'static crate::ecs::components::LocalPlayer>,
        ),
    >,
}

#[derive(bevy::ecs::system::SystemParam)]
struct UiStateResources<'w> {
    inv_state: Res<'w, InventoryState>,
    ability_state: Res<'w, AbilityState>,
    world_list_state: ResMut<'w, WorldListState>,
}

#[derive(bevy::ecs::system::SystemParam)]
struct InputBindingResources<'w> {
    input_bindings: ResMut<'w, crate::input::InputBindings>,
    unified_bindings: ResMut<'w, crate::input::UnifiedInputBindings>,
}

fn handle_ui_inbound_ingame(
    mut inbound: MessageReader<UiInbound>,
    mut outbound: MessageWriter<UiOutbound>,
    mut settings: ResMut<SettingsFile>,
    mut inventory_events: MessageWriter<InventoryEvent>,
    mut ability_events: MessageWriter<AbilityEvent>,
    mut chat_events: MessageWriter<ChatEvent>,
    mut next_state: ResMut<NextState<AppState>>,
    outbox: Res<crate::network::PacketOutbox>,
    ui_state: UiStateResources,
    mut menu_ctx: ResMut<ActiveMenuContext>,
    bindings: InputBindingResources,
    hotbar_res: HotbarResources,
    interaction_res: InteractionResources,
) {
    let mut hotbar_state = hotbar_res.hotbar_state;
    let mut hotbar_panel_state = hotbar_res.hotbar_panel_state;
    let inv_state = ui_state.inv_state;
    let ability_state = ui_state.ability_state;
    let mut world_list_state = ui_state.world_list_state;
    let mut input_bindings = bindings.input_bindings;
    let mut unified_bindings = bindings.unified_bindings;
    let mut zoom_state = interaction_res.zoom_state;
    let session = hotbar_res.session;
    for UiInbound(msg) in inbound.read() {
        match msg {
            UiToCore::InputKeyboard { .. } | UiToCore::InputPointer { .. } => {
                // Handled by handle_input_bridge
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
                            pursuit_id: menu_ctx.pursuit_id,
                            dialog_id: final_dialog_id as u16,
                            args,
                        });
                    }
                    continue;
                }

                let (pursuit_id, args) = if menu_ctx.pursuit_id > 0 {
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
                        packets::client::MenuInteractionArgs::Topics(topics)
                    };

                    (menu_ctx.pursuit_id, args)
                } else {
                    let pursuit_id = *id as u16;
                    let args = if !menu_ctx.args.is_empty() || !name.is_empty() {
                        let mut topics = Vec::new();
                        if !menu_ctx.args.is_empty() {
                            topics.push(menu_ctx.args.clone());
                        }
                        if !name.is_empty() {
                            topics.push(name.clone());
                        }
                        packets::client::MenuInteractionArgs::Topics(topics)
                    } else {
                        packets::client::MenuInteractionArgs::Slot(0)
                    };
                    (pursuit_id, args)
                };

                if let Some(entity_type) = menu_ctx.entity_type {
                    outbox.send(&packets::client::MenuInteraction {
                        entity_type,
                        entity_id: menu_ctx.entity_id,
                        pursuit_id,
                        args,
                    });
                }
            }
            UiToCore::MenuClose => {
                if let Some(dialog_id) = menu_ctx.dialog_id {
                    if let Some(entity_type) = menu_ctx.entity_type {
                        outbox.send(&packets::client::DialogInteraction {
                            entity_type,
                            entity_id: menu_ctx.entity_id,
                            pursuit_id: menu_ctx.pursuit_id,
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
            UiToCore::RequestSnapshot => {
                outbound.write(UiOutbound(settings.to_snapshot_message(None)));
                outbound.write(UiOutbound(settings.to_sync_message()));
            }
            UiToCore::SettingsChange { xray_size } => {
                settings.graphics.xray_size = crate::settings_types::XRaySize::from_u8(*xray_size);
            }
            UiToCore::VolumeChange { sfx, music } => {
                if let Some(sfx_vol) = sfx {
                    settings.audio.sfx_volume = *sfx_vol;
                }
                if let Some(music_vol) = music {
                    settings.audio.music_volume = *music_vol;
                }
            }
            UiToCore::ScaleChange { scale } => {
                settings.graphics.scale = *scale;
                zoom_state.set_zoom(*scale);
            }
            UiToCore::RebindKey {
                action,
                new_key,
                index,
            } => {
                use crate::input::{InputBindings, UnifiedInputBindings};
                let index = *index;

                // Conflict detection: if new_key is already bound to another action, clear that action's binding at that index
                macro_rules! check_conflict {
                    ($field:ident) => {
                        for k in settings.key_bindings.$field.iter_mut() {
                            if !new_key.is_empty() && k == new_key {
                                *k = "".to_string();
                            }
                        }
                    };
                }

                check_conflict!(move_up);
                check_conflict!(move_down);
                check_conflict!(move_left);
                check_conflict!(move_right);
                check_conflict!(inventory);
                check_conflict!(skills);
                check_conflict!(spells);
                check_conflict!(settings);
                check_conflict!(refresh);
                check_conflict!(basic_attack);
                check_conflict!(hotbar_slot_1);
                check_conflict!(hotbar_slot_2);
                check_conflict!(hotbar_slot_3);
                check_conflict!(hotbar_slot_4);
                check_conflict!(hotbar_slot_5);
                check_conflict!(hotbar_slot_6);
                check_conflict!(hotbar_slot_7);
                check_conflict!(hotbar_slot_8);
                check_conflict!(hotbar_slot_9);
                check_conflict!(hotbar_slot_10);
                check_conflict!(hotbar_slot_11);
                check_conflict!(hotbar_slot_12);
                check_conflict!(switch_to_inventory);
                check_conflict!(switch_to_skills);
                check_conflict!(switch_to_spells);
                check_conflict!(switch_to_hotbar_1);
                check_conflict!(switch_to_hotbar_2);
                check_conflict!(switch_to_hotbar_3);

                macro_rules! set_field {
                    ($field:ident) => {
                        if action == stringify!($field) {
                            settings.key_bindings.$field[index] = new_key.clone();
                        }
                    };
                }

                set_field!(move_up);
                set_field!(move_down);
                set_field!(move_left);
                set_field!(move_right);
                set_field!(inventory);
                set_field!(skills);
                set_field!(spells);
                set_field!(settings);
                set_field!(refresh);
                set_field!(basic_attack);
                set_field!(hotbar_slot_1);
                set_field!(hotbar_slot_2);
                set_field!(hotbar_slot_3);
                set_field!(hotbar_slot_4);
                set_field!(hotbar_slot_5);
                set_field!(hotbar_slot_6);
                set_field!(hotbar_slot_7);
                set_field!(hotbar_slot_8);
                set_field!(hotbar_slot_9);
                set_field!(hotbar_slot_10);
                set_field!(hotbar_slot_11);
                set_field!(hotbar_slot_12);
                set_field!(switch_to_inventory);
                set_field!(switch_to_skills);
                set_field!(switch_to_spells);
                set_field!(switch_to_hotbar_1);
                set_field!(switch_to_hotbar_2);
                set_field!(switch_to_hotbar_3);

                // Refresh the runtime bindings from the updated settings
                *unified_bindings = UnifiedInputBindings::from_settings(&settings.key_bindings);
                *input_bindings = InputBindings::from_settings(&settings.key_bindings);
            }
            UiToCore::UnbindKey { action, index } => {
                use crate::input::{InputBindings, UnifiedInputBindings};
                let index = *index;

                macro_rules! clear_field {
                    ($field:ident) => {
                        if action == stringify!($field) {
                            settings.key_bindings.$field[index] = "".to_string();
                        }
                    };
                }

                clear_field!(move_up);
                clear_field!(move_down);
                clear_field!(move_left);
                clear_field!(move_right);
                clear_field!(inventory);
                clear_field!(skills);
                clear_field!(spells);
                clear_field!(settings);
                clear_field!(refresh);
                clear_field!(basic_attack);
                clear_field!(hotbar_slot_1);
                clear_field!(hotbar_slot_2);
                clear_field!(hotbar_slot_3);
                clear_field!(hotbar_slot_4);
                clear_field!(hotbar_slot_5);
                clear_field!(hotbar_slot_6);
                clear_field!(hotbar_slot_7);
                clear_field!(hotbar_slot_8);
                clear_field!(hotbar_slot_9);
                clear_field!(hotbar_slot_10);
                clear_field!(hotbar_slot_11);
                clear_field!(hotbar_slot_12);
                clear_field!(switch_to_inventory);
                clear_field!(switch_to_skills);
                clear_field!(switch_to_spells);
                clear_field!(switch_to_hotbar_1);
                clear_field!(switch_to_hotbar_2);
                clear_field!(switch_to_hotbar_3);

                // Refresh the runtime bindings from the updated settings
                *unified_bindings = UnifiedInputBindings::from_settings(&settings.key_bindings);
                *input_bindings = InputBindings::from_settings(&settings.key_bindings);
            }
            UiToCore::ExitApplication => {
                let _ = slint::quit_event_loop();
            }
            UiToCore::ReturnToMainMenu => {
                next_state.set(AppState::MainMenu);
            }
            UiToCore::SetHotbarPanel { panel_num } => {
                hotbar_panel_state.current_panel =
                    crate::ecs::hotbar::HotbarPanel::from_u8(*panel_num);
            }
            UiToCore::RequestWorldList => {
                outbox.send(&packets::client::WorldListRequest);
            }
            UiToCore::SetWorldListFilter { filter } => {
                world_list_state.filter = filter.clone();
                world_list_state.version = world_list_state.version.wrapping_add(1);
            }
            UiToCore::Unequip { slot } => {
                inventory_events.write(InventoryEvent::Unequip { slot: *slot });
            }
            UiToCore::ActivateAction { category, index } => match category {
                SlotPanelType::Item => {
                    inventory_events.write(InventoryEvent::Use { slot: *index as u8 });
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
                                _ => {}
                            }
                        }
                    }
                }
                SlotPanelType::World => {}
                SlotPanelType::None => {}
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
                    let camera = &interaction_res.camera;
                    let cam_pos = camera.camera.position();
                    let cam_zoom = camera.camera.zoom();
                    let win_size = Vec2::new(
                        interaction_res.window_surface.width as f32,
                        interaction_res.window_surface.height as f32,
                    );

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
                    for (entity, pos, hitbox, entity_id, npc, player, local) in
                        interaction_res.entity_query.iter()
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
                                            count: 1, // Only drop 1 as requested
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

                    if matches!(src_category, SlotPanelType::Hotbar) {
                        let bar = *src_index / 12;
                        let slot = *src_index % 12;
                        hotbar_state.config.set_slot(bar, slot, "".to_string());

                        settings.set_hotbars(
                            session.server_id,
                            &session.username,
                            hotbar_state.config.clone(),
                        );
                        tracing::info!("Deallocated hotbar slot {} (dropped on world)", src_index);
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
            _ => {}
        }
    }
}

fn handle_ui_inbound_login(
    mut inbound: MessageReader<UiInbound>,
    mut outbound: MessageWriter<UiOutbound>,
    mut settings: ResMut<SettingsFile>,
    mut commands: Commands,
    bindings: InputBindingResources,
) {
    let mut input_bindings = bindings.input_bindings;
    let mut unified_bindings = bindings.unified_bindings;

    for UiInbound(msg) in inbound.read() {
        match msg {
            UiToCore::InputKeyboard { .. } | UiToCore::InputPointer { .. } => {}
            UiToCore::ExitApplication => {
                let _ = slint::quit_event_loop();
            }
            UiToCore::ReturnToMainMenu => {}
            UiToCore::RequestSnapshot => {
                outbound.write(UiOutbound(settings.to_snapshot_message(None)));
                outbound.write(UiOutbound(settings.to_sync_message()));
            }
            UiToCore::LoginSubmit {
                server_id,
                username,
                password,
                remember,
            } => {
                println!(
                    "[webui] LoginSubmit: server_id={:?} username={}",
                    server_id, username
                );
                // Stay on the login screen and start background login task
                let server = settings
                    .servers
                    .iter()
                    .find(|s| s.id == *server_id)
                    .cloned();
                if let Some(server) = server {
                    let uname = username.clone();
                    let pw = password.clone();
                    let uname_task = uname.clone();
                    let pw_task = pw.clone();
                    let remember = *remember;
                    let cred_id = format!("{}:{}", server.id, uname);
                    let task: Task<
                        Result<(network::DecryptedReceiver, network::EncryptedSender), LoginError>,
                    > = IoTaskPool::get().spawn(async move {
                        let (host, port) = parse_host_port(&server.address)
                            .unwrap_or((server.address.clone(), 2610));
                        match crate::session_prelogin::PreLoginSession::new(&host, port).await {
                            Ok(lobby) => match lobby.login(&uname_task, &pw_task).await {
                                Ok((rx, tx)) => Ok((rx, tx)),
                                Err(code) => Err(code),
                            },
                            Err(_) => Err(LoginError::Unknown),
                        }
                    });
                    commands.spawn(LoginTaskEntity(LoginTaskInner {
                        task,
                        remember,
                        cred_id,
                        server_id: server.id,
                        username: uname,
                        password: Some(pw),
                    }));
                } else {
                    println!(
                        "[webui] LoginSubmit: server id {} not found in settings",
                        server_id
                    );
                }
                outbound.write(UiOutbound(settings.to_snapshot_message(None)));
            }
            UiToCore::LoginUseSaved { id } => {
                println!("[webui] LoginUseSaved: id={}", id);
                let mut emitted_snapshot = false;
                let (cred_id, server_id, username) = {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_millis() as u64)
                        .unwrap_or(0);
                    if let Some(c) = settings.saved_credentials.iter_mut().find(|c| &c.id == id) {
                        c.last_used = now;
                        (c.id.clone(), c.server_id, c.username.clone())
                    } else {
                        println!("[webui] LoginUseSaved: credential id not found");
                        continue;
                    }
                };
                match keyring::get_password(&cred_id) {
                    Ok(password) => {
                        if let Some(server) =
                            settings.servers.iter().find(|s| s.id == server_id).cloned()
                        {
                            println!(
                                "[webui] LoginUseSaved: starting background login for server {}",
                                server_id
                            );
                            let uname = username.clone();
                            let pw_task = password.clone();
                            let uname_for_task = uname.clone();
                            let task: Task<
                                Result<
                                    (network::DecryptedReceiver, network::EncryptedSender),
                                    LoginError,
                                >,
                            > = IoTaskPool::get().spawn(async move {
                                let (host, port) = parse_host_port(&server.address)
                                    .unwrap_or((server.address.clone(), 2610));
                                match crate::session_prelogin::PreLoginSession::new(&host, port)
                                    .await
                                {
                                    Ok(lobby) => match lobby.login(&uname_for_task, &pw_task).await
                                    {
                                        Ok((rx, tx)) => Ok((rx, tx)),
                                        Err(code) => Err(code),
                                    },
                                    Err(_) => Err(LoginError::Unknown),
                                }
                            });
                            commands.spawn(LoginTaskEntity(LoginTaskInner {
                                task,
                                remember: false,
                                cred_id,
                                server_id,
                                username: uname,
                                password: None,
                            }));
                        } else {
                            println!(
                                "[webui] LoginUseSaved: server {} not found in settings",
                                server_id
                            );
                            outbound.write(UiOutbound(settings.to_snapshot_message(Some(
                                LoginError::Network("Server missing".to_string()),
                            ))));
                            emitted_snapshot = true;
                        }
                    }
                    Err(err) => {
                        println!(
                            "[webui] LoginUseSaved: keyring missing password for id={} ({}). Prompting user to re-enter.",
                            cred_id, err
                        );
                        outbound.write(UiOutbound(settings.to_snapshot_message(Some(
                            LoginError::Network("Missing saved password".to_string()),
                        ))));
                        emitted_snapshot = true;
                    }
                }
                if !emitted_snapshot {
                    let logins_public: Vec<SavedCredentialPublic> =
                        settings.saved_credentials.iter().map(to_public).collect();
                    outbound.write(UiOutbound(CoreToUi::Snapshot {
                        servers: settings.servers.clone(),
                        current_server_id: settings.gameplay.current_server_id,
                        logins: logins_public,
                        login_error: None,
                    }));
                }
            }
            UiToCore::LoginRemoveSaved { id } => {
                let _ = keyring::delete_password(id);
                settings.remove_credential(id);
                outbound.write(UiOutbound(settings.to_snapshot_message(None)));
            }
            UiToCore::ServersChangeCurrent { id } => {
                settings.gameplay.current_server_id = Some(*id);
                outbound.write(UiOutbound(settings.to_snapshot_message(None)));
            }
            UiToCore::ServersAdd { server } => {
                let new_id = next_id(settings.servers.iter().map(|s| s.id));
                settings.servers.push(ServerEntry {
                    id: new_id,
                    name: server.name.clone(),
                    address: server.address.clone(),
                });
                if settings.gameplay.current_server_id.is_none() {
                    settings.gameplay.current_server_id = Some(new_id);
                }
                outbound.write(UiOutbound(settings.to_snapshot_message(None)));
            }
            UiToCore::ServersEdit { server } => {
                if let Some(s) = settings.servers.iter_mut().find(|s| s.id == server.id) {
                    s.name = server.name.clone();
                    s.address = server.address.clone();
                }
                outbound.write(UiOutbound(settings.to_snapshot_message(None)));
            }
            UiToCore::ServersRemove { id } => {
                settings.servers.retain(|s| s.id != *id);
                if settings.gameplay.current_server_id == Some(*id) {
                    settings.gameplay.current_server_id = settings.servers.first().map(|s| s.id);
                }
                outbound.write(UiOutbound(settings.to_snapshot_message(None)));
            }
            UiToCore::SettingsChange { xray_size } => {
                settings.graphics.xray_size = crate::settings_types::XRaySize::from_u8(*xray_size);
            }
            UiToCore::VolumeChange { sfx, music } => {
                if let Some(sfx_vol) = sfx {
                    settings.audio.sfx_volume = *sfx_vol;
                }
                if let Some(music_vol) = music {
                    settings.audio.music_volume = *music_vol;
                }
            }
            UiToCore::ScaleChange { scale } => {
                settings.graphics.scale = *scale;
            }
            UiToCore::RebindKey {
                action,
                new_key,
                index,
            } => {
                use crate::input::{InputBindings, UnifiedInputBindings};
                let index = *index;

                // Conflict detection: if new_key is already bound to another action, clear that action's binding at that index
                macro_rules! check_conflict {
                    ($field:ident) => {
                        for k in settings.key_bindings.$field.iter_mut() {
                            if !new_key.is_empty() && k == new_key {
                                *k = "".to_string();
                            }
                        }
                    };
                }

                check_conflict!(move_up);
                check_conflict!(move_down);
                check_conflict!(move_left);
                check_conflict!(move_right);
                check_conflict!(inventory);
                check_conflict!(skills);
                check_conflict!(spells);
                check_conflict!(settings);
                check_conflict!(refresh);
                check_conflict!(basic_attack);
                check_conflict!(hotbar_slot_1);
                check_conflict!(hotbar_slot_2);
                check_conflict!(hotbar_slot_3);
                check_conflict!(hotbar_slot_4);
                check_conflict!(hotbar_slot_5);
                check_conflict!(hotbar_slot_6);
                check_conflict!(hotbar_slot_7);
                check_conflict!(hotbar_slot_8);
                check_conflict!(hotbar_slot_9);
                check_conflict!(hotbar_slot_10);
                check_conflict!(hotbar_slot_11);
                check_conflict!(hotbar_slot_12);
                check_conflict!(switch_to_inventory);
                check_conflict!(switch_to_skills);
                check_conflict!(switch_to_spells);
                check_conflict!(switch_to_hotbar_1);
                check_conflict!(switch_to_hotbar_2);
                check_conflict!(switch_to_hotbar_3);

                macro_rules! set_field {
                    ($field:ident) => {
                        if action == stringify!($field) {
                            settings.key_bindings.$field[index] = new_key.clone();
                        }
                    };
                }

                set_field!(move_up);
                set_field!(move_down);
                set_field!(move_left);
                set_field!(move_right);
                set_field!(inventory);
                set_field!(skills);
                set_field!(spells);
                set_field!(settings);
                set_field!(refresh);
                set_field!(basic_attack);
                set_field!(hotbar_slot_1);
                set_field!(hotbar_slot_2);
                set_field!(hotbar_slot_3);
                set_field!(hotbar_slot_4);
                set_field!(hotbar_slot_5);
                set_field!(hotbar_slot_6);
                set_field!(hotbar_slot_7);
                set_field!(hotbar_slot_8);
                set_field!(hotbar_slot_9);
                set_field!(hotbar_slot_10);
                set_field!(hotbar_slot_11);
                set_field!(hotbar_slot_12);
                set_field!(switch_to_inventory);
                set_field!(switch_to_skills);
                set_field!(switch_to_spells);
                set_field!(switch_to_hotbar_1);
                set_field!(switch_to_hotbar_2);
                set_field!(switch_to_hotbar_3);

                // Refresh the runtime bindings from the updated settings
                *unified_bindings = UnifiedInputBindings::from_settings(&settings.key_bindings);
                *input_bindings = InputBindings::from_settings(&settings.key_bindings);
            }
            UiToCore::UnbindKey { action, index } => {
                use crate::input::{InputBindings, UnifiedInputBindings};
                let index = *index;

                macro_rules! clear_field {
                    ($field:ident) => {
                        if action == stringify!($field) {
                            settings.key_bindings.$field[index] = "".to_string();
                        }
                    };
                }

                clear_field!(move_up);
                clear_field!(move_down);
                clear_field!(move_left);
                clear_field!(move_right);
                clear_field!(inventory);
                clear_field!(skills);
                clear_field!(spells);
                clear_field!(settings);
                clear_field!(refresh);
                clear_field!(basic_attack);
                clear_field!(hotbar_slot_1);
                clear_field!(hotbar_slot_2);
                clear_field!(hotbar_slot_3);
                clear_field!(hotbar_slot_4);
                clear_field!(hotbar_slot_5);
                clear_field!(hotbar_slot_6);
                clear_field!(hotbar_slot_7);
                clear_field!(hotbar_slot_8);
                clear_field!(hotbar_slot_9);
                clear_field!(hotbar_slot_10);
                clear_field!(hotbar_slot_11);
                clear_field!(hotbar_slot_12);
                clear_field!(switch_to_inventory);
                clear_field!(switch_to_skills);
                clear_field!(switch_to_spells);
                clear_field!(switch_to_hotbar_1);
                clear_field!(switch_to_hotbar_2);
                clear_field!(switch_to_hotbar_3);

                // Refresh the runtime bindings from the updated settings
                *unified_bindings = UnifiedInputBindings::from_settings(&settings.key_bindings);
                *input_bindings = InputBindings::from_settings(&settings.key_bindings);
            }
            _ => {}
        }
    }
}

fn bridge_chat_events(
    mut chat_events: MessageReader<ChatEvent>,
    mut outbound: MessageWriter<UiOutbound>,
    mut menu_ctx: ResMut<ActiveMenuContext>,
) {
    use packets::server::{PublicMessageType, ServerMessageType};

    let mut to_append: Vec<ChatEntryUi> = Vec::new();
    for evt in chat_events.read() {
        match evt {
            ChatEvent::ServerMessage(pkt) => {
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
                        menu_ctx.pursuit_id = 0;
                        menu_ctx.entity_type = None;
                        menu_ctx.entity_id = 0;

                        outbound.write(UiOutbound(CoreToUi::DisplayMenu {
                            title: title.to_string(),
                            text: pkt.message.clone(),
                            sprite_id: 0,
                            entry_type: crate::webui::ipc::MenuEntryType::TextOptions,
                            pursuit_id: 0,
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

fn bridge_session_events(
    mut session_events: MessageReader<SessionEvent>,
    mut outbound: MessageWriter<UiOutbound>,
    mut menu_ctx: ResMut<ActiveMenuContext>,
    inv_state: Res<InventoryState>,
    ability_state: Res<AbilityState>,
    mut profile_state: ResMut<PlayerProfileState>,
    mut show_profile: MessageWriter<crate::slint_plugin::ShowSelfProfileEvent>,
    mut world_list_state: ResMut<WorldListState>,
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
                        menu_ctx.pursuit_id = header.pursuit_id;
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
                                pursuit_id: header.pursuit_id,
                                entries,
                            }));
                        } else {
                            outbound.write(UiOutbound(CoreToUi::DisplayMenu {
                                title: header.name.clone(),
                                text: header.text.clone(),
                                sprite_id: header.sprite,
                                entry_type: crate::webui::ipc::MenuEntryType::TextOptions,
                                pursuit_id: header.pursuit_id,
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
                profile_state.group_string = pkt.group_string.clone();
                profile_state.group_open = pkt.group_open;
                profile_state.profile_text = pkt.group_string.clone();
                profile_state.legend_marks = pkt.legend_marks.clone();
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
                profile_state.profile_text = pkt.profile_text.clone().unwrap_or_default();
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
                        menu_ctx.pursuit_id = 0;
                        entries = options
                            .iter()
                            .map(|(text, id)| MenuEntryUi::text_option(text.clone(), *id as i32))
                            .collect();
                    }
                    DisplayMenuPayload::MenuWithArgs { args, options } => {
                        menu_ctx.pursuit_id = 0;
                        menu_ctx.args = args.clone();
                        entries = options
                            .iter()
                            .map(|(text, id)| MenuEntryUi::text_option(text.clone(), *id as i32))
                            .collect();
                    }
                    DisplayMenuPayload::ShowItems { pursuit_id, items } => {
                        menu_ctx.pursuit_id = *pursuit_id;
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
                        menu_ctx.pursuit_id = *pursuit_id;
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
                        menu_ctx.pursuit_id = *pursuit_id;
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
                        menu_ctx.pursuit_id = *pursuit_id;
                        is_text_entry = true;
                    }
                    DisplayMenuPayload::TextEntryWithArgs { args, pursuit_id } => {
                        menu_ctx.pursuit_id = *pursuit_id;
                        menu_ctx.args = args.clone();
                        is_text_entry = true;
                    }
                    DisplayMenuPayload::ShowPlayerItems { pursuit_id, slots } => {
                        menu_ctx.pursuit_id = *pursuit_id;
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
                        menu_ctx.pursuit_id = *pursuit_id;
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
                        menu_ctx.pursuit_id = *pursuit_id;
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
                        pursuit_id: menu_ctx.pursuit_id,
                        entries,
                    }));
                } else {
                    outbound.write(UiOutbound(CoreToUi::DisplayMenu {
                        title: pkt.header.name.clone(),
                        text: pkt.header.text.clone(),
                        sprite_id: pkt.header.sprite,
                        entry_type,
                        pursuit_id: menu_ctx.pursuit_id,
                        entries,
                    }));
                }
            }
            _ => {}
        }
    }
}

// Bridge inventory GameEvents to UI CoreToUi messages
fn bridge_inventory_events(
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

fn update_world_list_filtered(mut state: ResMut<WorldListState>, mut last_version: Local<u32>) {
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
                packets::server::WorldListColor::WithinLevelRange => [0.6, 0.6, 1.0, 1.0], // Blue-ish
                packets::server::WorldListColor::White => [1.0, 1.0, 1.0, 1.0],
                packets::server::WorldListColor::NotSure => [0.5, 0.5, 0.5, 1.0], // Gray
            },
        })
        .collect();
}

#[derive(Resource, Default, Debug, Clone)]
pub struct InventoryState(pub Vec<InventoryItemUi>);

#[derive(Resource, Default, Debug, Clone)]
pub struct EquipmentState(
    pub std::collections::HashMap<packets::server::EquipmentSlot, packets::server::Equipment>,
);

#[derive(Resource, Default, Debug, Clone)]
pub struct PlayerProfileState {
    pub entity_id: Option<u32>,
    pub is_self: bool,
    pub name: String,
    pub class: String,
    pub guild: String,
    pub guild_rank: String,
    pub title: String,
    pub nation: packets::types::Nation,
    pub group_open: bool,
    pub group_string: String,
    pub profile_text: String,
    pub legend_marks: Vec<packets::types::LegendMarkInfo>,
    pub portrait: Option<Vec<u8>>,
    pub equipment:
        std::collections::HashMap<packets::server::EquipmentSlot, packets::types::ItemInfo>,
}

impl PlayerProfileState {
    pub fn clear(&mut self) {
        let _nation = self.nation; // Keep current nation or just reset to default? Default is better.
        *self = Self::default();
    }
}

#[derive(Resource, Default, Debug, Clone)]
pub struct AbilityState {
    pub skills: Vec<SkillUi>,
    pub spells: Vec<SpellUi>,
}

#[derive(Resource, Default, Debug, Clone)]
pub struct WorldListState {
    pub raw: Option<packets::server::WorldList>,
    pub filtered: Vec<WorldListMemberUi>,
    pub filter: WorldListFilter,
    pub version: u32,
}

fn update_skill_cooldowns(
    time: Res<Time>,
    mut timer: Local<Timer>,
    mut state: ResMut<AbilityState>,
) {
    if timer.duration().is_zero() {
        *timer = Timer::from_seconds(0.1, TimerMode::Repeating);
    }

    if !timer.tick(time.delta()).just_finished() {
        return;
    }

    if !state.skills.iter().any(|s| s.on_cooldown.is_some()) {
        return;
    }

    let now = Instant::now();
    for skill in state.skills.iter_mut() {
        if let Some(cd) = &mut skill.on_cooldown {
            let time_left = cd
                .start_time
                .checked_add(cd.duration)
                .and_then(|end| end.checked_duration_since(now))
                .unwrap_or_default();

            if time_left.is_zero() {
                skill.on_cooldown = None;
            } else {
                cd.time_left = time_left;
            }
        }
    }
}

// Bridge skill/spell GameEvents to UI
fn bridge_ability_events(
    mut ability_events: MessageReader<AbilityEvent>,
    mut state: ResMut<AbilityState>,
) {
    for evt in ability_events.read() {
        match evt {
            AbilityEvent::SkillCooldown {
                slot,
                cooldown_secs,
            } => {
                let Some(skill) = state.skills.iter().find(|s| s.slot == *slot) else {
                    continue;
                };

                if Some(*cooldown_secs) == skill.cooldown_secs {
                    continue;
                }

                let Some(skill) = state.skills.iter_mut().find(|s| s.slot == *slot) else {
                    continue;
                };

                skill.cooldown_secs = Some(*cooldown_secs);
                skill.on_cooldown = Some(Cooldown::new(*cooldown_secs));
            }
            AbilityEvent::UseSkill { slot } => {
                let Some(skill) = state.skills.iter().find(|s| s.slot == *slot) else {
                    continue;
                };

                let Some(cd) = skill.cooldown_secs else {
                    continue;
                };

                let Some(skill) = state.skills.iter_mut().find(|s| s.slot == *slot) else {
                    continue;
                };

                skill.on_cooldown = Some(Cooldown::new(cd));
            }
            AbilityEvent::AddSkill(pkt) => {
                if let Some(existing) = state.skills.iter_mut().find(|s| s.slot == pkt.slot) {
                    existing.name = pkt.name.clone();
                    existing.sprite = pkt.sprite;

                    let new_id = ActionId::from_skill(pkt.sprite, &pkt.name);

                    if new_id != existing.id {
                        existing.id = new_id;
                        existing.cooldown_secs = None;
                    }
                } else {
                    state.skills.push(SkillUi {
                        slot: pkt.slot,
                        id: ActionId::from_skill(pkt.sprite, &pkt.name),
                        name: pkt.name.clone(),
                        sprite: pkt.sprite,
                        cooldown_secs: None,
                        on_cooldown: None,
                    });
                }
            }
            AbilityEvent::RemoveSkill(pkt) => {
                state.skills.retain(|s| s.slot != pkt.slot);
            }
            AbilityEvent::AddSpell(pkt) => {
                if let Some(existing) = state.spells.iter_mut().find(|s| s.slot == pkt.slot) {
                    existing.sprite = pkt.sprite;
                    existing.panel_name = pkt.panel_name.clone();
                    existing.prompt = pkt.prompt.clone();
                    existing.cast_lines = pkt.cast_lines;
                    existing.id = ActionId::from_spell(pkt.sprite, &pkt.panel_name);
                    existing.spell_type = pkt.spell_type;
                } else {
                    state.spells.push(SpellUi {
                        slot: pkt.slot,
                        sprite: pkt.sprite,
                        id: ActionId::from_spell(pkt.sprite, &pkt.panel_name),
                        panel_name: pkt.panel_name.clone(),
                        prompt: pkt.prompt.clone(),
                        cast_lines: pkt.cast_lines,
                        spell_type: pkt.spell_type,
                    });
                }
            }
            AbilityEvent::RemoveSpell(pkt) => {
                state.spells.retain(|s| s.slot != pkt.slot);
            }
            AbilityEvent::UseSpell { .. } => {}
        }
    }
}

fn handle_input_bridge(
    mut inbound: MessageReader<UiInbound>,
    mut kb: ResMut<ButtonInput<KeyCode>>,
    mut mb: ResMut<ButtonInput<MouseButton>>,
    mut cursor: ResMut<CursorPosition>,
    mut edges: ResMut<KeyboardEdges>,
) {
    for UiInbound(msg) in inbound.read() {
        match msg {
            UiToCore::InputKeyboard { action, code } => {
                if let Some(key) = dom_code_to_keycode(code) {
                    tracing::trace!("ui->core key {:?} {}", key, action);
                    if action == "down" {
                        kb.press(key);
                        edges.just_pressed.push(key);
                    } else if action == "up" {
                        kb.release(key);
                        edges.just_released.push(key);
                    }
                }
            }
            UiToCore::InputPointer {
                action,
                button,
                x,
                y,
                ..
            } => {
                cursor.x = *x;
                cursor.y = *y;
                if let Some(b) = button {
                    let btn = match b {
                        0 => MouseButton::Left,
                        1 => MouseButton::Middle,
                        2 => MouseButton::Right,
                        _ => MouseButton::Other(*b as u16),
                    };
                    tracing::trace!("ui->core pointer {:?} at ({:.1},{:.1})", action, x, y);
                    match action.as_str() {
                        "down" => mb.press(btn),
                        "up" => mb.release(btn),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
}

fn dom_code_to_keycode(code: &str) -> Option<KeyCode> {
    use KeyCode::*;
    Some(match code {
        // Arrow keys
        "ArrowUp" => ArrowUp,
        "ArrowDown" => ArrowDown,
        "ArrowLeft" => ArrowLeft,
        "ArrowRight" => ArrowRight,
        // WASD
        "KeyW" => KeyW,
        "KeyA" => KeyA,
        "KeyS" => KeyS,
        "KeyD" => KeyD,
        // Space/Enter/Escape
        "Space" => Space,
        "Enter" => Enter,
        "Escape" => Escape,
        // Digits
        "Digit0" => Digit0,
        "Digit1" => Digit1,
        "Digit2" => Digit2,
        "Digit3" => Digit3,
        "Digit4" => Digit4,
        "Digit5" => Digit5,
        "Digit6" => Digit6,
        "Digit7" => Digit7,
        "Digit8" => Digit8,
        "Digit9" => Digit9,
        // Letters
        "KeyQ" => KeyQ,
        "KeyE" => KeyE,
        "KeyR" => KeyR,
        "KeyF" => KeyF,
        "KeyZ" => KeyZ,
        "KeyX" => KeyX,
        "KeyC" => KeyC,
        _ => return None,
    })
}

fn clear_just_input(
    mut kb: ResMut<ButtonInput<KeyCode>>,
    _: ResMut<ButtonInput<MouseButton>>,
    edges: Res<KeyboardEdges>,
) {
    for &k in &edges.just_pressed {
        kb.clear_just_pressed(k);
    }
    for &k in &edges.just_released {
        kb.clear_just_released(k);
    }
}

fn clear_input_edges(mut edges: ResMut<KeyboardEdges>) {
    edges.just_pressed.clear();
    edges.just_released.clear();
}

fn next_id(mut iter: impl Iterator<Item = u32>) -> u32 {
    let mut max = 0u32;
    while let Some(v) = iter.next() {
        if v > max {
            max = v;
        }
    }
    max.saturating_add(1)
}

// Helpers and login task pipeline

fn to_public(c: &SavedCredential) -> SavedCredentialPublic {
    SavedCredentialPublic {
        id: c.id.clone(),
        server_id: c.server_id,
        username: c.username.clone(),
        last_used: c.last_used,
        preview: c.preview.clone(),
    }
}

#[derive(Component)]
struct LoginTaskEntity(LoginTaskInner);

struct LoginTaskInner {
    task: Task<Result<(network::DecryptedReceiver, network::EncryptedSender), LoginError>>,
    remember: bool,
    cred_id: String,
    server_id: u32,
    username: String,
    password: Option<String>,
}

#[derive(Component)]
struct LoginResultComp(
    Option<network::DecryptedReceiver>,
    Option<network::EncryptedSender>,
    Option<LoginTaskInner>,
);

#[derive(Component)]
struct LoginErrorComp(LoginError, LoginTaskInner);

fn handle_login_tasks(mut commands: Commands, mut q: Query<(Entity, &mut LoginTaskEntity)>) {
    for (e, mut task_wrap) in &mut q {
        if let Some(res) = future::block_on(future::poll_once(&mut task_wrap.0.task)) {
            println!("[webui] LoginTask completed: success={}.", res.is_ok());
            let inner = std::mem::replace(
                &mut task_wrap.0,
                LoginTaskInner {
                    task: IoTaskPool::get().spawn(async { Err(LoginError::Unknown) }),
                    remember: false,
                    cred_id: String::new(),
                    server_id: 0,
                    username: String::new(),
                    password: None,
                },
            );

            match res {
                Ok((rx, tx)) => {
                    commands.spawn(LoginResultComp(Some(rx), Some(tx), Some(inner)));
                }
                Err(err) => {
                    commands.spawn(LoginErrorComp(err, inner));
                }
            }
            commands.entity(e).despawn();
        }
    }
}

fn parse_host_port(address: &str) -> Option<(String, u16)> {
    let mut parts = address.split(':');
    let host = parts.next()?.to_string();
    let port = parts
        .next()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(2610);
    Some((host, port))
}

fn handle_login_results(
    mut commands: Commands,
    mut success_q: Query<(Entity, &mut LoginResultComp)>,
    mut error_q: Query<(Entity, &mut LoginErrorComp)>,
    mut outbound: MessageWriter<UiOutbound>,
    mut settings: ResMut<SettingsFile>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    for (e, mut res) in &mut success_q {
        let (receiver, sender, inner) = {
            let LoginResultComp(rx, tx, inner) = &mut *res;
            (rx.take(), tx.take(), inner.take())
        };

        if receiver.is_none() || sender.is_none() || inner.is_none() {
            continue;
        }
        let receiver: network::DecryptedReceiver = receiver.unwrap();
        let sender: network::EncryptedSender = sender.unwrap();
        let inner: LoginTaskInner = inner.unwrap();

        commands.entity(e).despawn();

        println!(
            "[webui] LoginResult: success for user {} on server {}",
            inner.username, inner.server_id
        );
        // Spawn the background receiver task piping into NetEventRx
        use crate::session::runtime::{NetBgTask, NetEventRx};
        let (tx, rx) = crossbeam_channel::unbounded::<crate::events::NetworkEvent>();
        commands.insert_resource(NetEventRx(rx));

        let (outbox_tx, outbox_rx) = async_channel::unbounded::<Vec<u8>>();
        commands.insert_resource(crate::network::PacketOutbox(outbox_tx.clone()));

        let tx_for_task = tx.clone();
        let mut rx_loop = receiver;

        let reader_task = IoTaskPool::get().spawn(async move {
            loop {
                match rx_loop.receive().await {
                    Ok((packet_id, packet_data)) => {
                        use packets::server;
                        if let Ok(code) = server::Codes::try_from(packet_id) {
                            let _ = tx_for_task
                                .send(crate::events::NetworkEvent::Packet(code, packet_data));
                        }
                    }
                    Err(_) => {
                        let _ = tx_for_task.send(crate::events::NetworkEvent::Disconnected);
                        break;
                    }
                }
            }
        });

        // Spawn the background writer task on the IoTaskPool
        let mut tx_loop = sender;
        let writer_task = IoTaskPool::get().spawn(async move {
            while let Ok(packet) = outbox_rx.recv().await {
                if let Err(_) = tx_loop.send(&packet).await {
                    break;
                }
                while let Ok(extra_packet) = outbox_rx.try_recv() {
                    if let Err(_) = tx_loop.send(&extra_packet).await {
                        return;
                    }
                }
                let _ = tx_loop.flush().await;
            }
        });

        commands.spawn(NetBgTask(reader_task));
        commands.spawn(NetBgTask(writer_task));

        // Emit connected event to seed tick timers
        let _ = tx.send(crate::events::NetworkEvent::Connected);
        // On success, if remember was requested, persist cred and password
        if inner.remember {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            if let Some(pw) = &inner.password {
                let _ = keyring::set_password(&inner.cred_id, pw);
            }
            // Upsert saved credential record
            if let Some(existing) = settings
                .saved_credentials
                .iter_mut()
                .find(|c| c.id == inner.cred_id)
            {
                existing.last_used = now;
                existing.username = inner.username.clone();
                existing.server_id = inner.server_id;
            } else {
                settings.saved_credentials.push(SavedCredential {
                    id: inner.cred_id.clone(),
                    server_id: inner.server_id,
                    username: inner.username.clone(),
                    last_used: now,
                    preview: None,
                });
            }
        }
        let server_url = settings
            .servers
            .iter()
            .find(|s| s.id == inner.server_id)
            .map(|s| s.address.clone())
            .unwrap_or_default();

        commands.insert_resource(crate::CurrentSession {
            username: inner.username.clone(),
            server_id: inner.server_id,
            server_url,
        });

        let hotbars = settings.get_hotbars(inner.server_id, &inner.username);
        let mut hotbar_state = crate::ecs::hotbar::HotbarState::new();
        hotbar_state.config = hotbars;
        commands.insert_resource(hotbar_state);
        commands.insert_resource(crate::ecs::hotbar::HotbarPanelState::default());

        next_state.set(AppState::InGame);
        outbound.write(UiOutbound(CoreToUi::EnteredGame));
    }

    for (e, err) in &mut error_q {
        println!(
            "[webui] LoginResult: failed with code {:?} for user {} on server {}",
            err.0, err.1.username, err.1.server_id
        );
        // Login failed: keep user on the current screen (login) and emit error
        let logins_public: Vec<SavedCredentialPublic> =
            settings.saved_credentials.iter().map(to_public).collect();
        outbound.write(UiOutbound(CoreToUi::Snapshot {
            servers: settings.servers.clone(),
            current_server_id: settings.gameplay.current_server_id,
            logins: logins_public,
            login_error: Some(err.0.clone()),
        }));
        commands.entity(e).despawn();
    }
}

fn sync_settings_to_ui(settings: Res<SettingsFile>, mut outbound: MessageWriter<UiOutbound>) {
    if settings.is_changed() {
        outbound.write(UiOutbound(settings.to_sync_message()));
    }
}
