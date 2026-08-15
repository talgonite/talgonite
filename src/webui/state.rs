use bevy::prelude::*;

use game_ui::{BoardPostUi, Cooldown, InventoryItemUi, SkillUi, SpellUi, WorldListFilter, WorldListMemberUi};
use packets::types::{EntityType, MenuType};

use crate::events::WorldContextMenuEntry;
use crate::rich_text::RichText;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ActiveWindowType {
    #[default]
    None,
    Dialog,
    Menu,
    Info,
}

#[derive(Resource, Default)]
pub struct ActiveMenuContext {
    pub window_type: ActiveWindowType,
    pub entity_type: Option<EntityType>,
    pub entity_id: u32,
    /// For shop menus and text entry, this is the pursuit_id to send back
    /// For text (list) menus, this is 0 (pursuit_id comes from the selected option)
    pub pursuit_id: Option<u16>,
    pub menu_type: Option<MenuType>,
    pub args: String,
    pub dialog_id: Option<u16>,
}

#[derive(Resource, Default, Debug, Clone)]
pub struct ActiveWorldContextMenu {
    pub title: String,
    pub entries: Vec<WorldContextMenuEntry>,
}

#[derive(Resource, Default, Debug, Clone)]
pub struct BoardSessionState {
    pub visible: bool,
    pub session_token: i32,
    pub active_board_id: Option<u16>,
    pub board_name: String,
    pub posts: Vec<BoardPostUi>,
    pub selected_index: i32,
    pub loading_post_id: i32,
    pub last_post_id: Option<i16>,
    pub pending_request_session_token: Option<i32>,
    pub pending_start_post_id: Option<i16>,
}

impl BoardSessionState {
    pub(crate) fn invalidate(&mut self) {
        self.session_token = self.session_token.wrapping_add(1);
        self.visible = false;
        self.active_board_id = None;
        self.board_name.clear();
        self.posts.clear();
        self.selected_index = -1;
        self.loading_post_id = -1;
        self.last_post_id = None;
        self.pending_request_session_token = None;
        self.pending_start_post_id = None;
    }

    pub(crate) fn mark_request(&mut self, start_post_id: i16) {
        self.pending_request_session_token = Some(self.session_token);
        self.pending_start_post_id = Some(start_post_id);
    }

    pub(crate) fn merge_cached_post(&self, post: &mut BoardPostUi) {
        let Some(existing_post) = self
            .posts
            .iter()
            .find(|entry| entry.post_id == post.post_id)
        else {
            return;
        };

        if !existing_post.message.is_empty() {
            post.message = existing_post.message.clone();
            post.is_unread = existing_post.is_unread;
        }
    }

    pub(crate) fn has_cached_message(&self, index: i32, post_id: i32) -> bool {
        let Ok(index) = usize::try_from(index) else {
            return false;
        };

        self.posts
            .get(index)
            .filter(|entry| entry.post_id == post_id)
            .is_some_and(|entry| !entry.message.is_empty())
    }
}

#[derive(Resource, Default, Debug, Clone)]
pub struct ExchangeSessionState {
    pub is_active: bool,
    pub other_player_id: u32,
    pub other_player_name: String,
    pub my_gold: u32,
    pub other_gold: u32,
    pub my_items: Vec<ExchangeSlotItem>,
    pub other_items: Vec<ExchangeSlotItem>,
    pub my_accepted: bool,
    pub other_accepted: bool,
    pub quantity_prompt: Option<u8>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExchangeSlotItem {
    pub sprite: u16,
    pub color: u8,
    pub name: String,
}

impl ExchangeSessionState {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

#[derive(Resource, Default, Debug, Clone)]
pub struct InventoryState(pub Vec<InventoryItemUi>);

#[derive(Resource, Default, Debug, Clone)]
pub struct EquipmentState(
    pub std::collections::HashMap<packets::server::EquipmentSlot, packets::server::Equipment>,
);

#[derive(Resource, Default, Debug, Clone)]
pub struct PlayerProfileState {
    pub entity_id: Option<u32>,
    pub is_self: bool,
    pub name: String,
    pub class: String,
    pub guild: String,
    pub guild_rank: String,
    pub title: String,
    pub nation: packets::types::Nation,
    pub group_open: bool,
    pub group_string: RichText,
    pub profile_text: RichText,
    pub social_status: packets::types::SocialStatus,
    pub legend_marks: Vec<packets::types::LegendMarkInfo>,
    pub portrait: Option<Vec<u8>>,
    pub equipment:
        std::collections::HashMap<packets::server::EquipmentSlot, packets::types::ItemInfo>,
}

impl PlayerProfileState {
    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

#[derive(Resource, Default, Debug, Clone)]
pub struct AbilityState {
    pub skills: Vec<SkillUi>,
    pub spells: Vec<SpellUi>,
}

#[derive(Resource, Default, Debug, Clone)]
pub struct SkillCooldowns {
    pub cooldowns: std::collections::HashMap<u8, Cooldown>,
}

#[derive(Resource, Default, Debug, Clone)]
pub struct WorldListState {
    pub raw: Option<packets::server::WorldList>,
    pub filtered: Vec<WorldListMemberUi>,
    pub filter: WorldListFilter,
    pub version: u32,
}

#[derive(Debug, Clone, Default)]
pub struct PendingGroupInvite {
    pub source_name: String,
    pub group_name: String,
    pub group_note: String,
}

/// One group member: display name and whether the server marks them as leader (asterisk in SelfProfile).
pub type GroupMemberEntry = (String, bool);

#[derive(Resource, Default, Debug, Clone)]
pub struct GroupState {
    /// (display_name, is_leader_from_server). Leader line in group_string has "* " prefix.
    pub members: Vec<GroupMemberEntry>,
    pub is_groupable: bool,
    pub pending_invite: Option<PendingGroupInvite>,
}