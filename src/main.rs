#![windows_subsystem = "windows"]

use bevy::prelude::*;
use slint::ComponentHandle;

use talgonite::{render_plugin::GameRenderPlugin, session, slint_plugin, webui};

mod plugins {
    pub use talgonite::plugins::*;
}

fn main() {
    #[cfg(target_os = "windows")]
    unsafe {
        use windows_sys::Win32::System::Console::{ATTACH_PARENT_PROCESS, AttachConsole};
        AttachConsole(ATTACH_PARENT_PROCESS);
    }

    // Configure tracing to respect RUST_LOG if set, otherwise default to debug for our crate.
    // This ensures newly added debug! instrumentation is visible during troubleshooting.
    tracing_subscriber::fmt().with_target(false).try_init().ok();
    tracing::info!("Tracing initialized (debug enabled by default)");

    let mut app = App::new();
    app.add_message::<webui::plugin::UiOutbound>()
        .add_plugins(MinimalPlugins)
        .add_plugins(bevy::input::InputPlugin)
        .add_plugins((
            talgonite::CorePlugin,
            GameRenderPlugin,
            session::runtime::SessionRuntimePlugin,
            plugins::installer::InstallerPlugin,
            plugins::mouse_interaction::MouseInteractionPlugin,
            webui::plugin::UiBridgePlugin,
            slint_plugin::SlintBridgePlugin,
        ))
        .insert_resource(talgonite::audio::Audio::default());

    // Attach Slint UI and hand off control of the rendering notifier to the plugin.
    let slint_app = slint_plugin::attach_slint_ui(app);

    slint_app.run().unwrap();
}
