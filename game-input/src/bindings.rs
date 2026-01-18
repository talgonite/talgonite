use super::GameAction;
use bevy::input::ButtonInput;
use bevy::input::keyboard::KeyCode;
use bevy::prelude::Resource;
use game_types::KeyBindings;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modifiers {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

impl Modifiers {
    pub fn none() -> Self {
        Self::default()
    }

    pub fn matches(&self, input: &ButtonInput<KeyCode>) -> bool {
        let ctrl_pressed = input.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);
        let shift_pressed = input.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
        let alt_pressed = input.any_pressed([KeyCode::AltLeft, KeyCode::AltRight]);

        ctrl_pressed == self.ctrl && shift_pressed == self.shift && alt_pressed == self.alt
    }

    pub fn is_empty(&self) -> bool {
        !self.ctrl && !self.shift && !self.alt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyBinding {
    pub key: KeyCode,
    pub modifiers: Modifiers,
}

impl KeyBinding {
    pub fn new(key: KeyCode) -> Self {
        Self {
            key,
            modifiers: Modifiers::none(),
        }
    }

    pub fn with_modifiers(key: KeyCode, modifiers: Modifiers) -> Self {
        Self { key, modifiers }
    }

    pub fn is_pressed(&self, input: &ButtonInput<KeyCode>) -> bool {
        input.pressed(self.key) && self.modifiers.matches(input)
    }

    pub fn is_just_pressed(&self, input: &ButtonInput<KeyCode>) -> bool {
        input.just_pressed(self.key) && self.modifiers.matches(input)
    }

    pub fn from_dom_code(code: &str) -> Option<Self> {
        let mut modifiers = Modifiers::none();
        let mut key_part = code;

        if code.contains('+') {
            let parts: Vec<&str> = code.split('+').collect();
            if parts.is_empty() {
                return None;
            }

            key_part = parts.last().unwrap();

            for &modifier in &parts[..parts.len() - 1] {
                match modifier {
                    "Ctrl" => modifiers.ctrl = true,
                    "Shift" => modifiers.shift = true,
                    "Alt" => modifiers.alt = true,
                    _ => {}
                }
            }
        }

        dom_code_to_keycode(key_part).map(|key| KeyBinding { key, modifiers })
    }

    pub fn to_dom_code(&self) -> String {
        let key_str = keycode_to_dom_code(self.key);
        if self.modifiers.is_empty() {
            key_str.to_string()
        } else {
            let mut parts = Vec::new();
            if self.modifiers.ctrl {
                parts.push("Ctrl");
            }
            if self.modifiers.shift {
                parts.push("Shift");
            }
            if self.modifiers.alt {
                parts.push("Alt");
            }
            parts.push(key_str);
            parts.join("+")
        }
    }
}

#[derive(Resource)]
pub struct InputBindings {
    bindings: std::collections::HashMap<GameAction, KeyBinding>,
}

impl InputBindings {
    pub fn new() -> Self {
        let mut bindings = std::collections::HashMap::new();
        bindings.insert(GameAction::MoveUp, KeyBinding::new(KeyCode::ArrowUp));
        bindings.insert(GameAction::MoveDown, KeyBinding::new(KeyCode::ArrowDown));
        bindings.insert(GameAction::MoveLeft, KeyBinding::new(KeyCode::ArrowLeft));
        bindings.insert(GameAction::MoveRight, KeyBinding::new(KeyCode::ArrowRight));
        bindings.insert(GameAction::Inventory, KeyBinding::new(KeyCode::KeyI));
        bindings.insert(GameAction::Skills, KeyBinding::new(KeyCode::KeyK));
        bindings.insert(GameAction::Spells, KeyBinding::new(KeyCode::KeyP));
        bindings.insert(GameAction::Settings, KeyBinding::new(KeyCode::Escape));
        bindings.insert(GameAction::Refresh, KeyBinding::new(KeyCode::F5));
        bindings.insert(GameAction::BasicAttack, KeyBinding::new(KeyCode::Space));
        Self { bindings }
    }

    pub fn from_settings(settings: &KeyBindings) -> Self {
        let mut bindings = std::collections::HashMap::new();

        if let Some(kb) = KeyBinding::from_dom_code(&settings.move_up) {
            bindings.insert(GameAction::MoveUp, kb);
        }
        if let Some(kb) = KeyBinding::from_dom_code(&settings.move_down) {
            bindings.insert(GameAction::MoveDown, kb);
        }
        if let Some(kb) = KeyBinding::from_dom_code(&settings.move_left) {
            bindings.insert(GameAction::MoveLeft, kb);
        }
        if let Some(kb) = KeyBinding::from_dom_code(&settings.move_right) {
            bindings.insert(GameAction::MoveRight, kb);
        }
        if let Some(kb) = KeyBinding::from_dom_code(&settings.inventory) {
            bindings.insert(GameAction::Inventory, kb);
        }
        if let Some(kb) = KeyBinding::from_dom_code(&settings.skills) {
            bindings.insert(GameAction::Skills, kb);
        }
        if let Some(kb) = KeyBinding::from_dom_code(&settings.spells) {
            bindings.insert(GameAction::Spells, kb);
        }
        if let Some(kb) = KeyBinding::from_dom_code(&settings.settings) {
            bindings.insert(GameAction::Settings, kb);
        }
        if let Some(kb) = KeyBinding::from_dom_code(&settings.refresh) {
            bindings.insert(GameAction::Refresh, kb);
        }
        if let Some(kb) = KeyBinding::from_dom_code(&settings.basic_attack) {
            bindings.insert(GameAction::BasicAttack, kb);
        }

        Self { bindings }
    }

    pub fn get(&self, action: GameAction) -> Option<&KeyBinding> {
        self.bindings.get(&action)
    }

    pub fn set(&mut self, action: GameAction, binding: KeyBinding) {
        self.bindings.insert(action, binding);
    }

    pub fn is_pressed(&self, action: GameAction, input: &ButtonInput<KeyCode>) -> bool {
        self.get(action)
            .map(|kb| kb.is_pressed(input))
            .unwrap_or(false)
    }

    pub fn is_just_pressed(&self, action: GameAction, input: &ButtonInput<KeyCode>) -> bool {
        self.get(action)
            .map(|kb| kb.is_just_pressed(input))
            .unwrap_or(false)
    }

    pub fn any_pressed(&self, actions: &[GameAction], input: &ButtonInput<KeyCode>) -> bool {
        actions.iter().any(|&action| self.is_pressed(action, input))
    }

    pub fn any_just_pressed(&self, actions: &[GameAction], input: &ButtonInput<KeyCode>) -> bool {
        actions
            .iter()
            .any(|&action| self.is_just_pressed(action, input))
    }
}

impl Default for InputBindings {
    fn default() -> Self {
        Self::new()
    }
}

fn dom_code_to_keycode(code: &str) -> Option<KeyCode> {
    match code {
        "ArrowUp" => Some(KeyCode::ArrowUp),
        "ArrowDown" => Some(KeyCode::ArrowDown),
        "ArrowLeft" => Some(KeyCode::ArrowLeft),
        "ArrowRight" => Some(KeyCode::ArrowRight),
        "KeyW" => Some(KeyCode::KeyW),
        "KeyA" => Some(KeyCode::KeyA),
        "KeyS" => Some(KeyCode::KeyS),
        "KeyD" => Some(KeyCode::KeyD),
        "KeyI" => Some(KeyCode::KeyI),
        "KeyK" => Some(KeyCode::KeyK),
        "KeyP" => Some(KeyCode::KeyP),
        "KeyQ" => Some(KeyCode::KeyQ),
        "KeyE" => Some(KeyCode::KeyE),
        "KeyR" => Some(KeyCode::KeyR),
        "KeyF" => Some(KeyCode::KeyF),
        "KeyG" => Some(KeyCode::KeyG),
        "KeyH" => Some(KeyCode::KeyH),
        "KeyB" => Some(KeyCode::KeyB),
        "KeyJ" => Some(KeyCode::KeyJ),
        "KeyL" => Some(KeyCode::KeyL),
        "KeyM" => Some(KeyCode::KeyM),
        "KeyN" => Some(KeyCode::KeyN),
        "KeyO" => Some(KeyCode::KeyO),
        "KeyT" => Some(KeyCode::KeyT),
        "KeyU" => Some(KeyCode::KeyU),
        "KeyV" => Some(KeyCode::KeyV),
        "KeyY" => Some(KeyCode::KeyY),
        "KeyZ" => Some(KeyCode::KeyZ),
        "KeyX" => Some(KeyCode::KeyX),
        "KeyC" => Some(KeyCode::KeyC),
        "Escape" => Some(KeyCode::Escape),
        "Space" => Some(KeyCode::Space),
        "Enter" => Some(KeyCode::Enter),
        "Tab" => Some(KeyCode::Tab),
        "Minus" => Some(KeyCode::Minus),
        "Equal" => Some(KeyCode::Equal),
        "BracketLeft" => Some(KeyCode::BracketLeft),
        "BracketRight" => Some(KeyCode::BracketRight),
        "Backslash" => Some(KeyCode::Backslash),
        "Semicolon" => Some(KeyCode::Semicolon),
        "Quote" => Some(KeyCode::Quote),
        "Comma" => Some(KeyCode::Comma),
        "Period" => Some(KeyCode::Period),
        "Slash" => Some(KeyCode::Slash),
        "Backquote" => Some(KeyCode::Backquote),
        "Digit1" => Some(KeyCode::Digit1),
        "Digit2" => Some(KeyCode::Digit2),
        "Digit3" => Some(KeyCode::Digit3),
        "Digit4" => Some(KeyCode::Digit4),
        "Digit5" => Some(KeyCode::Digit5),
        "Digit6" => Some(KeyCode::Digit6),
        "Digit7" => Some(KeyCode::Digit7),
        "Digit8" => Some(KeyCode::Digit8),
        "Digit9" => Some(KeyCode::Digit9),
        "Digit0" => Some(KeyCode::Digit0),
        "F1" => Some(KeyCode::F1),
        "F2" => Some(KeyCode::F2),
        "F3" => Some(KeyCode::F3),
        "F4" => Some(KeyCode::F4),
        "F5" => Some(KeyCode::F5),
        "F6" => Some(KeyCode::F6),
        "F7" => Some(KeyCode::F7),
        "F8" => Some(KeyCode::F8),
        "F9" => Some(KeyCode::F9),
        "F10" => Some(KeyCode::F10),
        "F11" => Some(KeyCode::F11),
        "F12" => Some(KeyCode::F12),
        _ => None,
    }
}

fn keycode_to_dom_code(code: KeyCode) -> &'static str {
    match code {
        KeyCode::ArrowUp => "ArrowUp",
        KeyCode::ArrowDown => "ArrowDown",
        KeyCode::ArrowLeft => "ArrowLeft",
        KeyCode::ArrowRight => "ArrowRight",
        KeyCode::KeyW => "KeyW",
        KeyCode::KeyA => "KeyA",
        KeyCode::KeyS => "KeyS",
        KeyCode::KeyD => "KeyD",
        KeyCode::KeyI => "KeyI",
        KeyCode::KeyK => "KeyK",
        KeyCode::KeyP => "KeyP",
        KeyCode::KeyQ => "KeyQ",
        KeyCode::KeyE => "KeyE",
        KeyCode::KeyR => "KeyR",
        KeyCode::KeyF => "KeyF",
        KeyCode::KeyG => "KeyG",
        KeyCode::KeyH => "KeyH",
        KeyCode::KeyB => "KeyB",
        KeyCode::KeyJ => "KeyJ",
        KeyCode::KeyL => "KeyL",
        KeyCode::KeyM => "KeyM",
        KeyCode::KeyN => "KeyN",
        KeyCode::KeyO => "KeyO",
        KeyCode::KeyT => "KeyT",
        KeyCode::KeyU => "KeyU",
        KeyCode::KeyV => "KeyV",
        KeyCode::KeyY => "KeyY",
        KeyCode::KeyZ => "KeyZ",
        KeyCode::KeyX => "KeyX",
        KeyCode::KeyC => "KeyC",
        KeyCode::Escape => "Escape",
        KeyCode::Space => "Space",
        KeyCode::Enter => "Enter",
        KeyCode::Tab => "Tab",
        KeyCode::Minus => "Minus",
        KeyCode::Equal => "Equal",
        KeyCode::BracketLeft => "BracketLeft",
        KeyCode::BracketRight => "BracketRight",
        KeyCode::Backslash => "Backslash",
        KeyCode::Semicolon => "Semicolon",
        KeyCode::Quote => "Quote",
        KeyCode::Comma => "Comma",
        KeyCode::Period => "Period",
        KeyCode::Slash => "Slash",
        KeyCode::Backquote => "Backquote",
        KeyCode::Digit1 => "Digit1",
        KeyCode::Digit2 => "Digit2",
        KeyCode::Digit3 => "Digit3",
        KeyCode::Digit4 => "Digit4",
        KeyCode::Digit5 => "Digit5",
        KeyCode::Digit6 => "Digit6",
        KeyCode::Digit7 => "Digit7",
        KeyCode::Digit8 => "Digit8",
        KeyCode::Digit9 => "Digit9",
        KeyCode::Digit0 => "Digit0",
        KeyCode::F1 => "F1",
        KeyCode::F2 => "F2",
        KeyCode::F3 => "F3",
        KeyCode::F4 => "F4",
        KeyCode::F5 => "F5",
        KeyCode::F6 => "F6",
        KeyCode::F7 => "F7",
        KeyCode::F8 => "F8",
        KeyCode::F9 => "F9",
        KeyCode::F10 => "F10",
        KeyCode::F11 => "F11",
        KeyCode::F12 => "F12",
        _ => "Unknown",
    }
}
