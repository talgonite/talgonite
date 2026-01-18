use crate::TryFromBytes;
use byteorder::{BigEndian, ReadBytesExt};
use std::io::{Cursor, Read};

#[derive(Debug)]
pub struct ServerTableResponse {
    pub server_table: Vec<u8>,
}

impl TryFromBytes for ServerTableResponse {
    fn try_from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let mut cursor = Cursor::new(bytes);
        let server_table = {
            let mut buf = vec![0; cursor.read_u16::<BigEndian>()? as usize];
            cursor.read_exact(&mut buf)?;
            buf
        };
        Ok(ServerTableResponse { server_table })
    }
}
