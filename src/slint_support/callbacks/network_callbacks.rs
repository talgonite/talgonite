use crossbeam_channel::Sender;
use slint::ComponentHandle;

use crate::webui::ipc::UiToCore;
use crate::{MainWindow, NetworkState};

pub fn wire_network_callbacks(slint_app: &MainWindow, tx: Sender<UiToCore>) {
    let slint_app_weak = slint_app.as_weak();
    slint_app.global::<NetworkState>().on_acknowledged(move || {
        if let Some(strong) = slint_app_weak.upgrade() {
            strong.global::<NetworkState>().set_disconnected(false);
        }
        let _ = tx.send(UiToCore::ReturnToMainMenu);
    });
}
