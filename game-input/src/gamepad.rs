use bevy::input::gamepad::{
    GamepadAxis, GamepadButton, GamepadConnectionEvent, RawGamepadEvent,
};
use bevy::prelude::*;
use std::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GamepadInputType {
    Button(GamepadButton),
    LeftStickUp,
    LeftStickDown,
    LeftStickLeft,
    LeftStickRight,
}

impl GamepadInputType {
    pub fn is_pressed(&self, gamepad: &Gamepad, threshold: f32) -> bool {
        match self {
            GamepadInputType::Button(btn) => gamepad.pressed(*btn),
            GamepadInputType::LeftStickUp => gamepad
                .get(GamepadAxis::LeftStickY)
                .map(|v| v > threshold)
                .unwrap_or(false),
            GamepadInputType::LeftStickDown => gamepad
                .get(GamepadAxis::LeftStickY)
                .map(|v| v < -threshold)
                .unwrap_or(false),
            GamepadInputType::LeftStickLeft => gamepad
                .get(GamepadAxis::LeftStickX)
                .map(|v| v < -threshold)
                .unwrap_or(false),
            GamepadInputType::LeftStickRight => gamepad
                .get(GamepadAxis::LeftStickX)
                .map(|v| v > threshold)
                .unwrap_or(false),
        }
    }

    pub fn is_just_pressed(&self, gamepad: &Gamepad) -> bool {
        match self {
            GamepadInputType::Button(btn) => gamepad.just_pressed(*btn),
            _ => false,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            GamepadInputType::Button(GamepadButton::South) => "A/Cross",
            GamepadInputType::Button(GamepadButton::East) => "B/Circle",
            GamepadInputType::Button(GamepadButton::North) => "X/Square",
            GamepadInputType::Button(GamepadButton::West) => "Y/Triangle",
            GamepadInputType::Button(GamepadButton::LeftTrigger) => "L1/LB",
            GamepadInputType::Button(GamepadButton::RightTrigger) => "R1/RB",
            GamepadInputType::Button(GamepadButton::LeftTrigger2) => "L2/LT",
            GamepadInputType::Button(GamepadButton::RightTrigger2) => "R2/RT",
            GamepadInputType::Button(GamepadButton::Select) => "Select/Back",
            GamepadInputType::Button(GamepadButton::Start) => "Start/Menu",
            GamepadInputType::Button(GamepadButton::DPadUp) => "D-Pad Up",
            GamepadInputType::Button(GamepadButton::DPadDown) => "D-Pad Down",
            GamepadInputType::Button(GamepadButton::DPadLeft) => "D-Pad Left",
            GamepadInputType::Button(GamepadButton::DPadRight) => "D-Pad Right",
            GamepadInputType::LeftStickUp => "Left Stick Up",
            GamepadInputType::LeftStickDown => "Left Stick Down",
            GamepadInputType::LeftStickLeft => "Left Stick Left",
            GamepadInputType::LeftStickRight => "Left Stick Right",
            _ => "Unknown",
        }
    }

    pub fn from_string(s: &str) -> Option<Self> {
        let s = s.strip_prefix("Gamepad:")?;

        for button in GamepadButton::all() {
            if Self::button_label_static(button) == s {
                return Some(GamepadInputType::Button(button));
            }
        }

        match s {
            "LeftStickUp" => Some(GamepadInputType::LeftStickUp),
            "LeftStickDown" => Some(GamepadInputType::LeftStickDown),
            "LeftStickLeft" => Some(GamepadInputType::LeftStickLeft),
            "LeftStickRight" => Some(GamepadInputType::LeftStickRight),
            _ => None,
        }
    }

    fn button_label_static(button: GamepadButton) -> &'static str {
        match button {
            GamepadButton::South => "South",
            GamepadButton::East => "East",
            GamepadButton::North => "North",
            GamepadButton::West => "West",
            GamepadButton::LeftTrigger => "LeftTrigger",
            GamepadButton::RightTrigger => "RightTrigger",
            GamepadButton::LeftTrigger2 => "LeftTrigger2",
            GamepadButton::RightTrigger2 => "RightTrigger2",
            GamepadButton::Select => "Select",
            GamepadButton::Start => "Start",
            GamepadButton::Mode => "Mode",
            GamepadButton::LeftThumb => "LeftThumb",
            GamepadButton::RightThumb => "RightThumb",
            GamepadButton::DPadUp => "DPadUp",
            GamepadButton::DPadDown => "DPadDown",
            GamepadButton::DPadLeft => "DPadLeft",
            GamepadButton::DPadRight => "DPadRight",
            _ => "Unknown",
        }
    }
}

#[derive(Resource)]
pub struct GamepadConfig {
    pub primary_gamepad: Option<Entity>,
    pub stick_threshold: f32,
}

impl Default for GamepadConfig {
    fn default() -> Self {
        Self {
            primary_gamepad: None,
            stick_threshold: 0.5,
        }
    }
}

pub fn gamepad_connection_system(
    mut config: ResMut<GamepadConfig>,
    gamepads: Query<(Entity, &Name), With<Gamepad>>,
) {
    if config.primary_gamepad.is_none() {
        if let Some((entity, name)) = gamepads.iter().next() {
            config.primary_gamepad = Some(entity);
            tracing::info!("Connected gamepad: {}", name);
        }
    } else if let Some(primary) = config.primary_gamepad {
        if gamepads.get(primary).is_err() {
            config.primary_gamepad = None;
            tracing::info!("Gamepad disconnected");
        }
    }
}

#[derive(Resource)]
pub struct GilrsResource {
    pub gilrs: Mutex<Option<gilrs::Gilrs>>,
    pub gamepad_map: std::collections::HashMap<gilrs::GamepadId, Entity>,
}

impl Default for GilrsResource {
    fn default() -> Self {
        match gilrs::Gilrs::new() {
            Ok(gilrs) => {
                tracing::info!("Gilrs gamepad backend initialized");
                Self {
                    gilrs: Mutex::new(Some(gilrs)),
                    gamepad_map: std::collections::HashMap::new(),
                }
            }
            Err(e) => {
                tracing::warn!("Failed to initialize gamepad support: {}", e);
                Self {
                    gilrs: Mutex::new(None),
                    gamepad_map: std::collections::HashMap::new(),
                }
            }
        }
    }
}

pub fn gilrs_event_polling_system(
    mut gilrs_res: ResMut<GilrsResource>,
    mut raw_events: MessageWriter<RawGamepadEvent>,
    mut commands: Commands,
) {
    let mut events_to_process = Vec::new();

    {
        let Ok(mut gilrs_guard) = gilrs_res.gilrs.lock() else {
            return;
        };

        let Some(ref mut gilrs) = *gilrs_guard else {
            return;
        };

        while let Some(gilrs_event) = gilrs.next_event() {
            let event_data = match gilrs_event.event {
                gilrs::EventType::Connected => {
                    let gamepad = gilrs.gamepad(gilrs_event.id);
                    Some((
                        gilrs_event.id,
                        GamepadEventData::Connected {
                            name: gamepad.name().to_string(),
                            vendor_id: gamepad.vendor_id(),
                            product_id: gamepad.product_id(),
                        },
                    ))
                }
                gilrs::EventType::Disconnected => {
                    Some((gilrs_event.id, GamepadEventData::Disconnected))
                }
                gilrs::EventType::ButtonPressed(button, _) => convert_button(button)
                    .map(|b| (gilrs_event.id, GamepadEventData::ButtonPressed(b))),
                gilrs::EventType::ButtonReleased(button, _) => convert_button(button)
                    .map(|b| (gilrs_event.id, GamepadEventData::ButtonReleased(b))),
                gilrs::EventType::AxisChanged(axis, value, _) => convert_axis(axis)
                    .map(|a| (gilrs_event.id, GamepadEventData::AxisChanged(a, value))),
                _ => None,
            };

            if let Some(event) = event_data {
                events_to_process.push(event);
            }
        }
    }

    for (gamepad_id, event) in events_to_process {
        match event {
            GamepadEventData::Connected {
                name,
                vendor_id,
                product_id,
            } => {
                let entity = commands
                    .spawn((
                        bevy::input::gamepad::Gamepad::default(),
                        Name::new(name.clone()),
                    ))
                    .id();
                gilrs_res.gamepad_map.insert(gamepad_id, entity);

                tracing::info!("Gamepad connected: {} (Entity: {:?})", name, entity);

                raw_events.write(RawGamepadEvent::Connection(GamepadConnectionEvent::new(
                    entity,
                    bevy::input::gamepad::GamepadConnection::Connected {
                        name,
                        vendor_id,
                        product_id,
                    },
                )));
            }
            GamepadEventData::Disconnected => {
                if let Some(entity) = gilrs_res.gamepad_map.remove(&gamepad_id) {
                    tracing::info!("Gamepad disconnected (Entity: {:?})", entity);
                    raw_events.write(RawGamepadEvent::Connection(GamepadConnectionEvent::new(
                        entity,
                        bevy::input::gamepad::GamepadConnection::Disconnected,
                    )));
                    commands.entity(entity).despawn();
                }
            }
            GamepadEventData::ButtonPressed(button) => {
                if let Some(&entity) = gilrs_res.gamepad_map.get(&gamepad_id) {
                    raw_events.write(RawGamepadEvent::Button(
                        bevy::input::gamepad::RawGamepadButtonChangedEvent::new(
                            entity, button, 1.0,
                        ),
                    ));
                }
            }
            GamepadEventData::ButtonReleased(button) => {
                if let Some(&entity) = gilrs_res.gamepad_map.get(&gamepad_id) {
                    raw_events.write(RawGamepadEvent::Button(
                        bevy::input::gamepad::RawGamepadButtonChangedEvent::new(
                            entity, button, 0.0,
                        ),
                    ));
                }
            }
            GamepadEventData::AxisChanged(axis, value) => {
                if let Some(&entity) = gilrs_res.gamepad_map.get(&gamepad_id) {
                    raw_events.write(RawGamepadEvent::Axis(
                        bevy::input::gamepad::RawGamepadAxisChangedEvent::new(entity, axis, value),
                    ));
                }
            }
        }
    }
}

enum GamepadEventData {
    Connected {
        name: String,
        vendor_id: Option<u16>,
        product_id: Option<u16>,
    },
    Disconnected,
    ButtonPressed(GamepadButton),
    ButtonReleased(GamepadButton),
    AxisChanged(GamepadAxis, f32),
}

fn convert_button(button: gilrs::Button) -> Option<GamepadButton> {
    match button {
        gilrs::Button::South => Some(GamepadButton::South),
        gilrs::Button::East => Some(GamepadButton::East),
        gilrs::Button::North => Some(GamepadButton::North),
        gilrs::Button::West => Some(GamepadButton::West),
        gilrs::Button::C => Some(GamepadButton::C),
        gilrs::Button::Z => Some(GamepadButton::Z),
        gilrs::Button::LeftTrigger => Some(GamepadButton::LeftTrigger),
        gilrs::Button::LeftTrigger2 => Some(GamepadButton::LeftTrigger2),
        gilrs::Button::RightTrigger => Some(GamepadButton::RightTrigger),
        gilrs::Button::RightTrigger2 => Some(GamepadButton::RightTrigger2),
        gilrs::Button::Select => Some(GamepadButton::Select),
        gilrs::Button::Start => Some(GamepadButton::Start),
        gilrs::Button::Mode => Some(GamepadButton::Mode),
        gilrs::Button::LeftThumb => Some(GamepadButton::LeftThumb),
        gilrs::Button::RightThumb => Some(GamepadButton::RightThumb),
        gilrs::Button::DPadUp => Some(GamepadButton::DPadUp),
        gilrs::Button::DPadDown => Some(GamepadButton::DPadDown),
        gilrs::Button::DPadLeft => Some(GamepadButton::DPadLeft),
        gilrs::Button::DPadRight => Some(GamepadButton::DPadRight),
        _ => None,
    }
}

fn convert_axis(axis: gilrs::Axis) -> Option<GamepadAxis> {
    match axis {
        gilrs::Axis::LeftStickX => Some(GamepadAxis::LeftStickX),
        gilrs::Axis::LeftStickY => Some(GamepadAxis::LeftStickY),
        gilrs::Axis::LeftZ => Some(GamepadAxis::LeftZ),
        gilrs::Axis::RightStickX => Some(GamepadAxis::RightStickX),
        gilrs::Axis::RightStickY => Some(GamepadAxis::RightStickY),
        gilrs::Axis::RightZ => Some(GamepadAxis::RightZ),
        _ => None,
    }
}
