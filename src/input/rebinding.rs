use bevy::input::gamepad::GamepadButton;
use bevy::prelude::*;
use game_input::GamepadConfig;

#[derive(Resource, Default)]
pub struct RebindingState {
    pub active: bool,
    pub action: String,
}

pub fn sync_rebinding_state_from_slint(
    window: Option<Res<crate::slint_support::state_bridge::SlintWindow>>,
    mut rebinding: ResMut<RebindingState>,
) {
    let Some(window) = window else {
        return;
    };
    let Some(strong) = window.0.upgrade() else {
        return;
    };

    let settings_state = slint::ComponentHandle::global::<crate::SettingsState>(&strong);
    rebinding.active = settings_state.get_is_rebinding();
    rebinding.action = settings_state.get_rebinding_action().to_string();
}

pub fn gamepad_rebinding_system(
    rebinding: Res<RebindingState>,
    gamepads: Query<&Gamepad>,
    config: Res<GamepadConfig>,
    window: Option<Res<crate::slint_support::state_bridge::SlintWindow>>,
) {
    if !rebinding.active || rebinding.action.is_empty() {
        return;
    }

    let Some(window) = window else {
        return;
    };
    let Some(strong) = window.0.upgrade() else {
        return;
    };

    let Some(gamepad_entity) = config.primary_gamepad else {
        return;
    };

    let Ok(gamepad) = gamepads.get(gamepad_entity) else {
        return;
    };

    for button in GamepadButton::all() {
        if gamepad.just_pressed(button) {
            let button_name = format!("Gamepad:{}", button_label(button));

            let settings_state = slint::ComponentHandle::global::<crate::SettingsState>(&strong);
            settings_state.invoke_rebind_key(slint::SharedString::from(button_name));
            break;
        }
    }
}

fn button_label(button: GamepadButton) -> &'static str {
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
