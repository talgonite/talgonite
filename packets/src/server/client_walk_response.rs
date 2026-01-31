use crate::{TryFromBytes, types::Direction};
use byteorder::{BigEndian, ReadBytesExt};
use std::io::Cursor;

#[derive(Debug, Clone)]
pub enum ClientWalkResponseArgs {
    Rejected,
    Accepted(Direction),
}

#[derive(Debug, Clone)]
pub struct ClientWalkResponse {
    pub args: ClientWalkResponseArgs,
    pub from: (u16, u16),
}

impl TryFromBytes for ClientWalkResponse {
    fn try_from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let mut cursor = Cursor::new(bytes);
        let direction = cursor.read_u8()?;
        let from = {
            let x = cursor.read_u16::<BigEndian>()?;
            let y = cursor.read_u16::<BigEndian>()?;
            (x, y)
        };

        Ok(ClientWalkResponse {
            args: match Direction::try_from(direction) {
                Ok(dir) => ClientWalkResponseArgs::Accepted(dir),
                Err(_) => ClientWalkResponseArgs::Rejected,
            },
            from,
        })
    }
}
