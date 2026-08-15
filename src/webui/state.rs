use bevy::prelude::*;

use game_ui::{
    BoardEntryUi, BoardPostUi, BoardStateUi, Cooldown, CoreToUi, InventoryItemUi, SkillUi, SpellUi,
    WorldListFilter, WorldListMemberUi,
};
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
    pub boards: Vec<BoardEntryUi>,
    pub selected_board_index: i32,
    pub requested_board_id: Option<u16>,
    pub abandoned_page_request: Option<(u16, i16)>,
    pub can_post: bool,
    pub board_list_mode: bool,
    pub board_loading: bool,
    pub selected_index: i32,
    pub loading_post_id: i32,
    pub last_post_id: Option<i16>,
    pub pending_request_session_token: Option<i32>,
    pub pending_board_id: Option<u16>,
    pub pending_start_post_id: Option<i16>,
    pub deleting_post_id: Option<i32>,
    pub delete_requested_at: Option<std::time::Duration>,
    pub compose_open: bool,
    pub compose_reply_to: Option<String>,
    pub compose_title: String,
    pub compose_subject: String,
    pub compose_waiting: bool,
    pub compose_result: String,
    pub compose_submitted_at: Option<std::time::Duration>,
}

impl BoardSessionState {
    pub(crate) fn invalidate(&mut self) {
        self.session_token = self.session_token.wrapping_add(1);
        self.visible = false;
        self.active_board_id = None;
        self.board_name.clear();
        self.posts.clear();
        self.boards.clear();
        self.selected_board_index = -1;
        self.requested_board_id = None;
        self.abandoned_page_request = self.pending_board_id.zip(self.pending_start_post_id);
        self.can_post = false;
        self.board_list_mode = false;
        self.board_loading = false;
        self.selected_index = -1;
        self.loading_post_id = -1;
        self.last_post_id = None;
        self.pending_request_session_token = None;
        self.pending_board_id = None;
        self.pending_start_post_id = None;
        self.deleting_post_id = None;
        self.delete_requested_at = None;
        self.reset_compose();
    }

    pub(crate) fn open_board_list(&mut self) {
        self.boards.clear();
        self.selected_board_index = -1;
        self.active_board_id = None;
        self.board_name = "Boards".to_string();
        self.posts.clear();
        self.board_loading = true;
        self.board_list_mode = true;
        self.can_post = true;
        self.visible = false;
        self.selected_index = -1;
        self.loading_post_id = -1;
        self.last_post_id = None;
        self.pending_request_session_token = None;
        self.pending_board_id = None;
        self.pending_start_post_id = None;
        self.deleting_post_id = None;
        self.delete_requested_at = None;
        self.requested_board_id = Some(0);
        self.abandoned_page_request = None;
    }

    pub(crate) fn select_board(&mut self, board_id: u16, board_name: String) {
        self.requested_board_id = Some(board_id);
        self.abandoned_page_request = None;
        self.pending_request_session_token = None;
        self.pending_board_id = None;
        self.pending_start_post_id = None;
        self.active_board_id = None;
        self.board_name = board_name;
        self.posts.clear();
        self.board_loading = true;
        self.selected_index = -1;
        self.loading_post_id = -1;
        self.last_post_id = None;
        self.deleting_post_id = None;
        self.delete_requested_at = None;
    }

    pub(crate) fn clear_pending(&mut self) {
        self.pending_request_session_token = None;
        self.pending_board_id = None;
        self.pending_start_post_id = None;
    }

    pub(crate) fn reset_compose(&mut self) {
        self.compose_open = false;
        self.compose_reply_to = None;
        self.compose_title.clear();
        self.compose_subject.clear();
        self.compose_waiting = false;
        self.compose_result.clear();
        self.compose_submitted_at = None;
    }

    pub(crate) fn to_display_board_ui(&self, append: bool) -> BoardStateUi {
        BoardStateUi {
            visible: self.visible,
            board_name: self.board_name.clone(),
            can_post: self.can_post,
            board_list_mode: self.board_list_mode,
            board_loading: self.board_loading,
            selected_index: self.selected_index,
            loading_post_id: self.loading_post_id,
            session_token: self.session_token,
            append,
            posts: self.posts.clone(),
        }
    }

    pub(crate) fn to_compose_msg(&self) -> CoreToUi {
        let is_mail = self.active_board_id == Some(0);
        let (name, name_editable, name_label) = match (&self.compose_reply_to, is_mail) {
            (Some(to), _) => (to.clone(), false, "To"),
            (None, true) => (String::new(), true, "To"),
            (None, false) => (String::new(), false, "From"),
        };
        CoreToUi::BoardComposeUpdate {
            visible: self.compose_open,
            title: self.compose_title.clone(),
            name_label: name_label.to_string(),
            name,
            name_editable,
            subject: self.compose_subject.clone(),
            waiting: self.compose_waiting,
            result: self.compose_result.clone(),
        }
    }

    pub(crate) fn to_delete_msg(&self, message: impl Into<String>) -> CoreToUi {
        CoreToUi::BoardDeleteUpdate {
            deleting_post_id: self.deleting_post_id.unwrap_or(-1),
            message: message.into(),
        }
    }

    pub(crate) fn mark_request(&mut self, board_id: u16, start_post_id: i16) {
        self.pending_request_session_token = Some(self.session_token);
        self.pending_board_id = Some(board_id);
        self.pending_start_post_id = Some(start_post_id);
    }

    /// A page request is answered with posts at or below the requested id, so
    /// anything above it (or anything while no page request is pending) is a
    /// fresh board response.
    pub(crate) fn is_page_response(&self, board_id: u16, posts: &[BoardPostUi]) -> bool {
        self.pending_board_id == Some(board_id)
            && self.pending_start_post_id.is_some_and(|start| {
                posts.is_empty() || posts.iter().all(|post| post.post_id <= i32::from(start))
            })
    }

    pub(crate) fn merge_page(&mut self, mut page: Vec<BoardPostUi>) -> Vec<BoardPostUi> {
        let mut added = Vec::with_capacity(page.len());
        for post in &mut page {
            self.merge_cached_post(post);
            if self.posts.iter().any(|entry| entry.post_id == post.post_id) {
                continue;
            }
            let insert_at = self
                .posts
                .partition_point(|entry| entry.post_id > post.post_id);
            self.posts.insert(insert_at, post.clone());
            added.push(post.clone());
        }
        added
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
