use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[repr(u8)]
pub enum XRaySize {
    Off = 0,
    Small = 1,
    #[default]
    Medium = 2,
    Large = 3,
}

impl XRaySize {
    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::Off,
            1 => Self::Small,
            3 => Self::Large,
            _ => Self::Medium,
        }
    }

    pub fn to_shader_multiplier(self) -> f32 {
        match self {
            Self::Off => 0.0,
            Self::Small => 1.0,
            Self::Medium => 1.5,
            Self::Large => 2.0,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::Small => "Small",
            Self::Medium => "Medium",
            Self::Large => "Large",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct KeyBinding(pub [String; 2]);

impl std::ops::Deref for KeyBinding {
    type Target = [String; 2];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for KeyBinding {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<'a> IntoIterator for &'a KeyBinding {
    type Item = &'a String;
    type IntoIter = std::slice::Iter<'a, String>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl IntoIterator for KeyBinding {
    type Item = String;
    type IntoIter = std::array::IntoIter<String, 2>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl Serialize for KeyBinding {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if self.0[1].is_empty() {
            serializer.serialize_str(&self.0[0])
        } else {
            self.0.serialize(serializer)
        }
    }
}

impl<'de> Deserialize<'de> for KeyBinding {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Either {
            Single(String),
            Multiple(Vec<String>),
        }

        match Either::deserialize(deserializer)? {
            Either::Single(s) => Ok(KeyBinding([s, "".to_string()])),
            Either::Multiple(v) => {
                let mut bindings = ["".to_string(), "".to_string()];
                for (i, s) in v.into_iter().enumerate().take(2) {
                    bindings[i] = s;
                }
                Ok(KeyBinding(bindings))
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct KeyBindings {
    pub move_up: KeyBinding,
    pub move_down: KeyBinding,
    pub move_left: KeyBinding,
    pub move_right: KeyBinding,
    pub inventory: KeyBinding,
    pub skills: KeyBinding,
    pub spells: KeyBinding,
    pub settings: KeyBinding,
    pub refresh: KeyBinding,
    pub basic_attack: KeyBinding,
    pub hotbar_slot_1: KeyBinding,
    pub hotbar_slot_2: KeyBinding,
    pub hotbar_slot_3: KeyBinding,
    pub hotbar_slot_4: KeyBinding,
    pub hotbar_slot_5: KeyBinding,
    pub hotbar_slot_6: KeyBinding,
    pub hotbar_slot_7: KeyBinding,
    pub hotbar_slot_8: KeyBinding,
    pub hotbar_slot_9: KeyBinding,
    pub hotbar_slot_10: KeyBinding,
    pub hotbar_slot_11: KeyBinding,
    pub hotbar_slot_12: KeyBinding,
    pub switch_to_inventory: KeyBinding,
    pub switch_to_skills: KeyBinding,
    pub switch_to_spells: KeyBinding,
    pub switch_to_hotbar_1: KeyBinding,
    pub switch_to_hotbar_2: KeyBinding,
    pub switch_to_hotbar_3: KeyBinding,
}

impl Default for KeyBindings {
    fn default() -> Self {
        Self {
            move_up: KeyBinding(["ArrowUp".to_string(), "".to_string()]),
            move_down: KeyBinding(["ArrowDown".to_string(), "".to_string()]),
            move_left: KeyBinding(["ArrowLeft".to_string(), "".to_string()]),
            move_right: KeyBinding(["ArrowRight".to_string(), "".to_string()]),
            inventory: KeyBinding(["KeyI".to_string(), "".to_string()]),
            skills: KeyBinding(["KeyK".to_string(), "".to_string()]),
            spells: KeyBinding(["KeyP".to_string(), "".to_string()]),
            settings: KeyBinding(["Escape".to_string(), "".to_string()]),
            refresh: KeyBinding(["F5".to_string(), "".to_string()]),
            basic_attack: KeyBinding(["Space".to_string(), "".to_string()]),
            hotbar_slot_1: KeyBinding(["Digit1".to_string(), "".to_string()]),
            hotbar_slot_2: KeyBinding(["Digit2".to_string(), "".to_string()]),
            hotbar_slot_3: KeyBinding(["Digit3".to_string(), "".to_string()]),
            hotbar_slot_4: KeyBinding(["Digit4".to_string(), "".to_string()]),
            hotbar_slot_5: KeyBinding(["Digit5".to_string(), "".to_string()]),
            hotbar_slot_6: KeyBinding(["Digit6".to_string(), "".to_string()]),
            hotbar_slot_7: KeyBinding(["Digit7".to_string(), "".to_string()]),
            hotbar_slot_8: KeyBinding(["Digit8".to_string(), "".to_string()]),
            hotbar_slot_9: KeyBinding(["Digit9".to_string(), "".to_string()]),
            hotbar_slot_10: KeyBinding(["Digit0".to_string(), "".to_string()]),
            hotbar_slot_11: KeyBinding(["Minus".to_string(), "".to_string()]),
            hotbar_slot_12: KeyBinding(["Equal".to_string(), "".to_string()]),
            switch_to_inventory: KeyBinding(["KeyA".to_string(), "".to_string()]),
            switch_to_skills: KeyBinding(["KeyS".to_string(), "".to_string()]),
            switch_to_spells: KeyBinding(["KeyD".to_string(), "".to_string()]),
            switch_to_hotbar_1: KeyBinding(["KeyF".to_string(), "".to_string()]),
            switch_to_hotbar_2: KeyBinding(["KeyG".to_string(), "".to_string()]),
            switch_to_hotbar_3: KeyBinding(["KeyH".to_string(), "".to_string()]),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerEntry {
    pub id: u32,
    pub name: String,
    pub address: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedCredential {
    pub id: String,
    pub server_id: u32,
    pub username: String,
    pub last_used: u64,
    #[serde(default, deserialize_with = "deserialize_preview_lossy")]
    pub preview: Option<CharacterPreview>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedCredentialPublic {
    pub id: String,
    pub server_id: u32,
    pub username: String,
    pub last_used: u64,
    #[serde(default, deserialize_with = "deserialize_preview_lossy")]
    pub preview: Option<CharacterPreview>,
}

pub fn deserialize_preview_lossy<'de, D>(
    deserializer: D,
) -> Result<Option<CharacterPreview>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct PreviewVisitor;
    impl<'de> serde::de::Visitor<'de> for PreviewVisitor {
        type Value = Option<CharacterPreview>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a hex string or null")
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            // Manual attempt to deserialize from string
            if v.len() == 90 || v.len() == 88 {
                let mut offset = 0;
                let mut next = |len: usize| {
                    u32::from_str_radix(&v[offset..offset + len], 16).inspect(|_| {
                        offset += len;
                    })
                };

                let is_male = if v.len() == 90 {
                    next(2).map_err(|_| E::custom("hex error"))? != 0
                } else {
                    true
                };

                Ok(Some(CharacterPreview {
                    is_male,
                    body: next(4).map_err(|_| E::custom("hex error"))? as u16,
                    helmet: next(4).map_err(|_| E::custom("hex error"))? as u16,
                    helmet_color: next(8).map_err(|_| E::custom("hex error"))?,
                    boots: next(4).map_err(|_| E::custom("hex error"))? as u16,
                    boots_color: next(8).map_err(|_| E::custom("hex error"))?,
                    armor: next(4).map_err(|_| E::custom("hex error"))? as u16,
                    pants_color: next(8).map_err(|_| E::custom("hex error"))?,
                    shield: next(4).map_err(|_| E::custom("hex error"))? as u16,
                    shield_color: next(8).map_err(|_| E::custom("hex error"))?,
                    weapon: next(4).map_err(|_| E::custom("hex error"))? as u16,
                    weapon_color: next(8).map_err(|_| E::custom("hex error"))?,
                    accessory1: next(4).map_err(|_| E::custom("hex error"))? as u16,
                    accessory1_color: next(8).map_err(|_| E::custom("hex error"))?,
                    overcoat: next(4).map_err(|_| E::custom("hex error"))? as u16,
                    overcoat_color: next(8).map_err(|_| E::custom("hex error"))?,
                }))
            } else {
                // Wrong length, just ignore it instead of failing
                Ok(None)
            }
        }

        fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(None)
        }

        fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            deserializer.deserialize_str(self)
        }
    }

    deserializer.deserialize_option(PreviewVisitor)
}

#[derive(Debug, Clone)]
pub struct CharacterPreview {
    pub is_male: bool,
    pub body: u16,
    pub helmet: u16,
    pub helmet_color: u32,
    pub boots: u16,
    pub boots_color: u32,
    pub armor: u16,
    pub pants_color: u32,
    pub shield: u16,
    pub shield_color: u32,
    pub weapon: u16,
    pub weapon_color: u32,
    pub accessory1: u16,
    pub accessory1_color: u32,
    pub overcoat: u16,
    pub overcoat_color: u32,
}

impl Serialize for CharacterPreview {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let hex = format!(
            "{:02x}{:04x}{:04x}{:08x}{:04x}{:08x}{:04x}{:08x}{:04x}{:08x}{:04x}{:08x}{:04x}{:08x}{:04x}{:08x}",
            if self.is_male { 1u8 } else { 0u8 },
            self.body,
            self.helmet,
            self.helmet_color,
            self.boots,
            self.boots_color,
            self.armor,
            self.pants_color,
            self.shield,
            self.shield_color,
            self.weapon,
            self.weapon_color,
            self.accessory1,
            self.accessory1_color,
            self.overcoat,
            self.overcoat_color
        );
        serializer.serialize_str(&hex)
    }
}

impl<'de> Deserialize<'de> for CharacterPreview {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        if s.len() != 90 {
            return Err(serde::de::Error::custom("Invalid hex preview length"));
        }

        let mut offset = 0;
        let mut next = |len: usize| {
            let val = u32::from_str_radix(&s[offset..offset + len], 16)
                .map_err(|_| serde::de::Error::custom("Invalid hex in preview"))?;
            offset += len;
            Ok(val)
        };

        Ok(Self {
            is_male: next(2)? != 0,
            body: next(4)? as u16,
            helmet: next(4)? as u16,
            helmet_color: next(8)?,
            boots: next(4)? as u16,
            boots_color: next(8)?,
            armor: next(4)? as u16,
            pants_color: next(8)?,
            shield: next(4)? as u16,
            shield_color: next(8)?,
            weapon: next(4)? as u16,
            weapon_color: next(8)?,
            accessory1: next(4)? as u16,
            accessory1_color: next(8)?,
            overcoat: next(4)? as u16,
            overcoat_color: next(8)?,
        })
    }
}
