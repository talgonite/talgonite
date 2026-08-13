use bevy::prelude::Resource;
use game_ui::{CoreToUi, LoginError};

pub use game_types::{
    CharacterPreview, CustomHotBarSlot, CustomHotBars, KeyBindings, SavedCredential,
    SavedCredentialPublic, ServerEntry, XRaySize,
};
use std::collections::HashMap;

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default)]
pub struct HotbarData {
    #[serde(flatten)]
    pub bars: CustomHotBars,
    #[serde(default)]
    pub current_panel: i32,
    #[serde(default = "default_hotbar_row_count")]
    pub row_count: i32,
}

fn default_hotbar_row_count() -> i32 {
    1
}

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
}

impl GraphicsSettings {
    pub fn scale_from_progress(ratio: f32) -> f32 {
        if ratio < 0.0625 {
            0.5
        } else if ratio < 0.1875 {
            1.0
        } else if ratio < 0.3125 {
            1.5
        } else if ratio < 0.4375 {
            2.0
        } else if ratio < 0.5625 {
            2.5
        } else if ratio < 0.6875 {
            3.0
        } else if ratio < 0.8125 {
            4.0
        } else {
            5.0
        }
    }

    pub fn progress_from_scale(scale: f32) -> f32 {
        (scale - 0.5) / 4.5
    }

    pub fn format_scale(scale: f32) -> String {
        if (scale.round() - scale).abs() < 1e-3 {
            format!("{}x", scale.round() as i32)
        } else {
            format!("{scale:.1}x")
        }
    }
}
fn default_true() -> bool {
    true
}

fn default_modifier_hotbar_rows_target_custom_only() -> bool {
    true
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct GameplaySettings {
    pub current_server_id: Option<u32>,
    #[serde(default = "default_modifier_hotbar_rows_target_custom_only")]
    pub modifier_hotbar_rows_target_custom_only: bool,
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
    pub hotbars: HashMap<String, HotbarData>,
    #[serde(skip)]
    pub macros: HashMap<String, HashMap<String, String>>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct CharacterProfile {
    pub id: String,
    pub server_id: u32,
    pub username: String,
    pub last_used: u64,
    #[serde(default, deserialize_with = "game_types::deserialize_preview_lossy")]
    pub preview: Option<CharacterPreview>,
    #[serde(default)]
    pub hotbars: HotbarData,
    #[serde(default)]
    pub macros: HashMap<String, String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            audio: AudioSettings {
                music_volume: 0.5,
                sfx_volume: 0.7,
            },
            graphics: GraphicsSettings {
                xray_size: XRaySize::Off,
                scale: 1.0,
                high_quality_scaling: true,
            },
            gameplay: GameplaySettings {
                current_server_id: Some(1),
                modifier_hotbar_rows_target_custom_only: true,
            },
            key_bindings: KeyBindings::default(),
            servers: vec![ServerEntry {
                id: 1,
                name: "DA Official".to_string(),
                address: "da0.kru.com:2610".to_string(),
            }],
            saved_credentials: vec![],
            hotbars: HashMap::new(),
            macros: HashMap::new(),
        }
    }
}

impl Settings {
    pub fn get_hotbars(&self, server_id: u32, username: &str) -> CustomHotBars {
        let key = format!("{}:{}", server_id, username);
        self.hotbars
            .get(&key)
            .map(|data| data.bars.clone())
            .unwrap_or_else(CustomHotBars::new)
    }

    pub fn set_hotbars(&mut self, server_id: u32, username: &str, hotbars: CustomHotBars) {
        let key = format!("{}:{}", server_id, username);
        self.hotbars.entry(key).or_default().bars = hotbars;
    }

    pub fn get_current_hotbar_panel(&self, server_id: u32, username: &str) -> i32 {
        let key = format!("{}:{}", server_id, username);
        self.hotbars
            .get(&key)
            .map(|data| data.current_panel)
            .unwrap_or(0)
    }

    pub fn set_current_hotbar_panel(&mut self, server_id: u32, username: &str, panel: i32) {
        let key = format!("{}:{}", server_id, username);
        self.hotbars.entry(key).or_default().current_panel = panel;
    }

    pub fn get_macros(&self, server_id: u32, username: &str) -> HashMap<String, String> {
        let key = format!("{}:{}", server_id, username);
        self.macros.get(&key).cloned().unwrap_or_default()
    }

    pub fn set_macros(&mut self, server_id: u32, username: &str, macros: HashMap<String, String>) {
        let key = format!("{}:{}", server_id, username);
        self.macros.insert(key, macros);
    }

    pub fn get_hotbar_row_count(&self, server_id: u32, username: &str) -> i32 {
        let key = format!("{}:{}", server_id, username);
        self.hotbars
            .get(&key)
            .map(|data| data.row_count)
            .unwrap_or_else(default_hotbar_row_count)
    }

    pub fn set_hotbar_row_count(&mut self, server_id: u32, username: &str, row_count: i32) {
        let key = format!("{}:{}", server_id, username);
        self.hotbars.entry(key).or_default().row_count = row_count;
    }

    pub fn to_sync_message(&self) -> CoreToUi {
        CoreToUi::SettingsSync {
            xray_size: self.graphics.xray_size as u8,
            sfx_volume: self.audio.sfx_volume,
            music_volume: self.audio.music_volume,
            scale: self.graphics.scale,
            modifier_hotbar_rows_target_custom_only: self
                .gameplay
                .modifier_hotbar_rows_target_custom_only,
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
