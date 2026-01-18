use std::sync::{Arc, Mutex};

use bevy::ecs::{resource::Resource, system::Res};
use network::EncryptedSender;
use packets::ToBytes;

#[derive(Resource, Default)]
pub struct PacketOutbox(Mutex<Vec<u8>>);

impl PacketOutbox {
    pub fn send<T: ToBytes>(&self, packet: &T) {
        if let Ok(mut outbox) = self.0.lock() {
            packet.write_to(&mut outbox);
        }
    }
}

#[derive(Resource)]
pub struct NetworkManager {
    pub sender: Arc<Mutex<EncryptedSender>>,
}

impl NetworkManager {
    pub fn new(sender: EncryptedSender) -> Self {
        Self {
            sender: Arc::new(Mutex::new(sender)),
        }
    }
}

pub fn flush_packet_outbox(outbox: Res<PacketOutbox>, net_mgr: Option<Res<NetworkManager>>) {
    let Some(net_mgr) = net_mgr else {
        if let Ok(mut outbox) = outbox.0.lock() {
            outbox.clear();
        }
        return;
    };

    if let Ok(mut data) = outbox.0.lock() {
        if data.is_empty() {
            return;
        }

        if let Ok(mut sender) = net_mgr.sender.lock() {
            let _ = futures_lite::future::block_on(sender.send(data.drain(..).as_slice()));
        }
    } else {
        return;
    };
}
