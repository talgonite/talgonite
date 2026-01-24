use std::fmt;
use std::time::{Duration, Instant};

use game_types::{KeyBindings, SavedCredentialPublic, ServerEntry, SlotPanelType};
use packets::server::{LoginMessageType, SpellType};

#[derive(Debug, Clone)]
pub enum LoginError {
    Response(LoginMessageType),
    Network(String),
    Unknown,
}

#[derive(Debug)]
pub enum UiToCore {
    LoginSubmit {
        server_id: u32,
        username: String,
        password: String,
        remember: bool,
    },
    LoginUseSaved {
        id: String,
    },
    LoginRemoveSaved {
        id: String,
    },
    RequestSnapshot,
    ServersChangeCurrent {
        id: u32,
    },
    ServersAdd {
        server: ServerNoId,
    },
    ServersEdit {
        server: ServerWithId,
    },
    ServersRemove {
        id: u32,
    },
    InputKeyboard {
        action: String,
        code: String,
    },
    InputPointer {
        action: String,
        button: Option<u8>,
        x: f32,
        y: f32,
        delta_x: Option<f32>,
        delta_y: Option<f32>,
        shift: Option<bool>,
        ctrl: Option<bool>,
        alt: Option<bool>,
        meta: Option<bool>,
    },
    ActivateAction {
        category: SlotPanelType,
        index: usize,
    },
    Unequip {
        slot: u8,
    },
    DragDropAction {
        src_category: SlotPanelType,
        src_index: usize,
        dst_category: SlotPanelType,
        dst_index: usize,
        x: f32,
        y: f32,
    },
    ChatSubmit {
        mode: String,
        text: String,
        target: Option<String>,
    },
    WorldMapClick {
        map_id: u16,
        x: u16,
        y: u16,
        check_sum: u16,
    },
    /// User selected a menu entry.
    MenuSelect {
        /// For text menus: the option's pursuit_id. For shop menus: unused.
        id: i32,
        /// For shop menus: the item name to send as Topics. Empty for text menus.
        name: String,
    },
    /// Close the NPC dialog.
    MenuClose,
    SettingsChange {
        xray_size: u8,
    },
    VolumeChange {
        sfx: Option<f32>,
        music: Option<f32>,
    },
    ScaleChange {
        scale: f32,
    },
    RebindKey {
        action: String,
        new_key: String,
    },
    /// Quit the application.
    ExitApplication,
    /// Return to the main menu from in-game UI.
    ReturnToMainMenu,
    SetHotbarPanel {
        panel_num: u8,
    },
    RequestWorldList,
    SetWorldListFilter {
        filter: WorldListFilter,
    },
}

/// A menu entry that can be a text option or an item with sprite
#[derive(Debug, Clone)]
pub struct MenuEntryUi {
    pub text: String,
    /// Option ID for text menus, item index for shop menus
    pub id: i32,
    /// Sprite ID for icon loading (0 = no icon, text-only)
    pub sprite: u16,
    /// Color/palette index for the sprite
    pub color: u8,
    /// Cost in gold (0 = not a shop item)
    pub cost: i32,
}

impl MenuEntryUi {
    /// Create a text-only menu option
    pub fn text_option(text: String, id: i32) -> Self {
        Self {
            text,
            id,
            sprite: 0,
            color: 0,
            cost: 0,
        }
    }

    /// Create a shop item entry
    pub fn shop_item(name: String, index: i32, sprite: u16, color: u8, cost: i32) -> Self {
        Self {
            text: name,
            id: index,
            sprite,
            color,
            cost,
        }
    }

    /// Create a spell/skill entry (no cost)
    pub fn ability(name: String, index: i32, sprite: u16) -> Self {
        Self {
            text: name,
            id: index,
            sprite,
            color: 0,
            cost: 0,
        }
    }
}

/// What type of entries the menu contains (determines icon loading)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuEntryType {
    /// Text-only options, no icons
    TextOptions,
    /// Items with item sprites
    Items,
    /// Spells with spell sprites
    Spells,
    /// Skills with skill sprites
    Skills,
}

#[derive(Debug)]
pub enum CoreToUi {
    Snapshot {
        servers: Vec<ServerEntry>,
        current_server_id: Option<u32>,
        logins: Vec<SavedCredentialPublic>,
        login_error: Option<LoginError>,
    },
    EnteredGame,
    ChatAppend {
        entries: Vec<ChatEntryUi>,
    },
    WorldMapOpen {
        field_name: String,
        nodes: Vec<WorldMapNodeUi>,
    },
    DisplayMenu {
        title: String,
        text: String,
        sprite_id: u16,
        /// What type of content - determines how icons are loaded
        entry_type: MenuEntryType,
        /// Pursuit ID for shop responses (0 for text menus)
        pursuit_id: u16,
        entries: Vec<MenuEntryUi>,
    },
    /// Close any open menu/dialog
    DisplayMenuClose,
    /// Text entry dialog (e.g., quantity input)
    DisplayMenuTextEntry {
        title: String,
        text: String,
        sprite_id: u16,
        /// Context arg (e.g., item name)
        args: String,
        pursuit_id: u16,
    },
    SettingsSync {
        xray_size: u8,
        sfx_volume: f32,
        music_volume: f32,
        scale: f32,
        key_bindings: KeyBindingsUi,
    },
}

#[derive(Debug, Clone)]
pub struct KeyBindingsUi {
    pub move_up: String,
    pub move_down: String,
    pub move_left: String,
    pub move_right: String,
    pub inventory: String,
    pub skills: String,
    pub spells: String,
    pub settings: String,
    pub refresh: String,
    pub basic_attack: String,
    pub hotbar_slot_1: String,
    pub hotbar_slot_2: String,
    pub hotbar_slot_3: String,
    pub hotbar_slot_4: String,
    pub hotbar_slot_5: String,
    pub hotbar_slot_6: String,
    pub hotbar_slot_7: String,
    pub hotbar_slot_8: String,
    pub hotbar_slot_9: String,
    pub hotbar_slot_10: String,
    pub hotbar_slot_11: String,
    pub hotbar_slot_12: String,
    pub switch_to_inventory: String,
    pub switch_to_skills: String,
    pub switch_to_spells: String,
    pub switch_to_hotbar_1: String,
    pub switch_to_hotbar_2: String,
    pub switch_to_hotbar_3: String,
}

impl From<&KeyBindings> for KeyBindingsUi {
    fn from(kb: &KeyBindings) -> Self {
        Self {
            move_up: kb.move_up.clone(),
            move_down: kb.move_down.clone(),
            move_left: kb.move_left.clone(),
            move_right: kb.move_right.clone(),
            inventory: kb.inventory.clone(),
            skills: kb.skills.clone(),
            spells: kb.spells.clone(),
            settings: kb.settings.clone(),
            refresh: kb.refresh.clone(),
            basic_attack: kb.basic_attack.clone(),
            hotbar_slot_1: kb.hotbar_slot_1.clone(),
            hotbar_slot_2: kb.hotbar_slot_2.clone(),
            hotbar_slot_3: kb.hotbar_slot_3.clone(),
            hotbar_slot_4: kb.hotbar_slot_4.clone(),
            hotbar_slot_5: kb.hotbar_slot_5.clone(),
            hotbar_slot_6: kb.hotbar_slot_6.clone(),
            hotbar_slot_7: kb.hotbar_slot_7.clone(),
            hotbar_slot_8: kb.hotbar_slot_8.clone(),
            hotbar_slot_9: kb.hotbar_slot_9.clone(),
            hotbar_slot_10: kb.hotbar_slot_10.clone(),
            hotbar_slot_11: kb.hotbar_slot_11.clone(),
            hotbar_slot_12: kb.hotbar_slot_12.clone(),
            switch_to_inventory: kb.switch_to_inventory.clone(),
            switch_to_skills: kb.switch_to_skills.clone(),
            switch_to_spells: kb.switch_to_spells.clone(),
            switch_to_hotbar_1: kb.switch_to_hotbar_1.clone(),
            switch_to_hotbar_2: kb.switch_to_hotbar_2.clone(),
            switch_to_hotbar_3: kb.switch_to_hotbar_3.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct WorldMapNodeUi {
    pub text: String,
    pub map_id: u16,
    pub x: u16,
    pub y: u16,
    pub dest_x: u16,
    pub dest_y: u16,
    pub check_sum: u16,
}

#[derive(Debug, Clone)]
pub struct InventoryItemUi {
    pub id: ActionId,
    pub slot: u8,
    pub name: String,
    pub count: u32,
    pub sprite: u16,
    pub color: u8,
    pub stackable: bool,
    pub max_durability: u32,
    pub current_durability: u32,
}

#[derive(Debug, Clone)]
pub struct WorldListMemberUi {
    pub name: String,
    pub title: String,
    pub class: String,
    pub color: [f32; 4],
    pub is_master: bool,
}

#[derive(Debug, Clone, Default)]
pub struct WorldListFilter {
    pub class: Option<String>,
    pub master_only: bool,
    pub search: String,
}

#[derive(Debug, Clone)]
pub struct ChatEntryUi {
    pub kind: String,
    pub message_type: Option<u8>,
    pub text: String,
    pub show_in_message_box: bool,
    pub show_in_action_bar: bool,
    pub color: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ActionId {
    id: String,
    sprite: u16,
    panel_type: SlotPanelType,
}

impl PartialEq for ActionId {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for ActionId {}

impl std::hash::Hash for ActionId {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl ActionId {
    fn new(base: &str, sprite: u16, name: &str) -> Self {
        let mut out = String::with_capacity(name.len());
        let mut depth = 0u32;
        for ch in name.chars() {
            match ch {
                '(' => {
                    depth += 1;
                }
                ')' => {
                    if depth > 0 {
                        depth -= 1;
                    }
                }
                _ if depth > 0 => {}
                _ => out.push(ch),
            }
        }
        let mut slug = String::with_capacity(out.len());
        let mut prev_us = false;
        for ch in out.chars() {
            let lc = ch.to_ascii_lowercase();
            if lc.is_ascii_alphanumeric() {
                slug.push(lc);
                prev_us = false;
            } else if !prev_us {
                slug.push('_');
                prev_us = true;
            }
        }
        while slug.ends_with('_') {
            slug.pop();
        }
        while slug.starts_with('_') {
            slug.remove(0);
        }
        let panel_type = match base {
            "SK" => SlotPanelType::Skill,
            "SP" => SlotPanelType::Spell,
            "IT" => SlotPanelType::Item,
            _ => SlotPanelType::None,
        };

        let id = format!(
            "{}{:04}{}",
            base,
            sprite,
            if slug.is_empty() { name } else { &slug }
        );

        ActionId {
            id,
            sprite,
            panel_type,
        }
    }

    pub fn from_skill(sprite: u16, name: &str) -> Self {
        Self::new("SK", sprite, name)
    }

    pub fn from_spell(sprite: u16, name: &str) -> Self {
        Self::new("SP", sprite, name)
    }

    pub fn from_item(sprite: u16, name: &str) -> Self {
        Self::new("IT", sprite, name)
    }

    pub fn as_str(&self) -> &str {
        &self.id
    }

    pub fn sprite(&self) -> u16 {
        self.sprite
    }

    pub fn from_str(s: &str) -> Self {
        let sprite = if s.len() >= 6 {
            s[2..6].parse::<u16>().unwrap_or(0)
        } else {
            0
        };
        let panel_type = if s.starts_with("IT") {
            SlotPanelType::Item
        } else if s.starts_with("SK") {
            SlotPanelType::Skill
        } else if s.starts_with("SP") {
            SlotPanelType::Spell
        } else {
            SlotPanelType::None
        };
        ActionId {
            id: s.to_string(),
            sprite,
            panel_type,
        }
    }

    pub fn panel_type(&self) -> SlotPanelType {
        self.panel_type
    }
}

impl fmt::Display for ActionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.id)
    }
}

#[derive(Debug, Clone)]
pub struct SkillUi {
    pub id: ActionId,
    pub slot: u8,
    pub name: String,
    pub sprite: u16,
    pub on_cooldown: Option<Cooldown>,
    pub cooldown_secs: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct Cooldown {
    pub start_time: Instant,
    pub duration: Duration,
    pub time_left: Duration,
}

impl Cooldown {
    pub fn new(cooldown_secs: u32) -> Self {
        let duration = Duration::from_secs(cooldown_secs.into());
        Self {
            start_time: Instant::now(),
            duration,
            time_left: duration,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SpellUi {
    pub id: ActionId,
    pub slot: u8,
    pub sprite: u16,
    pub panel_name: String,
    pub prompt: String,
    pub cast_lines: u8,
    pub spell_type: SpellType,
}

#[derive(Debug, Clone)]
pub struct ServerNoId {
    pub name: String,
    pub address: String,
}

#[derive(Debug, Clone)]
pub struct ServerWithId {
    pub id: u32,
    pub name: String,
    pub address: String,
}
