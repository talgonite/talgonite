use bevy::prelude::*;

pub use game_ui::slint_types::{
    CastingIndicator, ChatMessage, ContextMenuEntry, ContextMenuState, Cooldown, DragDropState,
    EquipmentSlotData, GameState, GroupInviteNotification, GroupMember, HotbarEntry, InputBridge,
    InstallerState, InventoryItem, LegendMarkData, LobbyState, LoginBridge, LoginState,
    MailBoardPost, MailBoardState, MainWindow, MenuEntry, NetworkState, NpcDialogData,
    NpcDialogState, PlatformState, PopupId, PopupManagerState, ProfileData, SavedLoginItem,
    ServerItem, SettingsState, Skill, SlotPanelType, SocialStatus, SocialStatusEntry,
    SocialStatusState, SpeechBubble, Spell, WorldLabel, WorldListMemberUi, WorldMapNode,
};

use tracing_subscriber::prelude::*;

use slint::ComponentHandle;

pub mod app_state;
pub mod audio;
pub mod ecs;
pub mod events;
pub mod game_files;
pub mod input;
pub mod lighting;
pub mod map_store;
pub mod metafile_store;
pub mod minimap_assets;
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
pub mod weather;
pub mod webui;

pub use minimap_assets::{FULLSCREEN_MINIMAP_ASSETS, MinimapAssets};

pub use resources::{
    Camera, DarknessState, EffectManagerState, MapRendererState, MinimapCacheState,
    MinimapRendererState, PlayerPortraitState, PortraitRenderTarget, RendererState,
    SceneColorState, SpriteSceneState, StorageConfig, TranslucentPlayerPassState,
    UnifiedSpriteBatchState, WeatherState, WindowSurface,
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
    // Keep the guard alive for the whole process: dropping it is what finishes
    // writing the Chrome trace file (see init()).
    let trace_guard = init();

    // A single top-level span covering the whole session, so the trace has a
    // clear time ruler and load/decode/entity spans nest under it.
    let _app_span = tracing::info_span!("talgonite.app_run").entered();

    let mut app = App::new();
    app.insert_resource(resources::StorageConfig::new(storage_root))
        .add_message::<webui::plugin::UiOutbound>()
        .add_plugins(MinimalPlugins)
        .add_plugins(bevy::input::InputPlugin)
        .add_plugins((
            CorePlugin,
            plugins::input::InputPlugin,
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

    if let Some(guard) = trace_guard.as_ref() {
        tracing::info!("Flushing Chrome trace to disk");
        guard.flush();
    }

    result.unwrap();
}

fn init() -> Option<tracing_chrome::FlushGuard> {
    #[cfg(target_os = "windows")]
    unsafe {
        use windows_sys::Win32::System::Console::{ATTACH_PARENT_PROCESS, AttachConsole};
        AttachConsole(ATTACH_PARENT_PROCESS);
    }

    use tracing_subscriber::EnvFilter;

    // Debug builds keep the default at INFO (matching the static max level).
    // Release builds compile out everything below WARN, so default the filter
    // to WARN as well to avoid EnvFilter's "disabled statically" warning.
    #[cfg(debug_assertions)]
    let filter = EnvFilter::new("info")
        .add_directive("wgpu_core=warn".parse().unwrap())
        .add_directive("wgpu_hal=warn".parse().unwrap())
        .add_directive("naga=warn".parse().unwrap())
        .add_directive("MESA=off".parse().expect("Failed to parse MESA directive"));
    #[cfg(not(debug_assertions))]
    let filter = EnvFilter::new("warn")
        .add_directive("wgpu_core=warn".parse().unwrap())
        .add_directive("wgpu_hal=warn".parse().unwrap())
        .add_directive("naga=warn".parse().unwrap())
        .add_directive("MESA=off".parse().expect("Failed to parse MESA directive"));

    let fmt_layer = tracing_subscriber::fmt::layer().without_time().compact();

    // Debug builds only: when TALGONITE_TRACE is set, record every tracing
    // span (Bevy system spans from the `trace` feature, plus the
    // #[instrument] spans on load, decode, and entity construction) into a
    // Chrome trace JSON. Open it with https://ui.perfetto.dev or
    // chrome://tracing to view nested durations.
    #[cfg(all(not(target_os = "android"), debug_assertions))]
    let (subscriber, chrome_guard, trace_file) = {
        let base = tracing_subscriber::registry().with(filter).with(fmt_layer);
        match std::env::var_os("TALGONITE_TRACE") {
            Some(path) => {
                let file = if path.is_empty() || path == "1" {
                    std::path::PathBuf::from("talgonite-trace.json")
                } else {
                    std::path::PathBuf::from(path)
                };
                let (chrome_layer, guard) = tracing_chrome::ChromeLayerBuilder::new()
                    .file(&file)
                    .include_args(true)
                    .build();
                let trace_file = Some(file);
                (
                    Box::new(base.with(chrome_layer)) as Box<dyn tracing::Subscriber + Send + Sync>,
                    Some(guard),
                    trace_file,
                )
            }
            None => (
                Box::new(base) as Box<dyn tracing::Subscriber + Send + Sync>,
                None,
                None,
            ),
        }
    };

    #[cfg(all(not(target_os = "android"), not(debug_assertions)))]
    let subscriber: Box<dyn tracing::Subscriber + Send + Sync> =
        Box::new(tracing_subscriber::registry().with(filter).with(fmt_layer));

    // Upgrade logger on android
    #[cfg(target_os = "android")]
    let subscriber = {
        let android_layer = tracing_android::layer("talgonite").unwrap();
        tracing_subscriber::registry()
            .with(filter)
            .with(fmt_layer)
            .with(android_layer)
    };

    tracing::subscriber::set_global_default(subscriber).expect("Unable to set global subscriber");

    tracing::info!("Tracing initialized (debug enabled by default)");

    #[cfg(all(not(target_os = "android"), debug_assertions))]
    {
        if let Some(path) = trace_file {
            tracing::info!(
                trace_file = %path.display(),
                "Chrome trace recording enabled - exit the app to flush"
            );
        }
        return chrome_guard;
    }

    #[cfg(any(target_os = "android", not(debug_assertions)))]
    None
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: slint::android::AndroidApp) {
    let storage_dir = app
        .internal_data_path()
        .expect("Internal data path not available");

    let store = android_native_keyring_store::Store::new()
        .expect("Failed to initialize Android credentials store");
    keyring_core::set_default_store(store);

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
