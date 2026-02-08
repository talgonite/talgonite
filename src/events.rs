use bevy::prelude::{Entity, Message, MouseButton};
use packets::{client, server};

use crate::ecs::components::Direction;

// === Network Events ===

#[derive(Debug, Clone, Message)]
pub enum MapEvent {
    Clear,
    SetInfo(server::MapInfo, std::sync::Arc<[u8]>),
    SetLightLevel(server::LightLevelKind),
    SetDoors(server::Door),
}

#[derive(Debug, Clone, Message)]
pub enum EntityEvent {
    PlayerLocation(server::Location),
    DisplayPlayer(server::display_player::DisplayPlayer),
    DisplayEntities(server::DisplayVisibleEntities),
    Remove(server::RemoveEntity),
    Walk(server::CreatureWalk),
    PlayerWalkResponse(server::ClientWalkResponse),
    Turn(server::EntityTurn),
    Animate(server::BodyAnimation),
    Effect(server::Animation),
    HealthBar(server::HealthBar),
}

#[derive(Debug, Clone, Message)]
pub enum AudioEvent {
    PlaySound(server::Sound),
    SetVolume(f32),
}

#[derive(Debug, Clone, Message)]
pub enum InventoryEvent {
    // Inbound
    Cooldown { slot: u8, cooldown_secs: u32 },
    Add(server::AddItemToPane),
    Remove(server::RemoveItemFromPane),
    Equipment(server::Equipment),
    DisplayUnequip(server::DisplayUnequip),
    // Outbound
    Swap { src: u8, dst: u8 },
    Use { slot: u8 },
    Unequip { slot: u8 },
}

#[derive(Debug, Clone, Message)]
pub enum HotbarEvent {}

#[derive(Debug, Clone, Message)]
pub enum AbilityEvent {
    SkillCooldown { slot: u8, cooldown_secs: u32 },
    AddSkill(server::AddSkillToPane),
    RemoveSkill(server::RemoveSkillFromPane),
    AddSpell(server::AddSpellToPane),
    RemoveSpell(server::RemoveSpellFromPane),
    // Outbound
    UseSkill { slot: u8 },
    UseSpell { slot: u8 },
}

#[derive(Debug, Clone, Message)]
pub enum ChatEvent {
    ServerMessage(server::ServerMessage),
    PublicMessage(server::DisplayPublicMessage),
    // Outbound
    SendPublicMessage(String, client::PublicMessageType), // (message, message_type)
    SendWhisper(String, String),                          // (target, message)
}

// === Input Events ===

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputSource {
    Manual,
    Pathfinding,
}

#[derive(Debug, Clone, Message)]
pub enum PlayerAction {
    Walk {
        direction: Direction,
        source: InputSource,
    },
    Turn {
        direction: Direction,
        source: InputSource,
    },
    ItemPickupBelow,
}

impl PlayerAction {
    pub fn is_manual(&self) -> bool {
        match self {
            PlayerAction::Walk { source, .. } => *source == InputSource::Manual,
            PlayerAction::Turn { source, .. } => *source == InputSource::Manual,
            PlayerAction::ItemPickupBelow => true,
        }
    }
}

// === Session Events ===

#[derive(Debug, Clone, Message)]
pub enum SessionEvent {
    PlayerId(u32),
    StatusEffects(Vec<server::Effect>),
    WorldMap(server::WorldMap),
    DisplayMenu(server::DisplayMenu),
    DisplayDialog(server::DisplayDialog),
    SelfProfile(server::SelfProfile),
    OtherProfile(server::OtherProfile),
    WorldList(server::WorldList),
}

#[derive(Debug, Clone, Message)]
pub enum NetworkEvent {
    Packet(server::Codes, Vec<u8>),
    Connected,
    Disconnected,
}

// === Interaction Events ===

/// Emitted when an entity is hovered by the mouse
#[derive(Debug, Clone, Message)]
pub struct EntityHoverEvent {
    pub entity: Entity,
}

/// Emitted when an entity is clicked
#[derive(Debug, Clone, Message)]
pub struct EntityClickEvent {
    pub entity: Entity,
    pub button: MouseButton,
    pub is_double_click: bool,
}

/// Emitted when empty ground is clicked
#[derive(Debug, Clone, Message)]
pub struct TileClickEvent {
    pub tile_x: i32,
    pub tile_y: i32,
    pub button: MouseButton,
}

/// Emitted when a wall is clicked
#[derive(Debug, Clone, Message)]
pub struct WallClickEvent {
    pub tile_x: i32,
    pub tile_y: i32,
    pub is_right: bool,
    pub button: MouseButton,
}
