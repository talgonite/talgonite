use serde::{Deserialize, Serialize};

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
        let mut bars = vec![vec![CustomHotBarSlot::default(); 12]; 3];
        for (idx, action_id) in wrapper.bars.into_iter().enumerate() {
            let bar_idx = idx / 12;
            let slot_idx = idx % 12;
            if bar_idx < 3 {
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
                vec![CustomHotBarSlot::default(); 12],
                vec![CustomHotBarSlot::default(); 12],
                vec![CustomHotBarSlot::default(); 12],
            ],
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
