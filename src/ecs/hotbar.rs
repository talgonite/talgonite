use bevy::prelude::*;
use std::collections::HashMap;

use crate::settings_types::CustomHotBars;
use crate::webui::ipc::Cooldown;
use crate::{CurrentSession, settings::Settings};

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
    pub rows: HotbarRows,
}

pub fn sync_hotbar_view_to_settings(
    hotbar_panel: Res<HotbarPanelState>,
    mut settings: ResMut<Settings>,
    session: Option<Res<CurrentSession>>,
) {
    if !hotbar_panel.is_changed() {
        return;
    }

    let Some(session) = session.as_ref() else {
        return;
    };

    settings.set_current_hotbar_panel(
        session.server_id,
        &session.username,
        hotbar_panel.current_panel as i32,
    );
    settings.set_hotbar_row_count(
        session.server_id,
        &session.username,
        hotbar_panel.rows.as_i32(),
    );
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HotbarRows {
    #[default]
    One = 1,
    Three = 3,
    Five = 5,
}

impl HotbarRows {
    pub fn from_i32(value: i32) -> Self {
        match value {
            3 => Self::Three,
            5 => Self::Five,
            _ => Self::One,
        }
    }

    pub fn as_i32(self) -> i32 {
        self as i32
    }

    pub fn expand(self) -> Self {
        match self {
            Self::One => Self::Three,
            Self::Three => Self::Five,
            Self::Five => Self::Five,
        }
    }

    pub fn collapse(self) -> Self {
        match self {
            Self::Five => Self::Three,
            Self::Three => Self::One,
            Self::One => Self::One,
        }
    }
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

    pub fn is_custom(self) -> bool {
        matches!(self, Self::Hotbar1 | Self::Hotbar2 | Self::Hotbar3)
    }
}
