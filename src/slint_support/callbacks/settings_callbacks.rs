//! Settings-related callback wiring for Slint UI.

use crossbeam_channel::Sender;
use slint::ComponentHandle;

use crate::webui::ipc::UiToCore;
use crate::{MainWindow, SettingsState};

/// Wire all settings-related callbacks: volume, scale, keybind rebinding.
pub fn wire_settings_callbacks(slint_app: &MainWindow, tx: Sender<UiToCore>) {
    let settings_state = slint_app.global::<SettingsState>();

    // X-ray size changed
    {
        let tx = tx.clone();
        settings_state.on_xray_size_changed(move |size| {
            let _ = tx.send(UiToCore::SettingsChange {
                xray_size: size as u8,
            });
        });
    }

    // SFX volume changed
    {
        let tx = tx.clone();
        settings_state.on_sfx_volume_changed(move |vol| {
            let _ = tx.send(UiToCore::VolumeChange {
                sfx: Some(vol),
                music: None,
            });
        });
    }

    // Music volume changed
    {
        let tx = tx.clone();
        settings_state.on_music_volume_changed(move |vol| {
            let _ = tx.send(UiToCore::VolumeChange {
                sfx: None,
                music: Some(vol),
            });
        });
    }

    // Scale changed
    {
        let tx = tx.clone();
        settings_state.on_scale_changed(move |scale| {
            let _ = tx.send(UiToCore::ScaleChange { scale });
        });
    }

    // Start rebind
    {
        let slint_app_weak = slint_app.as_weak();
        settings_state.on_start_rebind(move |action, index| {
            if let Some(strong) = slint_app_weak.upgrade() {
                let settings_global = strong.global::<SettingsState>();
                settings_global.set_rebinding_action(action.clone());
                settings_global.set_rebinding_index(index);
                settings_global.set_is_rebinding(true);
            }
        });
    }

    // Rebind key
    {
        let slint_app_weak = slint_app.as_weak();
        let tx = tx.clone();
        settings_state.on_rebind_key(move |key_code| {
            if let Some(strong) = slint_app_weak.upgrade() {
                let settings_global = strong.global::<SettingsState>();
                let action = settings_global.get_rebinding_action().to_string();
                let index = settings_global.get_rebinding_index() as usize;

                settings_global.set_is_rebinding(false);
                settings_global.set_rebinding_action(slint::SharedString::from(""));
                let _ = tx.send(UiToCore::RebindKey {
                    action,
                    new_key: key_code.to_string(),
                    index,
                });
            }
        });
    }

    // Unbind key
    {
        let tx = tx.clone();
        settings_state.on_unbind_key(move |action, index| {
            let _ = tx.send(UiToCore::UnbindKey {
                action: action.to_string(),
                index: index as usize,
            });
        });
    }

    // Cancel rebind
    {
        let slint_app_weak = slint_app.as_weak();
        settings_state.on_cancel_rebind(move || {
            if let Some(strong) = slint_app_weak.upgrade() {
                let settings_global = strong.global::<SettingsState>();
                settings_global.set_is_rebinding(false);
                settings_global.set_rebinding_action(slint::SharedString::from(""));
            }
        });
    }

    // Return to main menu (logout)
    {
        let tx = tx.clone();
        settings_state.on_logout_requested(move || {
            let _ = tx.send(UiToCore::ReturnToMainMenu);
        });
    }

    {
        let tx = tx.clone();
        settings_state.on_exit_requested(move || {
            let _ = tx.send(UiToCore::ExitApplication);
        });
    }
}
