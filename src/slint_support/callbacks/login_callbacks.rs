//! Login-related callback wiring for Slint UI.

use crossbeam_channel::Sender;
use rand::Rng;
use slint::ComponentHandle;

use crate::webui::ipc::{ServerNoId, ServerWithId, UiToCore};
use crate::{LoginBridge, MainWindow};

const CHARACTER_CREATION_ARMOR_VARIANTS: i32 = 10;
const CHARACTER_CREATION_HAIR_COLORS: i32 = 14;
const CHARACTER_CREATION_GENDER_MALE: i32 = 1;
const CHARACTER_CREATION_GENDER_FEMALE: i32 = 2;

fn get_hair_style_count(gender: i32) -> i32 {
    if gender == CHARACTER_CREATION_GENDER_FEMALE {
        17 // Female
    } else {
        18 // Male
    }
}

fn random_character_creation_armor_id() -> i32 {
    rand::rng().random_range(1..=CHARACTER_CREATION_ARMOR_VARIANTS)
}

fn random_character_creation_hair_color() -> i32 {
    let mut rng = rand::rng();
    rng.random_range(0..CHARACTER_CREATION_HAIR_COLORS)
}

fn cycle_character_creation_hair_style(current: i32, delta: i32, gender: i32) -> i32 {
    let count = get_hair_style_count(gender);
    let next = current + delta;
    if next < 1 {
        count
    } else if next > count {
        1
    } else {
        next
    }
}

fn send_character_creation_preview_update(tx: &Sender<UiToCore>, window: &MainWindow) {
    let char_state = window.global::<game_ui::slint_types::CharacterCreationState>();

    let _ = tx.send(UiToCore::UpdateCharacterCreationPreview {
        gender: char_state.get_gender() as u8,
        hair_style: char_state.get_hair_style() as u8,
        hair_color: char_state.get_hair_color() as u8,
        armor_id: char_state.get_random_armor_id() as u16,
    });
}

/// Wire all login-related callbacks: login, saved credentials, server management.
pub fn wire_login_callbacks(slint_app: &MainWindow, tx: Sender<UiToCore>) {
    let login_bridge = slint_app.global::<LoginBridge>();

    // Initial character randomization
    {
        use rand::Rng;
        let mut rng = rand::rng();
        let char_state = slint_app.global::<game_ui::slint_types::CharacterCreationState>();
        let gender = if rng.random_bool(0.5) {
            CHARACTER_CREATION_GENDER_MALE
        } else {
            CHARACTER_CREATION_GENDER_FEMALE
        };
        char_state.set_gender(gender);
        char_state.set_hair_style(rng.random_range(1..=get_hair_style_count(gender)));
        char_state.set_hair_color(random_character_creation_hair_color());
        char_state.set_random_armor_id(random_character_creation_armor_id());
        send_character_creation_preview_update(&tx, slint_app);
    }

    // Attempt login
    {
        let tx = tx.clone();
        login_bridge.on_attempt_login(move |server_id, username, password, remember| {
            let _ = tx.send(UiToCore::LoginSubmit {
                server_id: server_id as u32,
                username: username.to_string(),
                password: password.to_string(),
                remember,
            });
        });
    }

    // Use saved credentials
    {
        let tx = tx.clone();
        login_bridge.on_use_saved(move |id| {
            let _ = tx.send(UiToCore::LoginUseSaved { id: id.to_string() });
        });
    }

    // Remove saved credentials
    {
        let tx = tx.clone();
        login_bridge.on_remove_saved(move |id| {
            let _ = tx.send(UiToCore::LoginRemoveSaved { id: id.to_string() });
        });
    }

    // Change current server
    {
        let tx = tx.clone();
        login_bridge.on_change_current_server(move |id| {
            let _ = tx.send(UiToCore::ServersChangeCurrent { id: id as u32 });
        });
    }

    // Add server
    {
        let tx = tx.clone();
        login_bridge.on_add_server(move |name, address| {
            let server = ServerNoId {
                name: name.to_string(),
                address: address.to_string(),
            };
            let _ = tx.send(UiToCore::ServersAdd { server });
        });
    }

    // Edit server
    {
        let tx = tx.clone();
        login_bridge.on_edit_server(move |id, name, address| {
            let server = ServerWithId {
                id: id as u32,
                name: name.to_string(),
                address: address.to_string(),
            };
            let _ = tx.send(UiToCore::ServersEdit { server });
        });
    }

    // Remove server
    {
        let tx = tx.clone();
        login_bridge.on_remove_server(move |id| {
            let _ = tx.send(UiToCore::ServersRemove { id: id as u32 });
        });
    }

    // Submit character creation
    {
        let slint_app_weak = slint_app.as_weak();
        let tx = tx.clone();
        login_bridge.on_submit_character_creation(move || {
            let Some(strong) = slint_app_weak.upgrade() else {
                return;
            };

            let lobby_state = strong.global::<crate::LobbyState>();
            let char_state = strong.global::<game_ui::slint_types::CharacterCreationState>();
            let server_id = lobby_state.get_current_server_id();
            let username = char_state.get_username();
            let password = char_state.get_password();
            let confirm_password = char_state.get_confirm_password();

            if server_id < 0 {
                char_state.set_error_message(slint::SharedString::from("No server selected"));
                char_state.set_is_submitting(false);
                return;
            }

            if username.is_empty() || password.is_empty() {
                char_state
                    .set_error_message(slint::SharedString::from("Name and password are required"));
                char_state.set_is_submitting(false);
                return;
            }

            if password != confirm_password {
                char_state.set_error_message(slint::SharedString::from("Passwords do not match"));
                char_state.set_is_submitting(false);
                return;
            }

            char_state.set_is_submitting(true);
            char_state.set_error_message(slint::SharedString::from(""));

            if tx
                .send(UiToCore::CharacterCreationSubmit {
                    server_id: server_id as u32,
                    username: username.to_string(),
                    password: password.to_string(),
                    save_login: char_state.get_save_login(),
                })
                .is_err()
            {
                char_state.set_is_submitting(false);
                char_state.set_error_message(slint::SharedString::from(
                    "Unable to start character creation",
                ));
            }
        });
    }

    // Character creation gender selection
    {
        let slint_app_weak = slint_app.as_weak();
        let tx = tx.clone();
        login_bridge.on_set_character_creation_gender(move |gender| {
            let Some(strong) = slint_app_weak.upgrade() else {
                return;
            };

            let char_state = strong.global::<game_ui::slint_types::CharacterCreationState>();
            let gender = if gender == CHARACTER_CREATION_GENDER_FEMALE {
                CHARACTER_CREATION_GENDER_FEMALE
            } else {
                CHARACTER_CREATION_GENDER_MALE
            };
            char_state.set_gender(gender);

            // Ensure hair style is within range for the new gender
            let count = get_hair_style_count(gender);
            if char_state.get_hair_style() > count {
                char_state.set_hair_style(count);
            }

            char_state.set_random_armor_id(random_character_creation_armor_id());
            send_character_creation_preview_update(&tx, &strong);
        });
    }

    // Character creation hair style selection
    {
        let slint_app_weak = slint_app.as_weak();
        let tx = tx.clone();
        login_bridge.on_cycle_character_creation_hair_style(move |delta| {
            let Some(strong) = slint_app_weak.upgrade() else {
                return;
            };

            let char_state = strong.global::<game_ui::slint_types::CharacterCreationState>();
            char_state.set_hair_style(cycle_character_creation_hair_style(
                char_state.get_hair_style(),
                delta,
                char_state.get_gender(),
            ));
            char_state.set_random_armor_id(random_character_creation_armor_id());
            send_character_creation_preview_update(&tx, &strong);
        });
    }

    // Character creation hair color selection
    {
        let slint_app_weak = slint_app.as_weak();
        let tx = tx.clone();
        login_bridge.on_set_character_creation_hair_color(move |hair_color| {
            let Some(strong) = slint_app_weak.upgrade() else {
                return;
            };

            let char_state = strong.global::<game_ui::slint_types::CharacterCreationState>();
            char_state.set_hair_color(hair_color.clamp(0, 13));
            send_character_creation_preview_update(&tx, &strong);
        });
    }

    // Request snapshot (on MainWindow, not login bridge)
    {
        let tx = tx.clone();
        slint_app.on_request_snapshot(move || {
            let _ = tx.send(UiToCore::RequestSnapshot);
        });
    }
}
