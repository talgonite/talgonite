use crate::{
    app_state::AppState,
    ecs::components::{Direction, LocalPlayer, MovementTween},
    ecs::systems::GameSet,
    events::{ClickSource, InputSource, PlayerAction, ResolvedPointerClickEvent},
    input::{
        GameAction, GamepadConfig, GilrsResource, InputBindings, RebindingState,
        UnifiedInputBindings, gamepad_rebinding_system, sync_rebinding_state_from_slint,
    },
    network::PacketOutbox,
    settings_types::Settings,
};
use bevy::prelude::*;
use game_types::SlotPanelType;
use packets::client::RefreshRequest;
use std::time::Duration;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InputPumpSet;

pub struct InputPlugin;

const HOTBAR_SLOT_ACTIONS: [GameAction; 48] = [
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
    GameAction::HotbarSlot13,
    GameAction::HotbarSlot14,
    GameAction::HotbarSlot15,
    GameAction::HotbarSlot16,
    GameAction::HotbarSlot17,
    GameAction::HotbarSlot18,
    GameAction::HotbarSlot19,
    GameAction::HotbarSlot20,
    GameAction::HotbarSlot21,
    GameAction::HotbarSlot22,
    GameAction::HotbarSlot23,
    GameAction::HotbarSlot24,
    GameAction::HotbarSlot25,
    GameAction::HotbarSlot26,
    GameAction::HotbarSlot27,
    GameAction::HotbarSlot28,
    GameAction::HotbarSlot29,
    GameAction::HotbarSlot30,
    GameAction::HotbarSlot31,
    GameAction::HotbarSlot32,
    GameAction::HotbarSlot33,
    GameAction::HotbarSlot34,
    GameAction::HotbarSlot35,
    GameAction::HotbarSlot36,
    GameAction::HotbarSlot37,
    GameAction::HotbarSlot38,
    GameAction::HotbarSlot39,
    GameAction::HotbarSlot40,
    GameAction::HotbarSlot41,
    GameAction::HotbarSlot42,
    GameAction::HotbarSlot43,
    GameAction::HotbarSlot44,
    GameAction::HotbarSlot45,
    GameAction::HotbarSlot46,
    GameAction::HotbarSlot47,
    GameAction::HotbarSlot48,
];

fn hotbar_panel_category(panel: crate::ecs::hotbar::HotbarPanel) -> Option<SlotPanelType> {
    match panel {
        crate::ecs::hotbar::HotbarPanel::Inventory => Some(SlotPanelType::Item),
        crate::ecs::hotbar::HotbarPanel::Skills => Some(SlotPanelType::Skill),
        crate::ecs::hotbar::HotbarPanel::Spells => Some(SlotPanelType::Spell),
        crate::ecs::hotbar::HotbarPanel::Hotbar1
        | crate::ecs::hotbar::HotbarPanel::Hotbar2
        | crate::ecs::hotbar::HotbarPanel::Hotbar3 => Some(SlotPanelType::Hotbar),
    }
}

fn hotbar_panel_base_offset(panel_state: &crate::ecs::hotbar::HotbarPanelState) -> usize {
    let panel = panel_state.current_panel as u8;
    let expanded_custom = panel_state.rows != crate::ecs::hotbar::HotbarRows::One
        && panel_state.current_panel.is_custom();

    match panel {
        0..=2 => 0,
        3..=5 if expanded_custom => 0,
        3..=5 => (panel - 3) as usize * 12,
        _ => 0,
    }
}

fn resolve_hotbar_slot_target(
    slot_index: usize,
    panel_state: &crate::ecs::hotbar::HotbarPanelState,
    settings: &Settings,
) -> Option<(SlotPanelType, usize)> {
    let row = slot_index / 12;
    let column = slot_index % 12;
    let active_category = hotbar_panel_category(panel_state.current_panel)?;
    let active_base = hotbar_panel_base_offset(panel_state);

    if row == 0 {
        return Some((active_category, active_base + column));
    }

    if settings.gameplay.modifier_hotbar_rows_target_custom_only {
        return Some((SlotPanelType::Hotbar, row * 12 + column));
    }

    Some((active_category, active_base + (row * 12) + column))
}

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InputTimer>()
            .init_resource::<AndroidTouchInputState>()
            .init_resource::<GamepadConfig>()
            .init_resource::<GilrsResource>()
            .init_resource::<RebindingState>()
            .init_resource::<UnifiedInputBindings>()
            .add_message::<bevy::input::mouse::MouseWheel>()
            .add_message::<bevy::input::gamepad::RawGamepadEvent>()
            .add_message::<crate::slint_support::input_bridge::SlintPointerEvent>()
            .add_systems(Startup, initialize_input_bindings)
            .add_systems(PreUpdate, crate::input::gamepad::gilrs_event_polling_system)
            .add_systems(
                Update,
                (
                    crate::slint_support::input_bridge::pump_slint_key_events_system,
                    crate::slint_support::input_bridge::pump_slint_pointer_events_system,
                    resolve_android_touch_events_system,
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
                    input_handling_system.run_if(in_state(AppState::InGame)),
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

const ANDROID_LONG_PRESS_THRESHOLD: Duration = Duration::from_millis(500);
const ANDROID_LONG_PRESS_SLOP: f32 = 12.0;

#[derive(Resource, Default)]
struct AndroidTouchInputState {
    active_press: Option<AndroidTouchPress>,
}

#[derive(Debug, Clone, Copy)]
struct AndroidTouchPress {
    start: Duration,
    start_position: (f32, f32),
    last_position: (f32, f32),
    moved_too_far: bool,
    long_press_fired: bool,
}

impl AndroidTouchInputState {
    fn pointer_moved_too_far(start: (f32, f32), current: (f32, f32)) -> bool {
        let dx = current.0 - start.0;
        let dy = current.1 - start.1;
        (dx * dx) + (dy * dy) > ANDROID_LONG_PRESS_SLOP * ANDROID_LONG_PRESS_SLOP
    }

    fn begin_press(&mut self, now: Duration, position: (f32, f32)) {
        self.active_press = Some(AndroidTouchPress {
            start: now,
            start_position: position,
            last_position: position,
            moved_too_far: false,
            long_press_fired: false,
        });
    }

    fn update_press(&mut self, position: (f32, f32)) {
        let Some(press) = self.active_press.as_mut() else {
            return;
        };

        press.last_position = position;
        press.moved_too_far |= Self::pointer_moved_too_far(press.start_position, position);
    }

    fn maybe_fire_long_press(&mut self, now: Duration) -> Option<ResolvedPointerClickEvent> {
        let press = self.active_press.as_mut()?;

        if press.long_press_fired || press.moved_too_far {
            return None;
        }

        if now.saturating_sub(press.start) < ANDROID_LONG_PRESS_THRESHOLD {
            return None;
        }

        press.long_press_fired = true;
        Some(ResolvedPointerClickEvent {
            position: press.last_position,
            button: MouseButton::Right,
            source: ClickSource::AndroidLongPress,
        })
    }

    fn release_press(&mut self, position: (f32, f32)) -> Option<ResolvedPointerClickEvent> {
        self.update_press(position);

        let press = self.active_press.take()?;
        if press.long_press_fired || press.moved_too_far {
            return None;
        }

        Some(ResolvedPointerClickEvent {
            position,
            button: MouseButton::Left,
            source: ClickSource::AndroidShortPress,
        })
    }

    fn cancel_press(&mut self) {
        self.active_press = None;
    }
}

fn resolve_android_touch_events_system(
    time: Res<Time>,
    mut pointer_events: MessageReader<crate::slint_support::input_bridge::SlintPointerEvent>,
    mut touch_state: ResMut<AndroidTouchInputState>,
    mut resolved_clicks: MessageWriter<ResolvedPointerClickEvent>,
) {
    if !cfg!(target_os = "android") {
        return;
    }

    let now = time.elapsed();

    if let Some(event) = touch_state.maybe_fire_long_press(now) {
        resolved_clicks.write(event);
    }

    for event in pointer_events.read() {
        match event.0.kind {
            i_slint_core::items::PointerEventKind::Down => {
                touch_state.begin_press(now, event.0.position);
            }
            i_slint_core::items::PointerEventKind::Move => {
                touch_state.update_press(event.0.position);
                if let Some(event) = touch_state.maybe_fire_long_press(now) {
                    resolved_clicks.write(event);
                }
            }
            i_slint_core::items::PointerEventKind::Up => {
                if let Some(event) = touch_state.release_press(event.0.position) {
                    resolved_clicks.write(event);
                }
            }
            i_slint_core::items::PointerEventKind::Cancel => {
                touch_state.cancel_press();
            }
            _ => {}
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::slint_support::input_bridge::QueuedPointerEvent;
    use i_slint_core::items::{PointerEventButton, PointerEventKind};

    fn pointer_event(kind: PointerEventKind, position: (f32, f32)) -> QueuedPointerEvent {
        QueuedPointerEvent {
            kind,
            button: PointerEventButton::Left,
            position,
        }
    }

    #[test]
    fn short_press_resolves_to_left_click() {
        let mut state = AndroidTouchInputState::default();
        state.begin_press(Duration::ZERO, (10.0, 20.0));

        let resolved = state.release_press((10.0, 20.0)).unwrap();
        assert_eq!(resolved.button, MouseButton::Left);
        assert_eq!(resolved.source, ClickSource::AndroidShortPress);
    }

    #[test]
    fn long_press_resolves_once_to_right_click() {
        let mut state = AndroidTouchInputState::default();
        let event = pointer_event(PointerEventKind::Down, (10.0, 20.0));
        state.begin_press(Duration::ZERO, event.position);

        let resolved = state
            .maybe_fire_long_press(ANDROID_LONG_PRESS_THRESHOLD)
            .unwrap();
        assert_eq!(resolved.button, MouseButton::Right);
        assert_eq!(resolved.source, ClickSource::AndroidLongPress);

        assert!(
            state
                .maybe_fire_long_press(ANDROID_LONG_PRESS_THRESHOLD + Duration::from_millis(1))
                .is_none()
        );
    }

    #[test]
    fn movement_cancels_long_press() {
        let mut state = AndroidTouchInputState::default();
        state.begin_press(Duration::ZERO, (10.0, 20.0));
        state.update_press((40.0, 60.0));

        assert!(
            state
                .maybe_fire_long_press(ANDROID_LONG_PRESS_THRESHOLD + Duration::from_millis(1))
                .is_none()
        );
        assert!(state.release_press((40.0, 60.0)).is_none());
    }

    #[test]
    fn release_after_long_press_does_not_emit_short_press() {
        let mut state = AndroidTouchInputState::default();
        state.begin_press(Duration::ZERO, (10.0, 20.0));
        assert!(
            state
                .maybe_fire_long_press(ANDROID_LONG_PRESS_THRESHOLD + Duration::from_millis(1))
                .is_some()
        );

        assert!(state.release_press((10.0, 20.0)).is_none());
    }

    #[test]
    fn modifier_rows_target_next_hotbar_row() {
        let settings = Settings::default();
        let panel_state = crate::ecs::hotbar::HotbarPanelState {
            current_panel: crate::ecs::hotbar::HotbarPanel::Inventory,
            rows: crate::ecs::hotbar::HotbarRows::Three,
        };

        let resolved = resolve_hotbar_slot_target(12, &panel_state, &settings);

        assert_eq!(resolved, Some((SlotPanelType::Hotbar, 12)));
    }
}

fn initialize_input_bindings(
    mut commands: Commands,
    settings: Res<Settings>,
    mut unified: ResMut<UnifiedInputBindings>,
) {
    let bindings = InputBindings::from_settings(&settings.key_bindings);
    commands.insert_resource(bindings);

    *unified = UnifiedInputBindings::from_settings(&settings.key_bindings);
}

pub fn input_handling_system(
    time: Res<Time>,
    mut input_timer: ResMut<InputTimer>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    unified_bindings: Res<UnifiedInputBindings>,
    settings: Res<Settings>,
    gamepad_query: Query<&Gamepad>,
    gamepad_config: Res<GamepadConfig>,
    minimap_renderer_state: Option<ResMut<crate::resources::MinimapRendererState>>,
    window: Option<Res<crate::slint_support::state_bridge::SlintWindow>>,
    mut player_actions: MessageWriter<PlayerAction>,
    mut player_query: Query<
        (&mut LocalPlayer, &mut Direction, Option<&MovementTween>),
        With<LocalPlayer>,
    >,
    outbox: Res<PacketOutbox>,
    mut hotbar_panel_state: ResMut<crate::ecs::hotbar::HotbarPanelState>,
    mut ui_inbound: MessageWriter<crate::webui::plugin::UiInbound>,
    mut inventory_events: MessageWriter<crate::events::InventoryEvent>,
    mut ability_events: MessageWriter<crate::events::AbilityEvent>,
) {
    let bindings = unified_bindings;

    if bindings.is_just_pressed(
        GameAction::Refresh,
        &keyboard_input,
        Some(&gamepad_query),
        Some(&gamepad_config),
    ) {
        tracing::info!("Refresh triggered");
        outbox.send(&RefreshRequest);
    }

    if bindings.is_just_pressed(
        GameAction::BasicAttack,
        &keyboard_input,
        Some(&gamepad_query),
        Some(&gamepad_config),
    ) {
        player_actions.write(PlayerAction::BasicAttack);
    }

    if bindings.is_just_pressed(
        GameAction::AutoAttackToggle,
        &keyboard_input,
        Some(&gamepad_query),
        Some(&gamepad_config),
    ) {
        player_actions.write(PlayerAction::ToggleAutoAttack);
    }

    if bindings.is_just_pressed(
        GameAction::ItemPickupBelow,
        &keyboard_input,
        Some(&gamepad_query),
        Some(&gamepad_config),
    ) {
        player_actions.write(PlayerAction::ItemPickupBelow);
    }

    if bindings.is_just_pressed(
        GameAction::ToggleOverview,
        &keyboard_input,
        Some(&gamepad_query),
        Some(&gamepad_config),
    ) {
        if let Some(mut minimap_state) = minimap_renderer_state {
            minimap_state.visible = !minimap_state.visible;
        }
    }

    // Toggle Panels
    if let Some(strong) = window.as_ref().and_then(|w| w.0.upgrade()) {
        let game_state = slint::ComponentHandle::global::<crate::GameState>(&strong);

        if bindings.is_just_pressed(
            GameAction::Inventory,
            &keyboard_input,
            Some(&gamepad_query),
            Some(&gamepad_config),
        ) {
            game_state.set_show_inventory(!game_state.get_show_inventory());
        }
        if bindings.is_just_pressed(
            GameAction::Skills,
            &keyboard_input,
            Some(&gamepad_query),
            Some(&gamepad_config),
        ) {
            game_state.set_show_skills(!game_state.get_show_skills());
        }
        if bindings.is_just_pressed(
            GameAction::Spells,
            &keyboard_input,
            Some(&gamepad_query),
            Some(&gamepad_config),
        ) {
            game_state.set_show_spells(!game_state.get_show_spells());
        }
        if bindings.is_just_pressed(
            GameAction::Settings,
            &keyboard_input,
            Some(&gamepad_query),
            Some(&gamepad_config),
        ) {
            let settings_state = slint::ComponentHandle::global::<crate::SettingsState>(&strong);
            settings_state.set_show_game_menu(!settings_state.get_show_game_menu());
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
            hotbar_panel_state.current_panel = *panel;
        }
    }

    // Hotbar slot activation
    for (i, action) in HOTBAR_SLOT_ACTIONS.iter().enumerate() {
        if bindings.is_just_pressed(
            *action,
            &keyboard_input,
            Some(&gamepad_query),
            Some(&gamepad_config),
        ) {
            let Some((category, slot_index)) =
                resolve_hotbar_slot_target(i, &hotbar_panel_state, &settings)
            else {
                continue;
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
                SlotPanelType::Macro | SlotPanelType::World | SlotPanelType::None => {}
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
                    direction: new_direction,
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
                            direction: new_direction,
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
