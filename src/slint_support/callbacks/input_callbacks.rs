//! Input-related callback wiring for Slint UI.

use std::sync::Arc;

use slint::ComponentHandle;

use crate::slint_support::input_bridge::{
    self, QueuedKeyAction, QueuedKeyEvent, QueuedPointerEvent, SharedDoubleClickQueue,
    SharedKeyEventQueue, SharedPointerEventQueue, SharedScrollEventQueue,
};
use crate::{InputBridge, MainWindow, SettingsState};

/// Wire all input-related callbacks: key press/release, pointer, scroll, double-click.
pub fn wire_input_callbacks(
    slint_app: &MainWindow,
    key_event_queue: &SharedKeyEventQueue,
    pointer_event_queue: &SharedPointerEventQueue,
    scroll_event_queue: &SharedScrollEventQueue,
    double_click_queue: &SharedDoubleClickQueue,
) {
    let input_bridge = slint_app.global::<InputBridge>();

    // Key pressed
    {
        let queue = Arc::clone(key_event_queue);
        input_bridge.on_key_pressed(move |key| {
            if let Some(code) = input_bridge::slint_key_to_keycode(&key) {
                if let Ok(mut guard) = queue.lock() {
                    guard.push_back(QueuedKeyEvent {
                        code,
                        action: QueuedKeyAction::Press,
                        ctrl: key.modifiers.control,
                        shift: key.modifiers.shift,
                        alt: key.modifiers.alt,
                    });
                }
            }
            i_slint_core::items::EventResult::Accept
        });
    }

    // Key released
    {
        let queue = Arc::clone(key_event_queue);
        input_bridge.on_key_released(move |key| {
            if let Some(code) = input_bridge::slint_key_to_keycode(&key) {
                if let Ok(mut guard) = queue.lock() {
                    guard.push_back(QueuedKeyEvent {
                        code,
                        action: QueuedKeyAction::Release,
                        ctrl: key.modifiers.control,
                        shift: key.modifiers.shift,
                        alt: key.modifiers.alt,
                    });
                }
            }
            i_slint_core::items::EventResult::Accept
        });
    }

    // Rebind key event
    {
        let slint_app_weak = slint_app.as_weak();
        input_bridge.on_rebind_key_event(move |event| {
            let Some(strong) = slint_app_weak.upgrade() else {
                return false;
            };

            let settings_global = strong.global::<SettingsState>();

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

    // Pointer event
    {
        let queue = Arc::clone(pointer_event_queue);
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

    // Double-click
    {
        let queue = Arc::clone(double_click_queue);
        input_bridge.on_double_click(move |x, y| {
            if let Ok(mut guard) = queue.lock() {
                guard.push_back((x, y));
            }
        });
    }

    // Scroll event
    {
        let queue = Arc::clone(scroll_event_queue);
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
}
