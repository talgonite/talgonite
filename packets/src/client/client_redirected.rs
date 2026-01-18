use std::io::Read;

use crate::{ToBytes, TryFromBytes};

use super::Codes;
use byteorder::ReadBytesExt;
use encoding::all::WINDOWS_949;
use encoding::{EncoderTrap, Encoding};

#[derive(Debug)]
pub struct ClientRedirected {
    pub seed: u8,
    pub key: Vec<u8>,
    pub name: String,
    pub id: u32,
}

impl ToBytes for ClientRedirected {
    const OPCODE: u8 = Codes::ClientRedirected as _;

    fn write_payload(&self, bytes: &mut Vec<u8>) {
        bytes.push(self.seed);
        bytes.push(self.key.len() as u8);
        bytes.extend_from_slice(&self.key);
        let name_bytes = WINDOWS_949
            .encode(&self.name, EncoderTrap::Replace)
            .unwrap_or_default();
        bytes.push(name_bytes.len() as u8);
        bytes.extend_from_slice(&name_bytes);
        bytes.extend_from_slice(&self.id.to_be_bytes());
    }
}

impl TryFromBytes for ClientRedirected {
    fn try_from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let mut cursor = std::io::Cursor::new(bytes);
        let seed = cursor.read_u8()?;
        let key_len = cursor.read_u8()? as usize;
        let mut key = vec![0; key_len];
        cursor.read_exact(&mut key)?;
        let name_len = cursor.read_u8()? as usize;
        let mut name_buf = vec![0; name_len];
        cursor.read_exact(&mut name_buf)?;
        let name = WINDOWS_949
            .decode(&name_buf, encoding::DecoderTrap::Replace)
            .map_err(|e| anyhow::anyhow!("Failed to decode name: {}", e))?;
        let id = cursor.read_u32::<byteorder::BigEndian>()?;
        Ok(ClientRedirected {
            seed,
            key,
            name,
            id,
        })
    }
}
