//! Centralized LIFO popup/window stack. [`PopupManager`] is the source of
//! truth; Slint renders it via `PopupLayer` and the synced `open-popups` model.
//!
//! `PopupId` is mirrored in `game-ui/ui/popup_manager.slint` — when adding a
//! popup, keep the two enums, `from_slint`/`to_slint`, and the `*-open` sync
//! flags in `sync_popup_to_slint` in step.

use bevy::prelude::*;

/// Presentation semantics (dim backdrop + input blocking); order comes from the
/// LIFO stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopupKind {
    /// Full-screen dim backdrop, blocks input to everything behind it.
    Modal,
    /// Side panel / window. No dim, does not block clicks outside.
    Window,
    /// Anchored overlay (context menu, dropdown). Click-away closes.
    Popover,
    /// Transient toast notification (auto/button dismissed).
    Toast,
}

/// Identifier for every popup/window in the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PopupId {
    // In-game
    GameMenu,
    Settings,
    NpcDialog,
    Profile,
    MailBoard,
    WorldMap,
    Inventory,
    Skills,
    Spells,
    WorldList,
    Group,
    ContextMenu,
    GroupInvite,
    // Prelogin
    LoginModal,
    ServerManager,
    CharacterCreation,
}

impl PopupId {
    /// The presentation category of this popup.
    pub fn kind(self) -> PopupKind {
        use PopupId::*;
        match self {
            GameMenu | Settings | NpcDialog | Profile | MailBoard | WorldMap | LoginModal
            | ServerManager | CharacterCreation => PopupKind::Modal,
            Inventory | Skills | Spells | WorldList | Group => PopupKind::Window,
            ContextMenu => PopupKind::Popover,
            GroupInvite => PopupKind::Toast,
        }
    }

    /// Whether Escape closes this popup (toasts stay until dismissed).
    pub fn escape_closable(self) -> bool {
        self.kind() != PopupKind::Toast
    }

    /// Convert from the Slint-generated `PopupId` (mirror of this enum).
    pub fn from_slint(id: crate::PopupId) -> PopupId {
        use crate::PopupId as S;
        match id {
            S::GameMenu => PopupId::GameMenu,
            S::Settings => PopupId::Settings,
            S::NpcDialog => PopupId::NpcDialog,
            S::Profile => PopupId::Profile,
            S::MailBoard => PopupId::MailBoard,
            S::WorldMap => PopupId::WorldMap,
            S::Inventory => PopupId::Inventory,
            S::Skills => PopupId::Skills,
            S::Spells => PopupId::Spells,
            S::WorldList => PopupId::WorldList,
            S::Group => PopupId::Group,
            S::ContextMenu => PopupId::ContextMenu,
            S::GroupInvite => PopupId::GroupInvite,
            S::LoginModal => PopupId::LoginModal,
            S::ServerManager => PopupId::ServerManager,
            S::CharacterCreation => PopupId::CharacterCreation,
        }
    }

    /// Convert to the Slint-generated `PopupId` (mirror of this enum).
    pub fn to_slint(self) -> crate::PopupId {
        use crate::PopupId as S;
        match self {
            PopupId::GameMenu => S::GameMenu,
            PopupId::Settings => S::Settings,
            PopupId::NpcDialog => S::NpcDialog,
            PopupId::Profile => S::Profile,
            PopupId::MailBoard => S::MailBoard,
            PopupId::WorldMap => S::WorldMap,
            PopupId::Inventory => S::Inventory,
            PopupId::Skills => S::Skills,
            PopupId::Spells => S::Spells,
            PopupId::WorldList => S::WorldList,
            PopupId::Group => S::Group,
            PopupId::ContextMenu => S::ContextMenu,
            PopupId::GroupInvite => S::GroupInvite,
            PopupId::LoginModal => S::LoginModal,
            PopupId::ServerManager => S::ServerManager,
            PopupId::CharacterCreation => S::CharacterCreation,
        }
    }
}

/// The authoritative popup stack (bottom -> top). The topmost entry renders
/// above everything else and is what Escape closes first.
#[derive(Resource, Default, Debug)]
pub struct PopupManager {
    stack: Vec<PopupId>,
}

impl PopupManager {
    /// Open a popup, bringing it to the front if already open. Returns whether
    /// it is now open.
    ///
    /// Window/Popover opens are ignored while a Modal is topmost (modal
    /// exclusivity); Modals may stack on top of each other.
    pub fn open(&mut self, id: PopupId) -> bool {
        if let Some(pos) = self.stack.iter().position(|&e| e == id) {
            let entry = self.stack.remove(pos);
            self.stack.push(entry);
            return true;
        }
        if matches!(id.kind(), PopupKind::Window | PopupKind::Popover) {
            if let Some(&top) = self.stack.last() {
                if top.kind() == PopupKind::Modal {
                    return false;
                }
            }
        }
        self.stack.push(id);
        true
    }

    /// Close a specific popup, wherever it is in the stack.
    pub fn close(&mut self, id: PopupId) -> bool {
        let before = self.stack.len();
        self.stack.retain(|&e| e != id);
        self.stack.len() != before
    }

    /// Close the topmost escape-closable popup, skipping (and leaving) toasts.
    pub fn close_top(&mut self) -> Option<PopupId> {
        let pos = self
            .stack
            .iter()
            .rposition(|&e| e.escape_closable())?;
        Some(self.stack.remove(pos))
    }

    pub fn top(&self) -> Option<PopupId> {
        self.stack.last().copied()
    }

    pub fn is_open(&self, id: PopupId) -> bool {
        self.stack.contains(&id)
    }

    /// Close everything, returning them top -> bottom.
    pub fn clear(&mut self) -> Vec<PopupId> {
        self.stack.drain(..).rev().collect()
    }

    /// Popups bottom -> top (Slint render order; later = on top).
    pub fn open_ids(&self) -> impl Iterator<Item = PopupId> + '_ {
        self.stack.iter().copied()
    }

    pub fn len(&self) -> usize {
        self.stack.len()
    }

    pub fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_close_is_open() {
        let mut m = PopupManager::default();
        assert!(m.is_empty());
        assert!(m.open(PopupId::Settings));
        assert!(m.is_open(PopupId::Settings));
        assert_eq!(m.top(), Some(PopupId::Settings));
        assert_eq!(m.len(), 1);
        assert!(m.close(PopupId::Settings));
        assert!(!m.is_open(PopupId::Settings));
        assert!(m.is_empty());
        assert!(!m.close(PopupId::Settings));
    }

    #[test]
    fn open_brings_existing_to_front() {
        let mut m = PopupManager::default();
        m.open(PopupId::Inventory);
        m.open(PopupId::Settings);
        // Re-open inventory: it should move above settings.
        m.open(PopupId::Inventory);
        assert_eq!(m.top(), Some(PopupId::Inventory));
        assert_eq!(
            m.open_ids().collect::<Vec<_>>(),
            vec![PopupId::Settings, PopupId::Inventory]
        );
    }

    #[test]
    fn close_top_is_lifo() {
        let mut m = PopupManager::default();
        m.open(PopupId::Settings);
        m.open(PopupId::NpcDialog);
        m.open(PopupId::WorldMap);
        assert_eq!(m.close_top(), Some(PopupId::WorldMap));
        assert_eq!(m.close_top(), Some(PopupId::NpcDialog));
        assert_eq!(m.close_top(), Some(PopupId::Settings));
        assert_eq!(m.close_top(), None);
    }

    #[test]
    fn modal_exclusivity_blocks_windows() {
        let mut m = PopupManager::default();
        m.open(PopupId::Settings); // Modal
        assert!(!m.open(PopupId::Inventory)); // Window ignored
        assert!(!m.is_open(PopupId::Inventory));
        // Popover also ignored.
        assert!(!m.open(PopupId::ContextMenu));
        // Modal-over-Modal is allowed.
        assert!(m.open(PopupId::NpcDialog));
        assert_eq!(m.top(), Some(PopupId::NpcDialog));
    }

    #[test]
    fn windows_allow_windows() {
        let mut m = PopupManager::default();
        m.open(PopupId::Inventory);
        assert!(m.open(PopupId::Skills));
        assert_eq!(m.top(), Some(PopupId::Skills));
    }

    #[test]
    fn close_specific_keeps_order() {
        let mut m = PopupManager::default();
        m.open(PopupId::Inventory);
        m.open(PopupId::Skills);
        m.open(PopupId::Spells);
        assert!(m.close(PopupId::Skills));
        assert_eq!(
            m.open_ids().collect::<Vec<_>>(),
            vec![PopupId::Inventory, PopupId::Spells]
        );
    }

    #[test]
    fn clear_empties_stack() {
        let mut m = PopupManager::default();
        m.open(PopupId::Inventory);
        m.open(PopupId::NpcDialog);
        let closed = m.clear();
        assert_eq!(closed.len(), 2);
        assert!(m.is_empty());
        assert!(!m.is_open(PopupId::NpcDialog));
    }

    #[test]
    fn close_top_skips_toasts() {
        let mut m = PopupManager::default();
        m.open(PopupId::Settings);
        m.open(PopupId::GroupInvite); // Toast (not escape-closable), on top
        assert_eq!(m.top(), Some(PopupId::GroupInvite));
        // Escape closes the settings below the toast, leaving the toast.
        assert_eq!(m.close_top(), Some(PopupId::Settings));
        assert_eq!(m.top(), Some(PopupId::GroupInvite));
        // Toast alone: close_top returns None and leaves it.
        assert_eq!(m.close_top(), None);
        assert!(m.is_open(PopupId::GroupInvite));
    }

    #[test]
    fn kind_assignments() {
        assert_eq!(PopupId::Settings.kind(), PopupKind::Modal);
        assert_eq!(PopupId::NpcDialog.kind(), PopupKind::Modal);
        assert_eq!(PopupId::Inventory.kind(), PopupKind::Window);
        assert_eq!(PopupId::ContextMenu.kind(), PopupKind::Popover);
        assert_eq!(PopupId::GroupInvite.kind(), PopupKind::Toast);
        assert!(!PopupId::GroupInvite.escape_closable());
    }
}
