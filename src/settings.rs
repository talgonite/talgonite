pub use crate::settings_types::*;
use crate::storage_dir;
use bevy::prelude::*;
use std::fs;
use tracing::{error, info};

impl Settings {
    pub fn load() -> Self {
        let root = storage_dir();
        let path = root.join("settings.toml");
        let mut settings = if path.exists() {
            match fs::read_to_string(&path) {
                Ok(content) => match toml::from_str::<Settings>(&content) {
                    Ok(s) => {
                        info!("Loaded global settings from {:?}", path);
                        s
                    }
                    Err(e) => {
                        error!("Failed to parse settings.toml: {}", e);
                        Settings::default()
                    }
                },
                Err(e) => {
                    error!("Failed to read settings.toml: {}", e);
                    Settings::default()
                }
            }
        } else {
            info!("Creating default settings at {:?}", path);
            let default_settings = Settings::default();
            default_settings.save();
            default_settings
        };

        // Load profiles
        let profiles_dir = root.join("profiles");
        if profiles_dir.exists() {
            if let Ok(server_dirs) = fs::read_dir(profiles_dir) {
                for server_dir in server_dirs.flatten() {
                    if server_dir.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                        if let Ok(char_files) = fs::read_dir(server_dir.path()) {
                            for char_file in char_files.flatten() {
                                if char_file.path().extension().and_then(|s| s.to_str())
                                    == Some("toml")
                                {
                                    if let Ok(content) = fs::read_to_string(char_file.path()) {
                                        if let Ok(profile) =
                                            toml::from_str::<CharacterProfile>(&content)
                                        {
                                            settings.saved_credentials.push(SavedCredential {
                                                id: profile.id.clone(),
                                                server_id: profile.server_id,
                                                username: profile.username.clone(),
                                                last_used: profile.last_used,
                                                preview: profile.preview,
                                            });
                                            settings.hotbars.insert(profile.id, profile.hotbars);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        settings
    }

    pub fn save(&self) {
        let root = storage_dir();
        let path = root.join("settings.toml");

        // Save global settings
        match toml::to_string_pretty(self) {
            Ok(content) => {
                if let Err(e) = fs::write(&path, content) {
                    error!("Failed to write settings.toml: {}", e);
                } else {
                    info!("Saved global settings to {:?}", path);
                }
            }
            Err(e) => error!("Failed to serialize settings: {}", e),
        }

        // Save profiles
        for cred in &self.saved_credentials {
            let hotbars = self.get_hotbars(cred.server_id, &cred.username);
            let profile = CharacterProfile {
                id: cred.id.clone(),
                server_id: cred.server_id,
                username: cred.username.clone(),
                last_used: cred.last_used,
                preview: cred.preview.clone(),
                hotbars,
            };

            let profiles_dir = root.join("profiles").join(cred.server_id.to_string());
            let _ = fs::create_dir_all(&profiles_dir);
            let profile_path = profiles_dir.join(format!("{}.toml", cred.username));

            match toml::to_string_pretty(&profile) {
                Ok(content) => {
                    if let Err(e) = fs::write(&profile_path, content) {
                        error!("Failed to write profile {:?}: {}", profile_path, e);
                    }
                }
                Err(e) => error!("Failed to serialize profile for {}: {}", cred.username, e),
            }
        }
    }

    pub fn remove_credential(&mut self, id: &str) {
        if let Some(idx) = self.saved_credentials.iter().position(|c| c.id == id) {
            let cred = self.saved_credentials.remove(idx);
            let root = storage_dir();
            let profile_path = root
                .join("profiles")
                .join(cred.server_id.to_string())
                .join(format!("{}.toml", cred.username));
            if profile_path.exists() {
                let _ = fs::remove_file(profile_path);
            }
        }
    }

    pub fn update_character_preview(
        &mut self,
        server_url: &str,
        username: &str,
        preview: CharacterPreview,
    ) {
        // Find the server id from the url
        let server_id = self
            .servers
            .iter()
            .find(|s| s.address == server_url)
            .map(|s| s.id)
            .unwrap_or(0);

        let cred_id = format!("{}:{}", server_id, username);

        if let Some(cred) = self.saved_credentials.iter_mut().find(|c| c.id == cred_id) {
            cred.preview = Some(preview);
            cred.last_used = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
        }
    }
}

pub struct SettingsPlugin;

#[derive(Resource)]
struct SettingsSaveTimer(Timer);

impl Plugin for SettingsPlugin {
    fn build(&self, app: &mut App) {
        let settings = Settings::load();
        app.insert_resource(settings);
        app.insert_resource(SettingsSaveTimer(Timer::from_seconds(1.0, TimerMode::Once)));
        app.add_systems(Update, save_settings_on_change);
    }
}

fn save_settings_on_change(
    settings: Res<Settings>,
    mut timer: ResMut<SettingsSaveTimer>,
    time: Res<Time>,
) {
    if settings.is_changed() {
        timer.0.reset();
    }

    timer.0.tick(time.delta());

    if timer.0.just_finished() {
        settings.save();
    }
}
