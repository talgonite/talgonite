use bevy::prelude::Resource;

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
}
