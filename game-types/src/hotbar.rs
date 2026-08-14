use serde::{Deserialize, Serialize};

const HOTBAR_SLOTS_PER_BAR: usize = 12;
const DEFAULT_CUSTOM_HOTBAR_COUNT: usize = 5;
const DEFAULT_EMOTE_HOTBAR_LAYOUT: [&str; HOTBAR_SLOTS_PER_BAR * 4] = [
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "MC0009Smile",
    "MC0010Cry",
    "MC0011Frown",
    "MC0012Wink",
    "MC0013Surprise",
    "MC0014Tongue",
    "MC0040PuppyDog",
    "MC0016Snore",
    "MC0017Mouth",
    "MC0021BlowKiss",
    "MC0022Wave",
    "",
    "MC0034Silly",
    "MC0035Cute",
    "MC0036Yelling",
    "MC0037Mischievous",
    "MC0038Evil",
    "MC0039Horror",
    "MC0015Pleasant",
    "MC0041StoneFaced",
    "MC0042Tears",
    "MC0043FiredUp",
    "MC0044Confused",
    "",
    "MC0023RockOn",
    "MC0024Peace",
    "MC0025Stop",
    "MC0026Ouch",
    "MC0027Impatient",
    "MC0028Shock",
    "MC0029Pleasure",
    "MC0030Love",
    "MC0031SweatDrop",
    "MC0032Whistle",
    "MC0033Irritation",
    "",
];

#[derive(Debug, Clone, Default)]
pub struct CustomHotBarSlot {
    pub action_id: String,
}

#[derive(Debug, Clone, Default)]
pub struct CustomHotBars {
    pub bars: Vec<Vec<CustomHotBarSlot>>,
}

impl Serialize for CustomHotBars {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        #[derive(Serialize)]
        struct RawBars {
            bars: Vec<String>,
        }

        let mut flat: Vec<String> = self
            .bars
            .iter()
            .flat_map(|bar| bar.iter().map(|s| s.action_id.clone()))
            .collect();

        // Strip trailing blank entries from the end of the entire set
        if let Some(last_pos) = flat.iter().rposition(|id| !id.is_empty()) {
            flat.truncate(last_pos + 1);
        } else {
            flat.clear();
        }

        RawBars { bars: flat }.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for CustomHotBars {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct RawBars {
            bars: Vec<String>,
        }

        let wrapper = RawBars::deserialize(deserializer)?;
        let bar_count =
            DEFAULT_CUSTOM_HOTBAR_COUNT.max(wrapper.bars.len().div_ceil(HOTBAR_SLOTS_PER_BAR));
        let mut bars = vec![vec![CustomHotBarSlot::default(); HOTBAR_SLOTS_PER_BAR]; bar_count];
        for (idx, action_id) in wrapper.bars.into_iter().enumerate() {
            let bar_idx = idx / HOTBAR_SLOTS_PER_BAR;
            let slot_idx = idx % HOTBAR_SLOTS_PER_BAR;
            if bar_idx < bars.len() {
                bars[bar_idx][slot_idx] = CustomHotBarSlot { action_id };
            }
        }
        Ok(Self { bars })
    }
}

impl CustomHotBars {
    pub fn new() -> Self {
        Self {
            bars: vec![
                vec![CustomHotBarSlot::default(); HOTBAR_SLOTS_PER_BAR];
                DEFAULT_CUSTOM_HOTBAR_COUNT
            ],
        }
    }

    pub fn is_blank(&self) -> bool {
        self.bars
            .iter()
            .flatten()
            .all(|slot| slot.action_id.is_empty())
    }

    pub fn apply_default_emote_layout(&mut self) {
        for (index, action_id) in DEFAULT_EMOTE_HOTBAR_LAYOUT.iter().enumerate() {
            let bar = index / HOTBAR_SLOTS_PER_BAR;
            let slot = index % HOTBAR_SLOTS_PER_BAR;

            if let Some(slot_ref) = self
                .bars
                .get_mut(bar)
                .and_then(|bar_ref| bar_ref.get_mut(slot))
            {
                slot_ref.action_id = (*action_id).to_string();
            }
        }
    }

    pub fn get_slot(&self, bar: usize, slot: usize) -> Option<&CustomHotBarSlot> {
        self.bars.get(bar)?.get(slot)
    }

    pub fn set_slot(&mut self, bar: usize, slot: usize, action_id: String) {
        if let Some(slot_ref) = self.bars.get_mut(bar).and_then(|b| b.get_mut(slot)) {
            slot_ref.action_id = action_id;
        }
    }

    pub fn clear_slot(&mut self, bar: usize, slot: usize) {
        self.set_slot(bar, slot, String::new());
    }

    pub fn find_action_in_bar(&self, bar: usize, action_id: &str) -> Option<usize> {
        self.bars
            .get(bar)?
            .iter()
            .position(|slot| slot.action_id == action_id)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CustomHotBarSlot, CustomHotBars, DEFAULT_CUSTOM_HOTBAR_COUNT, DEFAULT_EMOTE_HOTBAR_LAYOUT,
        HOTBAR_SLOTS_PER_BAR,
    };

    #[test]
    fn deserializes_legacy_three_bar_payload_into_five_bars() {
        let mut entries = Vec::new();
        for idx in 0..(HOTBAR_SLOTS_PER_BAR * 3) {
            entries.push(format!("action-{idx}"));
        }

        let toml = toml::to_string(&toml::toml! {
            bars = entries
        })
        .expect("serialize test payload");

        let bars: CustomHotBars = toml::from_str(&toml).expect("deserialize legacy payload");

        assert_eq!(bars.bars.len(), DEFAULT_CUSTOM_HOTBAR_COUNT);
        assert_eq!(bars.bars[0][0].action_id, "action-0");
        assert_eq!(bars.bars[2][11].action_id, "action-35");
        assert!(bars.bars[3].iter().all(|slot| slot.action_id.is_empty()));
        assert!(bars.bars[4].iter().all(|slot| slot.action_id.is_empty()));
    }

    #[test]
    fn serializes_and_deserializes_five_bar_payload() {
        let mut bars = CustomHotBars::new();
        bars.bars[4][11] = CustomHotBarSlot {
            action_id: "tail-slot".to_string(),
        };

        let toml = toml::to_string(&bars).expect("serialize five-bar payload");
        let decoded: CustomHotBars = toml::from_str(&toml).expect("deserialize five-bar payload");

        assert_eq!(decoded.bars.len(), DEFAULT_CUSTOM_HOTBAR_COUNT);
        assert_eq!(decoded.bars[4][11].action_id, "tail-slot");
    }

    #[test]
    fn trims_trailing_empty_bars_during_serialization() {
        let mut bars = CustomHotBars::new();
        bars.bars[1][3] = CustomHotBarSlot {
            action_id: "mid-slot".to_string(),
        };

        let toml = toml::to_string(&bars).expect("serialize trimmed payload");
        let value: toml::Value = toml::from_str(&toml).expect("parse trimmed payload");
        let serialized = value
            .get("bars")
            .and_then(toml::Value::as_array)
            .expect("bars array");

        assert_eq!(serialized.len(), HOTBAR_SLOTS_PER_BAR + 4);
        assert_eq!(
            serialized.last().and_then(toml::Value::as_str),
            Some("mid-slot")
        );
    }

    #[test]
    fn applies_default_emote_layout_in_expected_slot_order() {
        let mut bars = CustomHotBars::new();
        bars.apply_default_emote_layout();

        let toml = toml::to_string(&bars).expect("serialize default emote layout");
        let value: toml::Value = toml::from_str(&toml).expect("parse default emote layout");
        let serialized = value
            .get("bars")
            .and_then(toml::Value::as_array)
            .expect("bars array");
        let actual: Vec<&str> = serialized
            .iter()
            .map(|entry| entry.as_str().expect("bar entry string"))
            .collect();

        assert_eq!(actual, &DEFAULT_EMOTE_HOTBAR_LAYOUT[..47]);
        assert!(bars.bars[4].iter().all(|slot| slot.action_id.is_empty()));
    }
}
