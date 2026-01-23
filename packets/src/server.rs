use num_enum::TryFromPrimitive;

mod accept_connection;
pub use accept_connection::AcceptConnection;

mod add_item_to_pane;
pub use add_item_to_pane::AddItemToPane;

mod add_skill_to_pane;
pub use add_skill_to_pane::AddSkillToPane;

mod add_spell_to_pane;
pub use add_spell_to_pane::{AddSpellToPane, SpellType};

mod animation;
pub use animation::Animation;

mod attributes;
pub use attributes::Attributes;

mod body_animation;
pub use body_animation::BodyAnimation;

mod cancel_casting;
pub use cancel_casting::CancelCasting;

mod client_walk_response;
pub use client_walk_response::ClientWalkResponse;

mod connection_info;
pub use connection_info::ConnectionInfo;

mod cooldown;
pub use cooldown::{Cooldown, CooldownType};

mod creature_turn;
pub use creature_turn::CreatureTurn;

mod creature_walk;
pub use creature_walk::CreatureWalk;

pub mod display_player;
pub use display_player::DisplayPlayer;

mod display_board;
pub use display_board::DisplayBoard;

mod display_dialog;
pub use display_dialog::{DisplayDialog, DisplayDialogPayload};

mod display_exchange;
pub use display_exchange::DisplayExchange;

mod display_group_invite;
pub use display_group_invite::DisplayGroupInvite;

pub mod display_menu;
pub use display_menu::DisplayMenu;

mod display_public_message;
pub use display_public_message::{DisplayPublicMessage, PublicMessageType};

mod display_unequip;
pub use display_unequip::DisplayUnequip;

mod display_visible_entities;
pub use display_visible_entities::{DisplayVisibleEntities, EntityInfo, VisibleEntityType};

mod door;
pub use door::{Door, DoorInstance};

mod editable_profile_request;
pub use editable_profile_request::EditableProfileRequest;

mod effect;
pub use effect::Effect;

mod equipment;
pub use crate::types::EquipmentSlot;
pub use equipment::{Equipment, ITEM_SPRITE_OFFSET};

mod exit_response;
pub use exit_response::ExitResponse;

mod force_client_packet;
pub use force_client_packet::ForceClientPacket;

mod health_bar;
pub use health_bar::HealthBar;

mod heart_beat_response;
pub use heart_beat_response::HeartBeatResponse;

mod light_level;
pub use light_level::{LightLevel, LightLevelKind};

mod location;
pub use location::Location;

mod login_control;
pub use login_control::LoginControl;

mod login_message;
pub use login_message::{LoginMessage, LoginMessageType};

mod login_notice;
pub use login_notice::LoginNotice;

mod map_change_complete;
pub use map_change_complete::MapChangeComplete;

mod map_change_pending;
pub use map_change_pending::MapChangePending;

mod map_data;
pub use map_data::MapData;

mod map_info;
pub use map_info::MapInfo;

mod map_load_complete;
pub use map_load_complete::MapLoadComplete;

mod meta_data;
pub use meta_data::MetaData;

mod notepad;
pub use notepad::Notepad;

mod other_profile;
pub use other_profile::OtherProfile;

pub mod redirect;
pub use redirect::{Redirect, RedirectServerType};

mod refresh_response;
pub use refresh_response::RefreshResponse;

mod remove_entity;
pub use remove_entity::RemoveEntity;

mod remove_item_from_pane;
pub use remove_item_from_pane::RemoveItemFromPane;

mod remove_skill_from_pane;
pub use remove_skill_from_pane::RemoveSkillFromPane;

mod remove_spell_from_pane;
pub use remove_spell_from_pane::RemoveSpellFromPane;

mod self_profile;
pub use self_profile::SelfProfile;

mod server_message;
pub use server_message::{ServerMessage, ServerMessageType};

mod server_table_response;
pub use server_table_response::ServerTableResponse;

mod sound;
pub use sound::Sound;

mod synchronize_ticks_response;
pub use synchronize_ticks_response::SynchronizeTicksResponse;

mod user_id;
pub use user_id::UserId;

mod world_list;
pub use world_list::{BaseClass, SocialStatus, WorldList, WorldListColor, WorldListMember};

mod world_map;
pub use world_map::WorldMap;

/// <summary>
///     OpCodes used when sending packets to a client
/// </summary>
/// <remarks>
///     In networking, an opcode is used to identify the type of packet that is being sent. The opcode is generally part of
///     a header near the beginning of the data stream
/// </remarks>
#[repr(u8)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, TryFromPrimitive)]
pub enum Codes {
    /// <summary>
    ///     OpCode used to send the encryption details and checksum of the details of available login servers
    /// </summary>
    ConnectionInfo = 0,

    /// <summary>
    ///     OpCode used to send a message to a client on the login server
    /// </summary>
    LoginMessage = 2,

    /// <summary>
    ///     OpCode used to redirect a client to another server
    /// </summary>
    Redirect = 3,

    /// <summary>
    ///     OpCode used to send a client it's location
    /// </summary>
    Location = 4,

    /// <summary>
    ///     OpCode used to send a client it's id
    /// </summary>
    UserId = 5,

    /// <summary>
    ///     OpCode used to send a client all non-aisling objects in it's viewport
    /// </summary>
    DisplayVisibleEntities = 7,

    /// <summary>
    ///     OpCode used to send a client it's attributes
    /// </summary>
    Attributes = 8,

    /// <summary>
    ///     OpCode used to send a client a non-public message
    /// </summary>
    ServerMessage = 10,

    /// <summary>
    ///     OpCode used to respond to a client's request to walk
    /// </summary>
    ClientWalkResponse = 11,

    /// <summary>
    ///     OpCode used to send a client another creature's walk
    /// </summary>
    CreatureWalk = 12,

    /// <summary>
    ///     OpCode used to send a client a public message
    /// </summary>
    DisplayPublicMessage = 13,

    /// <summary>
    ///     OpCode used to remove an object from the client's viewport
    /// </summary>
    RemoveEntity = 14,

    /// <summary>
    ///     OpCode used to add an item to the client's inventory
    /// </summary>
    AddItemToPane = 15,

    /// <summary>
    ///     OpCode used to remove an item from the client's inventory
    /// </summary>
    RemoveItemFromPane = 16,

    /// <summary>
    ///     OpCode used to send a client another creature's turn
    /// </summary>
    CreatureTurn = 17,

    /// <summary>
    ///     OpCode used to display a creature's health bar
    /// </summary>
    HealthBar = 19,

    /// <summary>
    ///     OpCode used to send map details to a client
    /// </summary>
    MapInfo = 21,

    /// <summary>
    ///     OpCode used to add a spell to the client's spellbook
    /// </summary>
    AddSpellToPane = 23,

    /// <summary>
    ///     OpCode used to remove a spell from the client's spellbook
    /// </summary>
    RemoveSpellFromPane = 24,

    /// <summary>
    ///     OpCode used to tell the client to play a sound
    /// </summary>
    Sound = 25,

    /// <summary>
    ///     OpCode used to animate a creature's body
    /// </summary>
    BodyAnimation = 26,

    /// <summary>
    ///     OpCode used to associate text with an object
    /// </summary>
    Notepad = 27,

    Unknown = 30,

    /// <summary>
    ///     OpCode used to signal to the client that a map change operation has completed
    /// </summary>
    MapChangeComplete = 31,

    /// <summary>
    ///     OpCode used to change the light level of the map
    /// </summary>
    LightLevel = 32,

    /// <summary>
    ///     OpCode used to respond to a client's request to refresh the viewport
    /// </summary>
    RefreshResponse = 34,

    /// <summary>
    ///     OpCode used to play an animation on a point or object
    /// </summary>
    Animation = 41,

    /// <summary>
    ///     OpCode used to add a skill to a client's skillbook
    /// </summary>
    AddSkillToPane = 44,

    /// <summary>
    ///     OpCode used to remove a skill from a client's skillbook
    /// </summary>
    RemoveSkillFromPane = 45,

    /// <summary>
    ///     OpCode used to transition a client to the world map
    /// </summary>
    WorldMap = 46,

    /// <summary>
    ///     OpCode used to display a merchant menu to a client
    /// </summary>
    DisplayMenu = 47,

    /// <summary>
    ///     OpCode used to display a dialog to a client
    /// </summary>
    DisplayDialog = 48,

    /// <summary>
    ///     OpCode used to display a board to a client
    /// </summary>
    DisplayBoard = 49,

    /// <summary>
    ///     OpCode used to give details of nearby doors to a client
    /// </summary>
    Door = 50,

    /// <summary>
    ///     OpCode used to display an aisling to a client
    /// </summary>
    DisplayPlayer = 51,

    /// <summary>
    ///     OpCode used to display an aisling's profile to a client
    /// </summary>
    OtherProfile = 52,

    /// <summary>
    ///     OpCode used to display the world list to a client
    /// </summary>
    WorldList = 54,

    /// <summary>
    ///     OpCode used to send a client a change in an equipment slot
    /// </summary>
    Equipment = 55,

    /// <summary>
    ///     OpCode used to send a client a removal from an equipment slot
    /// </summary>
    DisplayUnequip = 56,

    /// <summary>
    ///     OpCode used to send a client it's own profile
    /// </summary>
    SelfProfile = 57,

    /// <summary>
    ///     OpCode used to display an effect on the bar on the right hand side of the viewport
    /// </summary>
    Effect = 58,

    /// <summary>
    ///     OpCode used to respond to a client's heartbeat
    /// </summary>
    HeartBeatResponse = 59,

    /// <summary>
    ///     OpCode used to send a client tile data for a map
    /// </summary>
    MapData = 60,

    /// <summary>
    ///     OpCode used to send a skill or spell cooldown to a client
    /// </summary>
    Cooldown = 63,

    /// <summary>
    ///     OpCode used to send data displayed int an exchange window to a client
    /// </summary>
    DisplayExchange = 66,

    /// <summary>
    ///     OpCode used to tell a client to cancel a spellcast
    /// </summary>
    CancelCasting = 72,

    /// <summary>
    ///     OpCode used to request profile details from a client
    /// </summary>
    EditableProfileRequest = 73,

    /// <summary>
    ///     OpCode used to force a client to send back a specified packet
    /// </summary>
    ForceClientPacket = 75,

    /// <summary>
    ///     OpCode used to send a client confirmation of a request to exit to the login server
    /// </summary>
    ExitResponse = 76,

    /// <summary>
    ///     OpCode used to send a client a list of available servers
    /// </summary>
    ServerTableResponse = 86,

    /// <summary>
    ///     OpCode used to signal a client that it has finished sending map data
    /// </summary>
    MapLoadComplete = 88,

    /// <summary>
    ///     OpCode used to send a client the EULA / login notice
    /// </summary>
    LoginNotice = 96,

    /// <summary>
    ///     OpCode used to send a client a group request
    /// </summary>
    DisplayGroupInvite = 99,

    /// <summary>
    ///     OpCode used to send a client data for the login screen
    /// </summary>
    LoginControl = 102,

    /// <summary>
    ///     OpCode used to signal a client that it is changing maps
    /// </summary>
    MapChangePending = 103,

    /// <summary>
    ///     OpCode used to synchronize the server's ticks with a client's
    /// </summary>
    SynchronizeTicksResponse = 104,

    /// <summary>
    ///     OpCode used to send metadata data to a client
    /// </summary>
    MetaData = 111,

    /// <summary>
    ///     OpCode sent to a client to confirm their initial connection
    /// </summary>
    AcceptConnection = 126,
}
