use byteorder::{BigEndian, LittleEndian, ReadBytesExt};
use std::io::{Cursor, Read};
use std::net::{Ipv4Addr, SocketAddrV4};

use crate::{ToBytes, TryFromBytes};
use anyhow::anyhow;
use encoding::all::WINDOWS_949;
use encoding::{DecoderTrap, EncoderTrap, Encoding};

pub const OPCODE: u8 = super::Codes::Redirect as u8;

#[derive(Debug, PartialEq)]
pub struct Redirect {
    pub addr: SocketAddrV4,
    pub server_type: RedirectServerType,
    pub seed: u8,
    pub key: Vec<u8>,
    pub name: String,
    pub id: u32,
}

#[derive(Debug, PartialEq)]
pub enum RedirectServerType {
    Game,
    Login,
}

impl TryFromBytes for Redirect {
    fn try_from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let mut cursor = Cursor::new(bytes);
        let ip = SocketAddrV4::new(
            Ipv4Addr::from_bits(cursor.read_u32::<LittleEndian>()?),
            cursor.read_u16::<BigEndian>()?,
        );
        let server_type = match cursor.read_u8()? {
            0x19 => RedirectServerType::Game,
            _ => RedirectServerType::Login,
        };
        let seed = cursor.read_u8()?;
        let key = {
            let mut buf = vec![0; cursor.read_u8()? as usize];
            cursor.read_exact(&mut buf)?;
            buf
        };
        let name = {
            let mut buf = vec![0; cursor.read_u8()? as usize];
            cursor.read_exact(&mut buf)?;
            WINDOWS_949
                .decode(&buf, DecoderTrap::Replace)
                .map_err(|e| anyhow!("Failed to decode name: {}", e))?
        };
        let id = cursor.read_u32::<BigEndian>()?;
        Ok(Redirect {
            addr: ip,
            server_type,
            seed,
            key,
            name,
            id,
        })
    }
}

impl ToBytes for Redirect {
    const OPCODE: u8 = super::Codes::Redirect as _;

    fn write_payload(&self, bytes: &mut Vec<u8>) {
        bytes.extend_from_slice(&self.addr.ip().to_bits().to_le_bytes());
        bytes.extend_from_slice(&self.addr.port().to_be_bytes());
        bytes.push(match self.server_type {
            RedirectServerType::Game => 0x19,
            RedirectServerType::Login => 0x1A,
        });
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
