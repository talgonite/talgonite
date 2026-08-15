use bevy::input::ButtonInput;
use bevy::input::mouse::MouseButton;
use bevy::prelude::*;

use game_ui::{CursorPosition, KeyboardEdges, UiToCore};

use crate::webui::plugin::UiInbound;

#[derive(bevy::ecs::system::SystemParam)]
pub struct InputBindingResources<'w> {
    pub input_bindings: ResMut<'w, crate::input::InputBindings>,
    pub unified_bindings: ResMut<'w, crate::input::UnifiedInputBindings>,
}

pub fn handle_input_bridge(
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

pub fn clear_just_input(
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

pub fn clear_input_edges(mut edges: ResMut<KeyboardEdges>) {
    edges.just_pressed.clear();
    edges.just_released.clear();
}
