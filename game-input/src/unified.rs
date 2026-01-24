use super::{GameAction, GamepadConfig, GamepadInputType, KeyBinding};
use bevy::input::ButtonInput;
use bevy::input::keyboard::KeyCode;
use bevy::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputSource {
    Keyboard(KeyBinding),
    Gamepad(GamepadInputType),
}

impl InputSource {
    pub fn label(&self) -> String {
        match self {
            InputSource::Keyboard(kb) => kb.to_dom_code(),
            InputSource::Gamepad(gi) => gi.label().to_string(),
        }
    }

    pub fn from_string(s: &str) -> Option<Self> {
        GamepadInputType::from_string(s)
            .map(InputSource::Gamepad)
            .or_else(|| KeyBinding::from_dom_code(s).map(InputSource::Keyboard))
    }
}

#[derive(Resource)]
pub struct UnifiedInputBindings {
    bindings: std::collections::HashMap<GameAction, Vec<InputSource>>,
}

impl UnifiedInputBindings {
    pub fn new() -> Self {
        Self {
            bindings: std::collections::HashMap::new(),
        }
    }

    pub fn with_defaults() -> Self {
        use bevy::input::gamepad::GamepadButton;

        let mut bindings = std::collections::HashMap::new();

        bindings.insert(
            GameAction::MoveUp,
            vec![
                InputSource::Keyboard(KeyBinding::new(KeyCode::ArrowUp)),
                InputSource::Gamepad(GamepadInputType::Button(GamepadButton::DPadUp)),
                InputSource::Gamepad(GamepadInputType::LeftStickUp),
            ],
        );
        bindings.insert(
            GameAction::MoveDown,
            vec![
                InputSource::Keyboard(KeyBinding::new(KeyCode::ArrowDown)),
                InputSource::Gamepad(GamepadInputType::Button(GamepadButton::DPadDown)),
                InputSource::Gamepad(GamepadInputType::LeftStickDown),
            ],
        );
        bindings.insert(
            GameAction::MoveLeft,
            vec![
                InputSource::Keyboard(KeyBinding::new(KeyCode::ArrowLeft)),
                InputSource::Gamepad(GamepadInputType::Button(GamepadButton::DPadLeft)),
                InputSource::Gamepad(GamepadInputType::LeftStickLeft),
            ],
        );
        bindings.insert(
            GameAction::MoveRight,
            vec![
                InputSource::Keyboard(KeyBinding::new(KeyCode::ArrowRight)),
                InputSource::Gamepad(GamepadInputType::Button(GamepadButton::DPadRight)),
                InputSource::Gamepad(GamepadInputType::LeftStickRight),
            ],
        );
        bindings.insert(
            GameAction::Inventory,
            vec![
                InputSource::Keyboard(KeyBinding::new(KeyCode::KeyI)),
                InputSource::Gamepad(GamepadInputType::Button(GamepadButton::North)),
            ],
        );
        bindings.insert(
            GameAction::Skills,
            vec![
                InputSource::Keyboard(KeyBinding::new(KeyCode::KeyK)),
                InputSource::Gamepad(GamepadInputType::Button(GamepadButton::West)),
            ],
        );
        bindings.insert(
            GameAction::Spells,
            vec![
                InputSource::Keyboard(KeyBinding::new(KeyCode::KeyP)),
                InputSource::Gamepad(GamepadInputType::Button(GamepadButton::East)),
            ],
        );
        bindings.insert(
            GameAction::Settings,
            vec![
                InputSource::Keyboard(KeyBinding::new(KeyCode::Escape)),
                InputSource::Gamepad(GamepadInputType::Button(GamepadButton::Start)),
            ],
        );
        bindings.insert(
            GameAction::Refresh,
            vec![
                InputSource::Keyboard(KeyBinding::new(KeyCode::F5)),
                InputSource::Gamepad(GamepadInputType::Button(GamepadButton::Select)),
            ],
        );
        bindings.insert(
            GameAction::BasicAttack,
            vec![
                InputSource::Keyboard(KeyBinding::new(KeyCode::Space)),
                InputSource::Gamepad(GamepadInputType::Button(GamepadButton::South)),
            ],
        );

        Self { bindings }
    }

    pub fn from_settings(settings: &game_types::KeyBindings) -> Self {
        let mut unified = Self::with_defaults();

        macro_rules! bind {
            ($field:ident, $action:ident) => {
                for (i, key_str) in settings.$field.iter().enumerate() {
                    if !key_str.is_empty() {
                        if let Some(kb) = KeyBinding::from_dom_code(key_str) {
                            unified.set_keyboard_binding(GameAction::$action, kb, i);
                        }
                    } else {
                        unified.unbind_keyboard_at(GameAction::$action, i);
                    }
                }
            };
        }

        bind!(move_up, MoveUp);
        bind!(move_down, MoveDown);
        bind!(move_left, MoveLeft);
        bind!(move_right, MoveRight);
        bind!(inventory, Inventory);
        bind!(skills, Skills);
        bind!(spells, Spells);
        bind!(settings, Settings);
        bind!(refresh, Refresh);
        bind!(basic_attack, BasicAttack);
        bind!(hotbar_slot_1, HotbarSlot1);
        bind!(hotbar_slot_2, HotbarSlot2);
        bind!(hotbar_slot_3, HotbarSlot3);
        bind!(hotbar_slot_4, HotbarSlot4);
        bind!(hotbar_slot_5, HotbarSlot5);
        bind!(hotbar_slot_6, HotbarSlot6);
        bind!(hotbar_slot_7, HotbarSlot7);
        bind!(hotbar_slot_8, HotbarSlot8);
        bind!(hotbar_slot_9, HotbarSlot9);
        bind!(hotbar_slot_10, HotbarSlot10);
        bind!(hotbar_slot_11, HotbarSlot11);
        bind!(hotbar_slot_12, HotbarSlot12);
        bind!(switch_to_inventory, SwitchToInventory);
        bind!(switch_to_skills, SwitchToSkills);
        bind!(switch_to_spells, SwitchToSpells);
        bind!(switch_to_hotbar_1, SwitchToHotbar1);
        bind!(switch_to_hotbar_2, SwitchToHotbar2);
        bind!(switch_to_hotbar_3, SwitchToHotbar3);

        unified
    }

    pub fn get(&self, action: GameAction) -> Option<&[InputSource]> {
        self.bindings.get(&action).map(|v| v.as_slice())
    }

    pub fn add_binding(&mut self, action: GameAction, source: InputSource) {
        self.bindings.entry(action).or_default().push(source);
    }

    pub fn set_keyboard_binding(&mut self, action: GameAction, binding: KeyBinding, index: usize) {
        let sources = self.bindings.entry(action).or_default();

        let mut current_kb_count = 0;
        let mut target_idx = None;

        for (i, s) in sources.iter().enumerate() {
            if matches!(s, InputSource::Keyboard(_)) {
                if current_kb_count == index {
                    target_idx = Some(i);
                    break;
                }
                current_kb_count += 1;
            }
        }

        if let Some(i) = target_idx {
            sources[i] = InputSource::Keyboard(binding);
        } else {
            sources.push(InputSource::Keyboard(binding));
        }
    }

    pub fn unbind_keyboard_at(&mut self, action: GameAction, index: usize) {
        if let Some(sources) = self.bindings.get_mut(&action) {
            let mut current_kb_count = 0;
            let mut target_idx = None;

            for (i, s) in sources.iter().enumerate() {
                if matches!(s, InputSource::Keyboard(_)) {
                    if current_kb_count == index {
                        target_idx = Some(i);
                        break;
                    }
                    current_kb_count += 1;
                }
            }

            if let Some(i) = target_idx {
                sources.remove(i);
            }
        }
    }

    pub fn set_gamepad_binding(&mut self, action: GameAction, binding: GamepadInputType) {
        let sources = self.bindings.entry(action).or_default();
        sources.retain(|s| !matches!(s, InputSource::Gamepad(_)));
        sources.push(InputSource::Gamepad(binding));
    }

    pub fn set_binding(&mut self, action: GameAction, source: InputSource, index: usize) {
        match source {
            InputSource::Keyboard(kb) => self.set_keyboard_binding(action, kb, index),
            InputSource::Gamepad(gp) => self.set_gamepad_binding(action, gp),
        }
    }

    pub fn is_pressed(
        &self,
        action: GameAction,
        keyboard: &ButtonInput<KeyCode>,
        gamepad_query: Option<&Query<&Gamepad>>,
        gamepad_config: Option<&GamepadConfig>,
    ) -> bool {
        let Some(sources) = self.bindings.get(&action) else {
            return false;
        };

        for source in sources {
            match source {
                InputSource::Keyboard(kb) => {
                    if kb.is_pressed(keyboard) {
                        return true;
                    }
                }
                InputSource::Gamepad(gi) => {
                    if let (Some(config), Some(query)) = (gamepad_config, gamepad_query) {
                        if let Some(gamepad) =
                            config.primary_gamepad.and_then(|e| query.get(e).ok())
                        {
                            if gi.is_pressed(gamepad, config.stick_threshold) {
                                return true;
                            }
                        }
                    }
                }
            }
        }

        false
    }

    pub fn is_just_pressed(
        &self,
        action: GameAction,
        keyboard: &ButtonInput<KeyCode>,
        gamepad_query: Option<&Query<&Gamepad>>,
        gamepad_config: Option<&GamepadConfig>,
    ) -> bool {
        let Some(sources) = self.bindings.get(&action) else {
            return false;
        };

        for source in sources {
            match source {
                InputSource::Keyboard(kb) => {
                    if kb.is_just_pressed(keyboard) {
                        return true;
                    }
                }
                InputSource::Gamepad(gi) => {
                    if let (Some(config), Some(query)) = (gamepad_config, gamepad_query) {
                        if let Some(gamepad) =
                            config.primary_gamepad.and_then(|e| query.get(e).ok())
                        {
                            if gi.is_just_pressed(gamepad) {
                                return true;
                            }
                        }
                    }
                }
            }
        }

        false
    }

    pub fn any_pressed(
        &self,
        actions: &[GameAction],
        keyboard: &ButtonInput<KeyCode>,
        gamepad_query: Option<&Query<&Gamepad>>,
        gamepad_config: Option<&GamepadConfig>,
    ) -> bool {
        actions
            .iter()
            .any(|&action| self.is_pressed(action, keyboard, gamepad_query, gamepad_config))
    }

    pub fn any_just_pressed(
        &self,
        actions: &[GameAction],
        keyboard: &ButtonInput<KeyCode>,
        gamepad_query: Option<&Query<&Gamepad>>,
        gamepad_config: Option<&GamepadConfig>,
    ) -> bool {
        actions
            .iter()
            .any(|&action| self.is_just_pressed(action, keyboard, gamepad_query, gamepad_config))
    }
}

impl Default for UnifiedInputBindings {
    fn default() -> Self {
        Self::with_defaults()
    }
}
