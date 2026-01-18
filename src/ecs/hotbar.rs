use bevy::prelude::*;
use std::collections::HashMap;

use crate::settings_types::CustomHotBars;
use crate::webui::ipc::Cooldown;

#[derive(Resource, Default)]
pub struct HotbarState {
    pub config: CustomHotBars,
    pub cooldowns: HashMap<String, Cooldown>,
}

impl HotbarState {
    pub fn new() -> Self {
        Self {
            config: CustomHotBars::new(),
            cooldowns: HashMap::new(),
        }
    }

    pub fn assign_slot(&mut self, slot: usize, action_id: String) {
        let bar = slot / 12;
        let bar_slot = slot % 12;

        if let Some(existing_slot) = self.config.find_action_in_bar(bar, &action_id) {
            self.config.clear_slot(bar, existing_slot);
        }
        self.config.set_slot(bar, bar_slot, action_id);
    }

    pub fn clear_slot(&mut self, slot: usize) {
        let bar = slot / 12;
        let bar_slot = slot % 12;
        self.config.clear_slot(bar, bar_slot);
    }
}

#[derive(Resource, Default)]
pub struct HotbarPanelState {
    pub current_panel: HotbarPanel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HotbarPanel {
    #[default]
    Inventory = 0,
    Skills = 1,
    Spells = 2,
    Hotbar1 = 3,
    Hotbar2 = 4,
    Hotbar3 = 5,
}

impl HotbarPanel {
    pub fn from_u8(value: u8) -> Self {
        match value {
            0 => Self::Inventory,
            1 => Self::Skills,
            2 => Self::Spells,
            3 => Self::Hotbar1,
            4 => Self::Hotbar2,
            5 => Self::Hotbar3,
            _ => Self::Inventory,
        }
    }
}
