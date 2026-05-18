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
    pub toggle_overview: KeyBinding,
    pub basic_attack: KeyBinding,
    pub auto_attack_toggle: KeyBinding,
    pub item_pickup_below: KeyBinding,
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
    pub hotbar_slot_13: KeyBinding,
    pub hotbar_slot_14: KeyBinding,
    pub hotbar_slot_15: KeyBinding,
    pub hotbar_slot_16: KeyBinding,
    pub hotbar_slot_17: KeyBinding,
    pub hotbar_slot_18: KeyBinding,
    pub hotbar_slot_19: KeyBinding,
    pub hotbar_slot_20: KeyBinding,
    pub hotbar_slot_21: KeyBinding,
    pub hotbar_slot_22: KeyBinding,
    pub hotbar_slot_23: KeyBinding,
    pub hotbar_slot_24: KeyBinding,
    pub hotbar_slot_25: KeyBinding,
    pub hotbar_slot_26: KeyBinding,
    pub hotbar_slot_27: KeyBinding,
    pub hotbar_slot_28: KeyBinding,
    pub hotbar_slot_29: KeyBinding,
    pub hotbar_slot_30: KeyBinding,
    pub hotbar_slot_31: KeyBinding,
    pub hotbar_slot_32: KeyBinding,
    pub hotbar_slot_33: KeyBinding,
    pub hotbar_slot_34: KeyBinding,
    pub hotbar_slot_35: KeyBinding,
    pub hotbar_slot_36: KeyBinding,
    pub hotbar_slot_37: KeyBinding,
    pub hotbar_slot_38: KeyBinding,
    pub hotbar_slot_39: KeyBinding,
    pub hotbar_slot_40: KeyBinding,
    pub hotbar_slot_41: KeyBinding,
    pub hotbar_slot_42: KeyBinding,
    pub hotbar_slot_43: KeyBinding,
    pub hotbar_slot_44: KeyBinding,
    pub hotbar_slot_45: KeyBinding,
    pub hotbar_slot_46: KeyBinding,
    pub hotbar_slot_47: KeyBinding,
    pub hotbar_slot_48: KeyBinding,
    pub switch_to_inventory: KeyBinding,
    pub switch_to_skills: KeyBinding,
    pub switch_to_spells: KeyBinding,
    pub switch_to_hotbar_1: KeyBinding,
    pub switch_to_hotbar_2: KeyBinding,
    pub switch_to_hotbar_3: KeyBinding,
}

impl Default for KeyBindings {
    fn default() -> Self {
        fn hotbar_key(key: &str) -> KeyBinding {
            KeyBinding([key.to_string(), "".to_string()])
        }

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
            toggle_overview: KeyBinding(["Tab".to_string(), "".to_string()]),
            basic_attack: KeyBinding(["Space".to_string(), "".to_string()]),
            auto_attack_toggle: KeyBinding(["KeyT".to_string(), "".to_string()]),
            item_pickup_below: KeyBinding(["KeyB".to_string(), "".to_string()]),
            hotbar_slot_1: hotbar_key("Digit1"),
            hotbar_slot_2: hotbar_key("Digit2"),
            hotbar_slot_3: hotbar_key("Digit3"),
            hotbar_slot_4: hotbar_key("Digit4"),
            hotbar_slot_5: hotbar_key("Digit5"),
            hotbar_slot_6: hotbar_key("Digit6"),
            hotbar_slot_7: hotbar_key("Digit7"),
            hotbar_slot_8: hotbar_key("Digit8"),
            hotbar_slot_9: hotbar_key("Digit9"),
            hotbar_slot_10: hotbar_key("Digit0"),
            hotbar_slot_11: hotbar_key("Minus"),
            hotbar_slot_12: hotbar_key("Equal"),
            hotbar_slot_13: hotbar_key("Ctrl+Digit1"),
            hotbar_slot_14: hotbar_key("Ctrl+Digit2"),
            hotbar_slot_15: hotbar_key("Ctrl+Digit3"),
            hotbar_slot_16: hotbar_key("Ctrl+Digit4"),
            hotbar_slot_17: hotbar_key("Ctrl+Digit5"),
            hotbar_slot_18: hotbar_key("Ctrl+Digit6"),
            hotbar_slot_19: hotbar_key("Ctrl+Digit7"),
            hotbar_slot_20: hotbar_key("Ctrl+Digit8"),
            hotbar_slot_21: hotbar_key("Ctrl+Digit9"),
            hotbar_slot_22: hotbar_key("Ctrl+Digit0"),
            hotbar_slot_23: hotbar_key("Ctrl+Minus"),
            hotbar_slot_24: hotbar_key("Ctrl+Equal"),
            hotbar_slot_25: hotbar_key("Alt+Digit1"),
            hotbar_slot_26: hotbar_key("Alt+Digit2"),
            hotbar_slot_27: hotbar_key("Alt+Digit3"),
            hotbar_slot_28: hotbar_key("Alt+Digit4"),
            hotbar_slot_29: hotbar_key("Alt+Digit5"),
            hotbar_slot_30: hotbar_key("Alt+Digit6"),
            hotbar_slot_31: hotbar_key("Alt+Digit7"),
            hotbar_slot_32: hotbar_key("Alt+Digit8"),
            hotbar_slot_33: hotbar_key("Alt+Digit9"),
            hotbar_slot_34: hotbar_key("Alt+Digit0"),
            hotbar_slot_35: hotbar_key("Alt+Minus"),
            hotbar_slot_36: hotbar_key("Alt+Equal"),
            hotbar_slot_37: hotbar_key("Ctrl+Alt+Digit1"),
            hotbar_slot_38: hotbar_key("Ctrl+Alt+Digit2"),
            hotbar_slot_39: hotbar_key("Ctrl+Alt+Digit3"),
            hotbar_slot_40: hotbar_key("Ctrl+Alt+Digit4"),
            hotbar_slot_41: hotbar_key("Ctrl+Alt+Digit5"),
            hotbar_slot_42: hotbar_key("Ctrl+Alt+Digit6"),
            hotbar_slot_43: hotbar_key("Ctrl+Alt+Digit7"),
            hotbar_slot_44: hotbar_key("Ctrl+Alt+Digit8"),
            hotbar_slot_45: hotbar_key("Ctrl+Alt+Digit9"),
            hotbar_slot_46: hotbar_key("Ctrl+Alt+Digit0"),
            hotbar_slot_47: hotbar_key("Ctrl+Alt+Minus"),
            hotbar_slot_48: hotbar_key("Ctrl+Alt+Equal"),
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
