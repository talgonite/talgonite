use crate::ToBytes;

use super::Codes;

#[derive(Debug)]
pub struct SynchronizeTicks {
    pub server_ticks: u32,
    pub client_ticks: u32,
}

impl ToBytes for SynchronizeTicks {
    const OPCODE: u8 = Codes::SynchronizeTicks as _;

    fn write_payload(&self, bytes: &mut Vec<u8>) {
        bytes.extend_from_slice(&self.server_ticks.to_be_bytes());
        bytes.extend_from_slice(&self.client_ticks.to_be_bytes());
    }
}
