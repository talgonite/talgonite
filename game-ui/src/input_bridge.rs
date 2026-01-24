use bevy::input::ButtonInput;
use bevy::input::keyboard::KeyCode;
use bevy::input::mouse::MouseButton;
use bevy::prelude::*;
use i_slint_core::items::{KeyEvent, PointerEventButton, PointerEventKind};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

pub type SharedKeyEventQueue = Arc<Mutex<VecDeque<QueuedKeyEvent>>>;
pub type SharedPointerEventQueue = Arc<Mutex<VecDeque<QueuedPointerEvent>>>;
pub type SharedScrollEventQueue = Arc<Mutex<VecDeque<QueuedScrollEvent>>>;
pub type SharedDoubleClickQueue = Arc<Mutex<VecDeque<(f32, f32)>>>;

#[derive(Resource, Clone)]
pub struct SlintKeyEventQueue(pub SharedKeyEventQueue);

#[derive(Resource, Clone)]
pub struct SlintPointerEventQueue(pub SharedPointerEventQueue);

#[derive(Resource, Clone)]
pub struct SlintScrollEventQueue(pub SharedScrollEventQueue);

#[derive(Resource, Clone)]
pub struct SlintDoubleClickQueue(pub SharedDoubleClickQueue);

pub fn new_shared_queue() -> SharedKeyEventQueue {
    Arc::new(Mutex::new(VecDeque::new()))
}

pub fn new_shared_pointer_queue() -> SharedPointerEventQueue {
    Arc::new(Mutex::new(VecDeque::new()))
}

pub fn new_shared_scroll_queue() -> SharedScrollEventQueue {
    Arc::new(Mutex::new(VecDeque::new()))
}

pub fn new_shared_double_click_queue() -> SharedDoubleClickQueue {
    Arc::new(Mutex::new(VecDeque::new()))
}

#[derive(Clone, Copy, Debug)]
pub enum QueuedKeyAction {
    Press,
    Release,
}

#[derive(Clone, Copy, Debug)]
pub struct QueuedKeyEvent {
    pub code: KeyCode,
    pub action: QueuedKeyAction,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct QueuedPointerEvent {
    pub kind: PointerEventKind,
    pub button: PointerEventButton,
    pub position: (f32, f32),
}

#[derive(Clone, Copy, Debug)]
pub struct QueuedScrollEvent {
    pub position: (f32, f32),
    pub delta: (f32, f32),
}

#[derive(Resource, Default, Clone, Copy, Debug)]
pub struct CursorPosition {
    pub x: f32,
    pub y: f32,
}

#[derive(Resource, Default, Debug)]
pub struct KeyboardEdges {
    pub just_pressed: Vec<KeyCode>,
    pub just_released: Vec<KeyCode>,
}

pub fn pump_slint_key_events_system(
    queue: Res<SlintKeyEventQueue>,
    mut kb: ResMut<ButtonInput<KeyCode>>,
) {
    let events: Vec<QueuedKeyEvent> = {
        let Ok(mut guard) = queue.0.lock() else {
            return;
        };
        if guard.is_empty() {
            return;
        }
        guard.drain(..).collect()
    };

    for event in events {
        // Synchronize modifier states to Bevy's input set
        if event.ctrl {
            kb.press(KeyCode::ControlLeft);
        } else {
            kb.release(KeyCode::ControlLeft);
        }

        if event.shift {
            kb.press(KeyCode::ShiftLeft);
        } else {
            kb.release(KeyCode::ShiftLeft);
        }

        if event.alt {
            kb.press(KeyCode::AltLeft);
        } else {
            kb.release(KeyCode::AltLeft);
        }

        match event.action {
            QueuedKeyAction::Press => {
                kb.press(event.code);
            }
            QueuedKeyAction::Release => {
                kb.release(event.code);
            }
        }
    }
}

pub fn pump_slint_pointer_events_system(
    queue: Res<SlintPointerEventQueue>,
    mut input: ResMut<ButtonInput<MouseButton>>,
    mut cursor: ResMut<CursorPosition>,
) {
    let events: Vec<QueuedPointerEvent> = {
        let Ok(mut guard) = queue.0.lock() else {
            return;
        };
        guard.drain(..).collect()
    };

    for event in events {
        cursor.x = event.position.0;
        cursor.y = event.position.1;

        let button = match event.button {
            PointerEventButton::Left => Some(MouseButton::Left),
            PointerEventButton::Right => Some(MouseButton::Right),
            PointerEventButton::Middle => Some(MouseButton::Middle),
            _ => None,
        };

        if let Some(btn) = button {
            match event.kind {
                PointerEventKind::Down => {
                    input.press(btn);
                }
                PointerEventKind::Up => {
                    input.release(btn);
                }
                _ => {}
            }
        }
    }
}

pub fn pump_slint_scroll_events_system(
    queue: Res<SlintScrollEventQueue>,
    mut mouse_wheel_events: MessageWriter<bevy::input::mouse::MouseWheel>,
) {
    let events: Vec<QueuedScrollEvent> = {
        let Ok(mut guard) = queue.0.lock() else {
            return;
        };
        guard.drain(..).collect()
    };

    for event in events {
        mouse_wheel_events.write(bevy::input::mouse::MouseWheel {
            unit: bevy::input::mouse::MouseScrollUnit::Pixel,
            x: event.delta.0,
            y: event.delta.1,
            window: Entity::PLACEHOLDER,
        });
    }
}

pub fn slint_key_to_keycode(event: &KeyEvent) -> Option<KeyCode> {
    let text = event.text.as_str();
    if let Some(code) = map_special_key(text) {
        return Some(code);
    }

    if text.len() == 1 {
        let ch = text.chars().next().unwrap();
        if let Some(code) = map_alpha_key(ch) {
            return Some(code);
        }
        if let Some(code) = map_digit_key(ch) {
            return Some(code);
        }
    }

    None
}

pub fn keycode_to_string(code: KeyCode) -> String {
    use KeyCode::*;
    match code {
        ArrowUp => "ArrowUp",
        ArrowDown => "ArrowDown",
        ArrowLeft => "ArrowLeft",
        ArrowRight => "ArrowRight",
        Escape => "Escape",
        Enter => "Enter",
        Space => "Space",
        Tab => "Tab",
        Minus => "Minus",
        Equal => "Equal",
        BracketLeft => "BracketLeft",
        BracketRight => "BracketRight",
        Backslash => "Backslash",
        Semicolon => "Semicolon",
        Quote => "Quote",
        Comma => "Comma",
        Period => "Period",
        Slash => "Slash",
        Backquote => "Backquote",
        Backspace => "Backspace",
        Delete => "Delete",
        F1 => "F1",
        F2 => "F2",
        F3 => "F3",
        F4 => "F4",
        F5 => "F5",
        F6 => "F6",
        F7 => "F7",
        F8 => "F8",
        F9 => "F9",
        F10 => "F10",
        F11 => "F11",
        F12 => "F12",
        KeyA => "KeyA",
        KeyB => "KeyB",
        KeyC => "KeyC",
        KeyD => "KeyD",
        KeyE => "KeyE",
        KeyF => "KeyF",
        KeyG => "KeyG",
        KeyH => "KeyH",
        KeyI => "KeyI",
        KeyJ => "KeyJ",
        KeyK => "KeyK",
        KeyL => "KeyL",
        KeyM => "KeyM",
        KeyN => "KeyN",
        KeyO => "KeyO",
        KeyP => "KeyP",
        KeyQ => "KeyQ",
        KeyR => "KeyR",
        KeyS => "KeyS",
        KeyT => "KeyT",
        KeyU => "KeyU",
        KeyV => "KeyV",
        KeyW => "KeyW",
        KeyX => "KeyX",
        KeyY => "KeyY",
        KeyZ => "KeyZ",
        Digit0 => "Digit0",
        Digit1 => "Digit1",
        Digit2 => "Digit2",
        Digit3 => "Digit3",
        Digit4 => "Digit4",
        Digit5 => "Digit5",
        Digit6 => "Digit6",
        Digit7 => "Digit7",
        Digit8 => "Digit8",
        Digit9 => "Digit9",
        _ => return format!("{:?}", code),
    }
    .to_string()
}

fn map_special_key(text: &str) -> Option<KeyCode> {
    use KeyCode::*;
    match text {
        "\u{f700}" | "\u{2191}" => Some(ArrowUp),
        "\u{f701}" | "\u{2193}" => Some(ArrowDown),
        "\u{f702}" | "\u{2190}" => Some(ArrowLeft),
        "\u{f703}" | "\u{2192}" => Some(ArrowRight),
        "\u{1b}" => Some(Escape),
        "\r" | "\n" => Some(Enter),
        "-" | "_" => Some(Minus),
        "=" | "+" => Some(Equal),
        "[" | "{" => Some(BracketLeft),
        "]" | "}" => Some(BracketRight),
        "\\" | "|" => Some(Backslash),
        ";" | ":" => Some(Semicolon),
        "'" | "\"" => Some(Quote),
        "," | "<" => Some(Comma),
        "." | ">" => Some(Period),
        "/" | "?" => Some(Slash),
        "`" | "~" => Some(Backquote),
        " " => Some(Space),
        "\t" => Some(Tab),
        "\u{8}" => Some(Backspace),
        "\u{7f}" => Some(Delete),
        "\u{f704}" | "F1" => Some(F1),
        "\u{f705}" | "F2" => Some(F2),
        "\u{f706}" | "F3" => Some(F3),
        "\u{f707}" | "F4" => Some(F4),
        "\u{f708}" | "F5" => Some(F5),
        "\u{f709}" | "F6" => Some(F6),
        "\u{f70a}" | "F7" => Some(F7),
        "\u{f70b}" | "F8" => Some(F8),
        "\u{f70c}" | "F9" => Some(F9),
        "\u{f70d}" | "F10" => Some(F10),
        "\u{f70e}" | "F11" => Some(F11),
        "\u{f70f}" | "F12" => Some(F12),
        "Control" => Some(ControlLeft),
        "Shift" => Some(ShiftLeft),
        "Alt" => Some(AltLeft),
        _ => None,
    }
}

fn map_alpha_key(ch: char) -> Option<KeyCode> {
    use KeyCode::*;
    match ch.to_ascii_uppercase() {
        'A' => Some(KeyA),
        'B' => Some(KeyB),
        'C' => Some(KeyC),
        'D' => Some(KeyD),
        'E' => Some(KeyE),
        'F' => Some(KeyF),
        'G' => Some(KeyG),
        'H' => Some(KeyH),
        'I' => Some(KeyI),
        'J' => Some(KeyJ),
        'K' => Some(KeyK),
        'L' => Some(KeyL),
        'M' => Some(KeyM),
        'N' => Some(KeyN),
        'O' => Some(KeyO),
        'P' => Some(KeyP),
        'Q' => Some(KeyQ),
        'R' => Some(KeyR),
        'S' => Some(KeyS),
        'T' => Some(KeyT),
        'U' => Some(KeyU),
        'V' => Some(KeyV),
        'W' => Some(KeyW),
        'X' => Some(KeyX),
        'Y' => Some(KeyY),
        'Z' => Some(KeyZ),
        _ => None,
    }
}

fn map_digit_key(ch: char) -> Option<KeyCode> {
    use KeyCode::*;
    match ch {
        '0' => Some(Digit0),
        '1' => Some(Digit1),
        '2' => Some(Digit2),
        '3' => Some(Digit3),
        '4' => Some(Digit4),
        '5' => Some(Digit5),
        '6' => Some(Digit6),
        '7' => Some(Digit7),
        '8' => Some(Digit8),
        '9' => Some(Digit9),
        _ => None,
    }
}
