use std::{path::PathBuf, sync::Arc, thread};

use bevy::prelude::*;
use crossbeam_channel::{Receiver, Sender, unbounded};
use installer::InstallProgress;
use tracing::debug;

use crate::app_state::AppState;
use crate::storage_dir;

#[derive(Message, Debug, Clone)]
pub struct InstallerProgressEvent {
    pub percent: f32,
    pub message: Option<String>,
}

struct ProgressProxy {
    tx: Sender<InstallerProgressEvent>,
}

impl ::installer::InstallProgress for ProgressProxy {
    fn report(&self, percent: f32, message: String) {
        let _ = self.tx.send(InstallerProgressEvent {
            percent,
            message: Some(message),
        });
    }
}

#[derive(Resource)]
struct InstallerChannels {
    rx: Receiver<InstallerProgressEvent>,
}

#[derive(Resource)]
pub struct InstallerConfig {
    pub arx_path: PathBuf,
}

pub struct InstallerPlugin;

impl Plugin for InstallerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MaybeStartedInstaller>()
            .add_message::<InstallerProgressEvent>()
            .add_systems(OnEnter(AppState::Installing), start_installer_once)
            .add_systems(
                Update,
                (forward_installer_events, switch_on_complete)
                    .run_if(in_state(AppState::Installing)),
            );
    }
}

#[derive(Resource, Default)]
struct MaybeStartedInstaller(bool);

fn start_installer_once(
    mut commands: Commands,
    mut maybe_started: ResMut<MaybeStartedInstaller>,
    config: Option<Res<InstallerConfig>>,
) {
    if maybe_started.0 {
        return;
    }

    // Default path if not provided
    let arx_path = config
        .as_ref()
        .map(|c| c.arx_path.clone())
        .unwrap_or_else(|| {
            let mut path = storage_dir();
            let _ = std::fs::create_dir_all(&path);
            path.push("data.arx");
            path
        });

    let (tx, rx): (
        Sender<InstallerProgressEvent>,
        Receiver<InstallerProgressEvent>,
    ) = unbounded();

    let proxy = Arc::new(ProgressProxy { tx });

    // Spawn a background thread to run the blocking installer
    thread::spawn(move || {
        // Use the external workspace crate named `installer`, avoiding module name clash.
        let result = ::installer::install(&arx_path, Some(proxy.clone()));
        match result {
            Ok(()) => {
                proxy.report(1.0, "Install complete".to_string());
            }
            Err(e) => {
                proxy.report(1.0, format!("Install finished with error: {}", e));
            }
        }
    });

    commands.insert_resource(InstallerChannels { rx });
    maybe_started.0 = true;
}

fn forward_installer_events(
    channels: Option<Res<InstallerChannels>>,
    mut writer: MessageWriter<InstallerProgressEvent>,
) {
    let Some(ch) = channels else {
        return;
    };
    while let Ok(evt) = ch.rx.try_recv() {
        debug!(percent = evt.percent, message = ?evt.message, "installer progress event");
        writer.write(evt);
    }
}

fn switch_on_complete(
    mut reader: MessageReader<InstallerProgressEvent>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    for evt in reader.read() {
        if evt.percent >= 1.0 {
            next_state.set(AppState::MainMenu);
        }
    }
}
