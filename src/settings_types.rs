use bevy::prelude::Resource;
use game_ui::{CoreToUi, LoginError};

pub use game_types::{
    CharacterPreview, CustomHotBarSlot, CustomHotBars, KeyBindings, SavedCredential,
    SavedCredentialPublic, ServerEntry, XRaySize,
};
use std::collections::HashMap;

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct AudioSettings {
    pub music_volume: f32,
    pub sfx_volume: f32,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct GraphicsSettings {
    pub xray_size: XRaySize,
    pub scale: f32,
    #[serde(default = "default_true")]
    pub high_quality_scaling: bool,
    #[serde(default = "default_true")]
    pub show_hotbar_1: bool,
    #[serde(default = "default_false")]
    pub show_hotbar_2: bool,
}

fn default_true() -> bool {
    true
}

fn default_false() -> bool {
    false
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct GameplaySettings {
    pub current_server_id: Option<u32>,
}

#[derive(Resource, serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct Settings {
    pub audio: AudioSettings,
    pub graphics: GraphicsSettings,
    pub gameplay: GameplaySettings,
    pub key_bindings: KeyBindings,
    pub servers: Vec<ServerEntry>,
    #[serde(skip)]
    pub saved_credentials: Vec<SavedCredential>,
    #[serde(skip)]
    pub hotbars: HashMap<String, CustomHotBars>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct CharacterProfile {
    pub id: String,
    pub server_id: u32,
    pub username: String,
    pub last_used: u64,
    #[serde(default, deserialize_with = "game_types::deserialize_preview_lossy")]
    pub preview: Option<CharacterPreview>,
    pub hotbars: CustomHotBars,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            audio: AudioSettings {
                music_volume: 0.5,
                sfx_volume: 0.7,
            },
            graphics: GraphicsSettings {
                xray_size: XRaySize::Medium,
                scale: 1.0,
                high_quality_scaling: true,
                show_hotbar_1: true,
                show_hotbar_2: false,
            },
            gameplay: GameplaySettings {
                current_server_id: Some(1),
            },
            key_bindings: KeyBindings::default(),
            servers: vec![ServerEntry {
                id: 1,
                name: "DA Official".to_string(),
                address: "da0.kru.com:2610".to_string(),
            }],
            saved_credentials: vec![],
            hotbars: HashMap::new(),
        }
    }
}

impl Settings {
    pub fn get_hotbars(&self, server_id: u32, username: &str) -> CustomHotBars {
        let key = format!("{}:{}", server_id, username);
        self.hotbars
            .get(&key)
            .cloned()
            .unwrap_or_else(CustomHotBars::new)
    }

    pub fn set_hotbars(&mut self, server_id: u32, username: &str, hotbars: CustomHotBars) {
        let key = format!("{}:{}", server_id, username);
        self.hotbars.insert(key, hotbars);
    }

    pub fn to_sync_message(&self) -> CoreToUi {
        CoreToUi::SettingsSync {
            xray_size: self.graphics.xray_size as u8,
            sfx_volume: self.audio.sfx_volume,
            music_volume: self.audio.music_volume,
            scale: self.graphics.scale,
            show_hotbar_1: self.graphics.show_hotbar_1,
            show_hotbar_2: self.graphics.show_hotbar_2,
            key_bindings: (&self.key_bindings).into(),
        }
    }

    pub fn to_snapshot_message(&self, login_error: Option<LoginError>) -> CoreToUi {
        CoreToUi::Snapshot {
            servers: self.servers.clone(),
            current_server_id: self.gameplay.current_server_id,
            logins: self
                .saved_credentials
                .iter()
                .map(|c| SavedCredentialPublic {
                    id: c.id.clone(),
                    server_id: c.server_id,
                    username: c.username.clone(),
                    last_used: c.last_used,
                    preview: c.preview.clone(),
                })
                .collect(),
            login_error,
        }
    }
}
