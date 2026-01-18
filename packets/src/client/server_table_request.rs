use crate::ToBytes;

use super::Codes;

#[derive(Debug)]
pub enum ServerTableRequest {
    ServerId(u8),
    ServerList,
}

impl ToBytes for ServerTableRequest {
    const OPCODE: u8 = Codes::ServerTableRequest as _;

    fn write_payload(&self, bytes: &mut Vec<u8>) {
        match self {
            ServerTableRequest::ServerId(id) => {
                bytes.push(0);
                bytes.push(*id);
            }
            ServerTableRequest::ServerList => {
                bytes.push(1);
            }
        }
    }
}
