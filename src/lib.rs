use bevy::prelude::*;

pub use game_ui::slint_types::{
    ChatMessage, Cooldown, DragDropState, EquipmentSlotData, GameState, HotbarEntry, InputBridge,
    InstallerState, InventoryItem, LegendMarkData, LobbyState, LoginBridge, LoginState, MainWindow,
    MenuEntry, NpcDialogData, NpcDialogState, ProfileData, SavedLoginItem, ServerItem, SettingsState, Skill,
    SlotPanelType, Spell, WorldLabel, WorldListMemberUi, WorldMapNode,
};

pub mod app_state;
pub mod audio;
pub mod ecs;
pub mod events;
pub mod game_files;
pub mod input;
pub mod map_store;
pub mod metafile_store;
pub mod network;
pub mod plugins;
pub mod render_plugin;
pub mod resources;
pub mod session;
pub mod session_prelogin;
pub mod settings;
pub mod settings_types;
pub mod slint_plugin;
pub mod slint_support;
pub mod webui;

pub fn storage_dir() -> std::path::PathBuf {
    let mut path = dirs::data_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    path.push("Talgonite");
    let _ = std::fs::create_dir_all(&path);
    path
}

pub fn server_dir(server_id: u32) -> std::path::PathBuf {
    let path = storage_dir().join("servers").join(server_id.to_string());
    let _ = std::fs::create_dir_all(&path);
    path
}

pub fn server_maps_dir(server_id: u32) -> std::path::PathBuf {
    let path = server_dir(server_id).join("maps");
    let _ = std::fs::create_dir_all(&path);
    path
}

pub fn server_metafile_dir(server_id: u32) -> std::path::PathBuf {
    let path = server_dir(server_id).join("metafile");
    let _ = std::fs::create_dir_all(&path);
    path
}

pub fn server_characters_dir(server_id: u32) -> std::path::PathBuf {
    let path = server_dir(server_id).join("characters");
    let _ = std::fs::create_dir_all(&path);
    path
}

pub use resources::{
    Camera, CreatureAssetStoreState, CreatureBatchState, EffectManagerState, ItemAssetStoreState,
    ItemBatchState, MapRendererState, PlayerAssetStoreState, PlayerBatchState, PlayerPortraitState,
    RendererState, WindowSurface,
};

#[derive(Resource)]
pub struct CurrentSession {
    pub username: String,
    pub server_id: u32,
    pub server_url: String,
}

pub struct CoreEventsPlugin;

impl Plugin for CoreEventsPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<events::MapEvent>()
            .add_message::<events::EntityEvent>()
            .add_message::<events::AudioEvent>()
            .add_message::<events::InventoryEvent>()
            .add_message::<events::AbilityEvent>()
            .add_message::<events::ChatEvent>()
            .add_message::<events::PlayerAction>()
            .add_message::<events::SessionEvent>()
            .add_message::<events::NetworkEvent>()
            // Interaction events
            .add_message::<events::EntityHoverEvent>()
            .add_message::<events::EntityClickEvent>()
            .add_message::<events::TileClickEvent>()
            .add_message::<events::WallClickEvent>();
    }
}

pub struct CorePlugin;

impl Plugin for CorePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            bevy::state::app::StatesPlugin,
            CoreEventsPlugin,
            settings::SettingsPlugin,
            ecs::plugin::GamePlugin,
            plugins::input::InputPlugin,
        ))
        .insert_resource(map_store::MapStore::new())
        .insert_resource(metafile_store::MetafileStore::new())
        .init_state::<app_state::AppState>()
        .add_systems(
            OnEnter(app_state::AppState::MainMenu),
            app_state::setup_game_files,
        )
        .add_systems(
            OnEnter(app_state::AppState::Installing),
            app_state::cleanup_game_files,
        )
        .add_systems(
            OnExit(app_state::AppState::InGame),
            (
                app_state::cleanup_ingame_world,
                app_state::cleanup_ingame_resources,
            ),
        );
    }
}
