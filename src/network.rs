use bevy::ecs::resource::Resource;
use packets::ToBytes;

#[derive(Resource, Clone)]
pub struct PacketOutbox(pub async_channel::Sender<Vec<u8>>);

impl Default for PacketOutbox {
    fn default() -> Self {
        let (tx, _rx) = async_channel::unbounded();
        Self(tx)
    }
}

impl PacketOutbox {
    pub fn send<T: ToBytes>(&self, packet: &T) {
        let _ = self.0.try_send(packet.to_bytes());
    }
}
