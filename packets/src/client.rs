mod begin_chant;
pub use begin_chant::BeginChant;

mod board_interaction;
pub use board_interaction::{
    BoardControls, BoardInteraction, BoardInteractionArgs, BoardRequestType,
};

mod spell_chant;
pub use spell_chant::SpellChant;

mod click;
pub use click::Click;

mod client_exception;
pub use client_exception::ClientException;

mod client_redirected;
pub use client_redirected::ClientRedirected;

mod client_walk;
pub use client_walk::ClientWalk;

mod create_char_finalize;
pub use create_char_finalize::CreateCharFinalize;

mod create_char_initial;
pub use create_char_initial::CreateCharInitial;

mod dialog_interaction;
pub use dialog_interaction::{DialogInteraction, DialogInteractionArgs};

mod display_entity_request;
pub use display_entity_request::DisplayEntityRequest;

mod editable_profile;
pub use editable_profile::EditableProfile;

mod emote;
pub use emote::Emote;

mod exchange_interaction;
pub use exchange_interaction::ExchangeInteraction;

mod exit_request;
pub use exit_request::ExitRequest;

mod gold_drop;
pub use gold_drop::GoldDrop;

mod gold_dropped_on_creature;
pub use gold_dropped_on_creature::GoldDroppedOnCreature;

mod group_invite;
pub use group_invite::GroupInvite;

mod heart_beat;
pub use heart_beat::HeartBeat;

mod homepage_request;
pub use homepage_request::HomepageRequest;

mod ignore;
pub use ignore::Ignore;

mod item_drop;
pub use item_drop::ItemDrop;

mod item_dropped_on_creature;
pub use item_dropped_on_creature::ItemDroppedOnCreature;

mod item_use;
pub use item_use::ItemUse;

mod login;
pub use login::Login;

mod map_data_request;
pub use map_data_request::MapDataRequest;

mod menu_interaction;
pub use menu_interaction::{MenuInteraction, MenuInteractionArgs};

mod meta_data_request;
pub use meta_data_request::MetaDataRequest;

mod notice_request;
pub use notice_request::NoticeRequest;

mod option_toggle;
pub use option_toggle::OptionToggle;

mod password_change;
pub use password_change::PasswordChange;

mod pickup;
pub use pickup::Pickup;

mod public_message;
pub use public_message::{PublicMessage, PublicMessageType};

mod raise_stat;
pub use raise_stat::RaiseStat;

mod refresh_request;
pub use refresh_request::RefreshRequest;

mod self_profile_request;
pub use self_profile_request::SelfProfileRequest;

mod sequence_change;
pub use sequence_change::SequenceChange;

mod server_table_request;
pub use server_table_request::ServerTableRequest;

mod set_notepad;
pub use set_notepad::SetNotepad;

mod skill_use;
pub use skill_use::SkillUse;

mod social_status;
pub use social_status::SocialStatus;

mod spacebar;
pub use spacebar::Spacebar;

mod spell_use;
pub use spell_use::{SpellUse, SpellUseArgs};

mod swap_slot;
pub use swap_slot::{SwapSlot, SwapSlotPanelType};

mod synchronize_ticks;
pub use synchronize_ticks::SynchronizeTicks;

mod toggle_group;
pub use toggle_group::ToggleGroup;

mod turn;
pub use turn::Turn;

mod unequip;
pub use unequip::Unequip;

mod version;
pub use version::Version;

mod whisper;
pub use whisper::Whisper;

mod world_list_request;
pub use world_list_request::WorldListRequest;

mod world_map_click;
pub use world_map_click::WorldMapClick;

#[repr(u8)]
pub enum Codes {
    /// <summary>
    ///     OpCode used when a client requests the encryption details, and a checksum of the details of available login servers
    /// </summary>
    Version = 0,

    /// <summary>
    ///     Opcode used when a client requests to create a new character. This is the first step in the process and will only
    ///     contain a name and password
    /// </summary>
    CreateCharInitial = 2,

    /// <summary>
    ///     OpCode used when a client provides credentials to log into the world
    /// </summary>
    Login = 3,

    /// <summary>
    ///     OpCode used when a client requests to create a new character. This is the second step in the process and will
    ///     contain appearance details
    /// </summary>
    CreateCharFinalize = 4,

    /// <summary>
    ///     OpCode used when a client requests tile data for the current map
    /// </summary>
    MapDataRequest = 5,

    /// <summary>
    ///     OpCode used when a client walks in a direction
    /// </summary>
    ClientWalk = 6,

    /// <summary>
    ///     OpCode used when a client picks up an item from the ground
    /// </summary>
    Pickup = 7,

    /// <summary>
    ///     OpCode used when a client drops an item on the ground
    /// </summary>
    ItemDrop = 8,

    /// <summary>
    ///     OpCode used when a client tries to log out
    /// </summary>
    ExitRequest = 11,

    /// <summary>
    ///     OpCode used when a client requests the server to display an object
    /// </summary>
    DisplayEntityRequest = 12,

    /// <summary>
    ///     OpCode used when a client ignores or un-ignores another player, or requests a list of ignored players
    /// </summary>
    Ignore = 13,

    /// <summary>
    ///     OpCode used when a client sends a publicly visible message
    /// </summary>
    PublicMessage = 14,

    /// <summary>
    ///     OpCode used when a client uses a spell
    /// </summary>
    SpellUse = 15,

    /// <summary>
    ///     OpCode used when a client is redirected to this server
    /// </summary>
    ClientRedirected = 16,

    /// <summary>
    ///     OpCode used when a client changes their character's direction
    /// </summary>
    Turn = 17,

    /// <summary>
    ///     OpCode used when a client presses their spacebar
    /// </summary>
    Spacebar = 19,

    /// <summary>
    ///     OpCode used when a client requests a list of all online players
    /// </summary>
    WorldListRequest = 24,

    /// <summary>
    ///     OpCode used when a client sends a private message to the server
    /// </summary>
    Whisper = 25,

    /// <summary>
    ///     OpCode used when a client toggles a user option
    /// </summary>
    OptionToggle = 27,

    /// <summary>
    ///     OpCode used when a client uses an item
    /// </summary>
    ItemUse = 28,

    /// <summary>
    ///     OpCode used when a client uses an emote
    /// </summary>
    Emote = 29,

    /// <summary>
    ///     OpCode used when a client sets persistent text on an object
    /// </summary>
    SetNotepad = 35,

    /// <summary>
    ///     OpCode used when a client drops gold on the ground
    /// </summary>
    GoldDrop = 36,

    /// <summary>
    ///     OpCode used when a client requests to change a character's password
    /// </summary>
    PasswordChange = 38,

    /// <summary>
    ///     OpCode used when a client drops an item on a creature
    /// </summary>
    ItemDroppedOnCreature = 41,

    /// <summary>
    ///     OpCode used when a client drops gold on a creature
    /// </summary>
    GoldDroppedOnCreature = 42,

    /// <summary>
    ///     OpCode used when a client requests their own profile
    /// </summary>
    SelfProfileRequest = 45,

    /// <summary>
    ///     OpCode used when a client invites another player to a group, responds to a group invite, or creates or destroys a
    ///     group box
    /// </summary>
    GroupInvite = 46,

    /// <summary>
    ///     OpCode used when a client toggles their group availability
    /// </summary>
    ToggleGroup = 47,

    /// <summary>
    ///     OpCode used when a client swaps two panel objects
    /// </summary>
    SwapSlot = 48,

    /// <summary>
    ///     OpCode used when a client refreshes their viewport
    /// </summary>
    RefreshRequest = 56,

    /// <summary>
    ///     OpCode used when a client responds to a merchant menu
    /// </summary>
    MenuInteraction = 57,

    /// <summary>
    ///     OpCode used when a client responds to a dialog
    /// </summary>
    DialogInteraction = 58,

    /// <summary>
    ///     OpCode used when a client accesses a board or mail
    /// </summary>
    BoardInteraction = 59,

    /// <summary>
    ///     OpCode used when a client uses a skill
    /// </summary>
    SkillUse = 62,

    /// <summary>
    ///     OpCode used when a client clicks on a world map node
    /// </summary>
    WorldMapClick = 63,

    /// <summary>
    ///     OpCode used when a client experiences an exception caused by a bad packet
    /// </summary>
    ClientException = 66,

    /// <summary>
    ///     OpCode used when a client clicks on an object
    /// </summary>
    Click = 67,

    /// <summary>
    ///     OpCode used when a client unequips an item
    /// </summary>
    Unequip = 68,

    /// <summary>
    ///     OpCode used when a client sends a heartbeat(keep-alive) ping
    /// </summary>
    HeartBeat = 69,

    /// <summary>
    ///     OpCode used when a client requests to raise a stat
    /// </summary>
    RaiseStat = 71,

    /// <summary>
    ///     OpCode used when a client interacts with an exchange window
    /// </summary>
    ExchangeInteraction = 74,

    /// <summary>
    ///     OpCode used when a client requests EULA details
    /// </summary>
    NoticeRequest = 75,

    /// <summary>
    ///     OpCode used when a client begins casting a spell with cast lines
    /// </summary>
    BeginChant = 77,

    /// <summary>
    ///     OpCode used when a client uses an ability that has chant lines
    /// </summary>
    Chant = 78,

    /// <summary>
    ///     OpCode used when a client responds to a request for profile data (portrait, text)
    /// </summary>
    EditableProfile = 79,

    /// <summary>
    ///     OpCode used when a client requests the details of available login servers
    /// </summary>
    ServerTableRequest = 87,

    /// <summary>
    ///     OpCode used when a client requests to change the packet sequence number
    /// </summary>
    SequenceChange = 98,

    /// <summary>
    ///     OpCode used when a client requests the url of the homepage
    /// </summary>
    HomepageRequest = 104,

    /// <summary>
    ///     OpCode used when a client sends it's Environment.Ticks value
    /// </summary>
    SynchronizeTicks = 117,

    /// <summary>
    ///     OpCode used when a client changes their social status
    /// </summary>
    SocialStatus = 121,

    /// <summary>
    ///     OpCode used when a client requests metadata details or data
    /// </summary>
    MetaDataRequest = 123,
}
