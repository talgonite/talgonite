use crate::{
    ecs::components::{Direction, LocalPlayer, MovementTween},
    ecs::spell_casting::SpellCastingState,
    ecs::systems::GameSet,
    events::{InputSource, PlayerAction},
    input::{
        GameAction, GamepadConfig, GilrsResource, InputBindings, RebindingState,
        UnifiedInputBindings, gamepad_rebinding_system, sync_rebinding_state_from_slint,
    },
    network::PacketOutbox,
    settings_types::Settings,
};
use bevy::prelude::*;
use game_types::SlotPanelType;
use packets::client::{RefreshRequest, Spacebar};

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InputPumpSet;

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InputTimer>()
            .init_resource::<GamepadConfig>()
            .init_resource::<GilrsResource>()
            .init_resource::<RebindingState>()
            .init_resource::<UnifiedInputBindings>()
            .add_message::<bevy::input::mouse::MouseWheel>()
            .add_message::<bevy::input::gamepad::RawGamepadEvent>()
            .add_systems(Startup, initialize_input_bindings)
            .add_systems(PreUpdate, crate::input::gamepad::gilrs_event_polling_system)
            .add_systems(
                Update,
                (
                    crate::slint_support::input_bridge::pump_slint_key_events_system,
                    crate::slint_support::input_bridge::pump_slint_pointer_events_system,
                    crate::slint_support::input_bridge::pump_slint_scroll_events_system,
                    pump_double_clicks_system,
                )
                    .chain()
                    .in_set(InputPumpSet),
            )
            .add_systems(
                Update,
                (
                    sync_rebinding_state_from_slint,
                    gamepad_rebinding_system,
                    crate::input::gamepad::gamepad_connection_system,
                    input_handling_system,
                )
                    .chain()
                    .after(InputPumpSet)
                    .in_set(GameSet::EventProcessing),
            );
    }
}

pub fn pump_double_clicks_system(
    queue: Res<crate::slint_support::input_bridge::SlintDoubleClickQueue>,
    mut events: MessageWriter<crate::slint_plugin::SlintDoubleClickEvent>,
) {
    let raw_events: Vec<(f32, f32)> = {
        let Ok(mut guard) = queue.0.lock() else {
            return;
        };
        guard.drain(..).collect()
    };

    for (x, y) in raw_events {
        events.write(crate::slint_plugin::SlintDoubleClickEvent(x, y));
    }
}

#[derive(Resource)]
pub struct InputTimer {
    walk_cd: Timer,            // gates actual movement (walk)
    primed: bool,              // first walk allowed immediately
    turn_grace: Option<Timer>, // suppress walking right after a facing change
}

impl Default for InputTimer {
    fn default() -> Self {
        Self {
            walk_cd: Timer::from_seconds(0.0, TimerMode::Once), // finished immediately
            primed: true,
            turn_grace: None,
        }
    }
}

impl InputTimer {
    pub fn walk_cd_finished(&self) -> bool {
        self.walk_cd.is_finished()
    }
}

fn initialize_input_bindings(
    mut commands: Commands,
    settings: Res<Settings>,
    mut unified: ResMut<UnifiedInputBindings>,
) {
    let bindings = InputBindings::from_settings(&settings.key_bindings);
    commands.insert_resource(bindings);

    for action in GameAction::all() {
        if let Some(code) = match action {
            GameAction::MoveUp => Some(&settings.key_bindings.move_up),
            GameAction::MoveDown => Some(&settings.key_bindings.move_down),
            GameAction::MoveLeft => Some(&settings.key_bindings.move_left),
            GameAction::MoveRight => Some(&settings.key_bindings.move_right),
            GameAction::Inventory => Some(&settings.key_bindings.inventory),
            GameAction::Skills => Some(&settings.key_bindings.skills),
            GameAction::Spells => Some(&settings.key_bindings.spells),
            GameAction::Settings => Some(&settings.key_bindings.settings),
            GameAction::Refresh => Some(&settings.key_bindings.refresh),
            GameAction::BasicAttack => Some(&settings.key_bindings.basic_attack),
            GameAction::HotbarSlot1 => Some(&settings.key_bindings.hotbar_slot_1),
            GameAction::HotbarSlot2 => Some(&settings.key_bindings.hotbar_slot_2),
            GameAction::HotbarSlot3 => Some(&settings.key_bindings.hotbar_slot_3),
            GameAction::HotbarSlot4 => Some(&settings.key_bindings.hotbar_slot_4),
            GameAction::HotbarSlot5 => Some(&settings.key_bindings.hotbar_slot_5),
            GameAction::HotbarSlot6 => Some(&settings.key_bindings.hotbar_slot_6),
            GameAction::HotbarSlot7 => Some(&settings.key_bindings.hotbar_slot_7),
            GameAction::HotbarSlot8 => Some(&settings.key_bindings.hotbar_slot_8),
            GameAction::HotbarSlot9 => Some(&settings.key_bindings.hotbar_slot_9),
            GameAction::HotbarSlot10 => Some(&settings.key_bindings.hotbar_slot_10),
            GameAction::HotbarSlot11 => Some(&settings.key_bindings.hotbar_slot_11),
            GameAction::HotbarSlot12 => Some(&settings.key_bindings.hotbar_slot_12),
            GameAction::SwitchToInventory => Some(&settings.key_bindings.switch_to_inventory),
            GameAction::SwitchToSkills => Some(&settings.key_bindings.switch_to_skills),
            GameAction::SwitchToSpells => Some(&settings.key_bindings.switch_to_spells),
            GameAction::SwitchToHotbar1 => Some(&settings.key_bindings.switch_to_hotbar_1),
            GameAction::SwitchToHotbar2 => Some(&settings.key_bindings.switch_to_hotbar_2),
            GameAction::SwitchToHotbar3 => Some(&settings.key_bindings.switch_to_hotbar_3),
        } {
            if let Some(input_source) = crate::input::InputSource::from_string(code) {
                unified.set_binding(*action, input_source);
            }
        }
    }
}

pub fn input_handling_system(
    time: Res<Time>,
    mut input_timer: ResMut<InputTimer>,
    keyboard_input: Option<Res<ButtonInput<KeyCode>>>,
    unified_bindings: Option<Res<UnifiedInputBindings>>,
    gamepad_query: Query<&Gamepad>,
    gamepad_config: Res<GamepadConfig>,
    mut player_actions: MessageWriter<PlayerAction>,
    mut player_query: Query<
        (&mut LocalPlayer, &mut Direction, Option<&MovementTween>),
        With<LocalPlayer>,
    >,
    outbox: Option<Res<PacketOutbox>>,
    mut hotbar_panel_state: Option<ResMut<crate::ecs::hotbar::HotbarPanelState>>,
    mut ui_inbound: MessageWriter<crate::webui::plugin::UiInbound>,
    mut inventory_events: MessageWriter<crate::events::InventoryEvent>,
    mut ability_events: MessageWriter<crate::events::AbilityEvent>,
    mut spell_casting: ResMut<SpellCastingState>,
) {
    if keyboard_input.is_none() {
        // tracing::warn!("keyboard_input is None");
        return;
    }
    let keyboard_input = keyboard_input.unwrap();

    if unified_bindings.is_none() {
        // tracing::warn!("unified_bindings is None");
        return;
    }
    let bindings = unified_bindings.unwrap();

    if bindings.is_just_pressed(
        GameAction::Refresh,
        &keyboard_input,
        Some(&gamepad_query),
        Some(&gamepad_config),
    ) {
        tracing::info!("Refresh triggered");
        if let Some(outbox) = &outbox {
            outbox.send(&RefreshRequest);
        }
    }

    if bindings.is_just_pressed(
        GameAction::BasicAttack,
        &keyboard_input,
        Some(&gamepad_query),
        Some(&gamepad_config),
    ) {
        tracing::info!("Basic attack triggered");
        spell_casting.active_cast = None;
        if let Some(outbox) = &outbox {
            outbox.send(&Spacebar);
        }
    }

    // Panel switching
    let panel_actions = [
        (
            GameAction::SwitchToInventory,
            crate::ecs::hotbar::HotbarPanel::Inventory,
        ),
        (
            GameAction::SwitchToSkills,
            crate::ecs::hotbar::HotbarPanel::Skills,
        ),
        (
            GameAction::SwitchToSpells,
            crate::ecs::hotbar::HotbarPanel::Spells,
        ),
        (
            GameAction::SwitchToHotbar1,
            crate::ecs::hotbar::HotbarPanel::Hotbar1,
        ),
        (
            GameAction::SwitchToHotbar2,
            crate::ecs::hotbar::HotbarPanel::Hotbar2,
        ),
        (
            GameAction::SwitchToHotbar3,
            crate::ecs::hotbar::HotbarPanel::Hotbar3,
        ),
    ];

    for (action, panel) in &panel_actions {
        if bindings.is_just_pressed(
            *action,
            &keyboard_input,
            Some(&gamepad_query),
            Some(&gamepad_config),
        ) {
            if let Some(ref mut state) = hotbar_panel_state {
                state.current_panel = *panel;
            }
        }
    }

    // Hotbar slot activation
    let slot_actions = [
        GameAction::HotbarSlot1,
        GameAction::HotbarSlot2,
        GameAction::HotbarSlot3,
        GameAction::HotbarSlot4,
        GameAction::HotbarSlot5,
        GameAction::HotbarSlot6,
        GameAction::HotbarSlot7,
        GameAction::HotbarSlot8,
        GameAction::HotbarSlot9,
        GameAction::HotbarSlot10,
        GameAction::HotbarSlot11,
        GameAction::HotbarSlot12,
    ];

    for (i, action) in slot_actions.iter().enumerate() {
        if bindings.is_just_pressed(
            *action,
            &keyboard_input,
            Some(&gamepad_query),
            Some(&gamepad_config),
        ) {
            if let Some(ref state) = hotbar_panel_state {
                let panel = state.current_panel as u8;
                let slot_index = match panel {
                    0..=2 => i,
                    3..=5 => (panel - 3) as usize * 12 + i,
                    _ => continue,
                };

                let category = match panel {
                    0 => SlotPanelType::Item,
                    1 => SlotPanelType::Skill,
                    2 => SlotPanelType::Spell,
                    3..=5 => SlotPanelType::Hotbar,
                    _ => continue,
                };

                match category {
                    SlotPanelType::Item => {
                        inventory_events.write(crate::events::InventoryEvent::Use {
                            slot: (slot_index + 1) as u8,
                        });
                    }
                    SlotPanelType::Skill => {
                        ability_events.write(crate::events::AbilityEvent::UseSkill {
                            slot: (slot_index + 1) as u8,
                        });
                    }
                    SlotPanelType::Spell => {
                        ability_events.write(crate::events::AbilityEvent::UseSpell {
                            slot: (slot_index + 1) as u8,
                        });
                    }
                    SlotPanelType::Hotbar => {
                        ui_inbound.write(crate::webui::plugin::UiInbound(
                            crate::webui::ipc::UiToCore::ActivateAction {
                                category,
                                index: slot_index,
                            },
                        ));
                    }
                    SlotPanelType::World => {}
                    SlotPanelType::None => {}
                }
            }
        }
    }

    let movement_actions = [
        GameAction::MoveUp,
        GameAction::MoveDown,
        GameAction::MoveLeft,
        GameAction::MoveRight,
    ];

    if let Ok((_, mut current_direction, active_tween)) = player_query.single_mut() {
        input_timer.walk_cd.tick(time.delta());
        if let Some(grace) = input_timer.turn_grace.as_mut() {
            grace.tick(time.delta());
        }

        if active_tween.is_some() {
            return;
        }

        let any_pressed = bindings.any_pressed(
            &movement_actions,
            &keyboard_input,
            Some(&gamepad_query),
            Some(&gamepad_config),
        );
        let any_just_pressed = bindings.any_just_pressed(
            &movement_actions,
            &keyboard_input,
            Some(&gamepad_query),
            Some(&gamepad_config),
        );
        if !any_pressed {
            return;
        }

        let pressed_direction = if bindings.is_pressed(
            GameAction::MoveUp,
            &keyboard_input,
            Some(&gamepad_query),
            Some(&gamepad_config),
        ) {
            Some(Direction::Up)
        } else if bindings.is_pressed(
            GameAction::MoveDown,
            &keyboard_input,
            Some(&gamepad_query),
            Some(&gamepad_config),
        ) {
            Some(Direction::Down)
        } else if bindings.is_pressed(
            GameAction::MoveLeft,
            &keyboard_input,
            Some(&gamepad_query),
            Some(&gamepad_config),
        ) {
            Some(Direction::Left)
        } else if bindings.is_pressed(
            GameAction::MoveRight,
            &keyboard_input,
            Some(&gamepad_query),
            Some(&gamepad_config),
        ) {
            Some(Direction::Right)
        } else {
            None
        };

        if let Some(new_direction) = pressed_direction {
            const WALK_COOLDOWN_SECS: f32 = 0.5;
            if *current_direction != new_direction {
                player_actions.write(PlayerAction::Turn {
                    direction: new_direction as u8,
                    source: InputSource::Manual,
                });
                *current_direction = new_direction;
                input_timer.turn_grace = Some(Timer::from_seconds(0.12, TimerMode::Once));
            }

            if *current_direction == new_direction {
                let in_grace = input_timer
                    .turn_grace
                    .as_ref()
                    .map(|t| !t.is_finished())
                    .unwrap_or(false);
                if !in_grace {
                    let walk_ready = input_timer.primed || input_timer.walk_cd.is_finished();
                    if walk_ready && (any_just_pressed || input_timer.walk_cd.is_finished()) {
                        player_actions.write(PlayerAction::Walk {
                            direction: new_direction as u8,
                            source: InputSource::Manual,
                        });
                        input_timer.walk_cd =
                            Timer::from_seconds(WALK_COOLDOWN_SECS, TimerMode::Once);
                        input_timer.primed = false;
                    }
                }
            }
        }
    }
}
