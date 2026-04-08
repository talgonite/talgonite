use bevy::prelude::*;

pub use game_ui::slint_types::{
    ChatMessage, Cooldown, DragDropState, EquipmentSlotData, GameState, GroupInviteNotification,
    GroupMember, HotbarEntry, InputBridge, InstallerState, InventoryItem, LegendMarkData,
    LobbyState, LoginBridge, LoginState, MainWindow, MenuEntry, NpcDialogData, NpcDialogState,
    PlatformState, ProfileData, SavedLoginItem, ServerItem, SettingsState, Skill, SlotPanelType,
    Spell, WorldLabel, WorldListMemberUi, WorldMapNode,
};

#[cfg(target_os = "android")]
use tracing_subscriber::prelude::*;

use slint::ComponentHandle;

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
pub mod rich_text;
pub mod session;
pub mod session_prelogin;
pub mod settings;
pub mod settings_types;
pub mod slint_plugin;
pub mod slint_support;
pub mod webui;

pub use resources::{
    Camera, CreatureAssetStoreState, CreatureBatchState, EffectManagerState, ItemAssetStoreState,
    ItemBatchState, MapRendererState, PlayerAssetStoreState, PlayerBatchState, PlayerPortraitState,
    RendererState, StorageConfig, WindowSurface,
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
            .add_message::<events::ResolvedPointerClickEvent>()
            .add_message::<events::InteractionIntentEvent>()
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
        .add_systems(Update, app_state::setup_game_files)
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

pub fn main_with_storage(storage_root: std::path::PathBuf) {
    init();

    let mut app = App::new();
    app.insert_resource(resources::StorageConfig::new(storage_root))
        .add_message::<webui::plugin::UiOutbound>()
        .add_plugins(MinimalPlugins)
        .add_plugins(bevy::input::InputPlugin)
        .add_plugins((
            CorePlugin,
            render_plugin::GameRenderPlugin,
            session::runtime::SessionRuntimePlugin,
            plugins::installer::InstallerPlugin,
            plugins::mouse_interaction::MouseInteractionPlugin,
            webui::plugin::UiBridgePlugin,
            slint_plugin::SlintBridgePlugin,
        ))
        .insert_resource(audio::Audio::default());

    // Attach Slint UI and hand off control of the rendering notifier to the plugin.
    let slint_app = slint_plugin::attach_slint_ui(app);

    let result = slint_app.run();

    // Explicitly drop slint_app to trigger cleanup of Bevy App before main exits.
    // This prevents "threads should not terminate unexpectedly" panics on shutdown
    // by ensuring TaskPool threads are joined before the process termination begins.
    drop(slint_app);

    result.unwrap();
}

fn init() {
    #[cfg(target_os = "windows")]
    unsafe {
        use windows_sys::Win32::System::Console::{ATTACH_PARENT_PROCESS, AttachConsole};
        AttachConsole(ATTACH_PARENT_PROCESS);
    }

    use tracing_subscriber::EnvFilter;
    #[cfg(target_os = "android")]
    use tracing_subscriber::layer::SubscriberExt;

    let filter = EnvFilter::new("info")
        .add_directive("wgpu_core=warn".parse().unwrap())
        .add_directive("wgpu_hal=warn".parse().unwrap())
        .add_directive("naga=warn".parse().unwrap())
        .add_directive("MESA=off".parse().expect("Failed to parse MESA directive"));

    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .without_time()
        .compact()
        .finish();

    // Upgrade logger on android
    #[cfg(target_os = "android")]
    let subscriber = {
        let android_layer = tracing_android::layer("talgonite").unwrap();
        subscriber.with(android_layer)
    };

    tracing::subscriber::set_global_default(subscriber).expect("Unable to set global subscriber");

    tracing::info!("Tracing initialized (debug enabled by default)");
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: slint::android::AndroidApp) {
    let storage_dir = app
        .internal_data_path()
        .expect("Internal data path not available");

    use slint::android::android_activity::{MainEvent, PollEvent};
    slint::android::init_with_event_listener(app, |event| {
        match event {
            PollEvent::Main(MainEvent::SaveState { saver, .. }) => {}
            PollEvent::Main(MainEvent::Resume { loader, .. }) => {}

            _ => {}
        };
    })
    .unwrap();

    main_with_storage(storage_dir);
}
