use crate::TryFromBytes;
use anyhow::anyhow;
use byteorder::{BigEndian, ReadBytesExt};
use encoding::all::WINDOWS_949;
use encoding::{DecoderTrap, Encoding};
use std::io::{Cursor, Read};

#[derive(Debug)]
pub enum ConnectionInfo {
    TooLow {
        expected_version: u16,
        patch_url: String,
    },
    Ok {
        server_table_checksum: u32,
        seed: u8,
        encryption_key: Vec<u8>,
    },
}

impl TryFromBytes for ConnectionInfo {
    fn try_from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let mut cursor = Cursor::new(bytes);
        Ok(match cursor.read_u8()? {
            1 => ConnectionInfo::TooLow {
                expected_version: cursor.read_u16::<BigEndian>()?,
                patch_url: {
                    let mut buf = vec![0; cursor.read_u8()? as usize];
                    cursor.read_exact(&mut buf)?;
                    WINDOWS_949
                        .decode(&buf, DecoderTrap::Replace)
                        .map_err(|e| anyhow!("Failed to decode patch_url: {}", e))?
                },
            },
            _ => ConnectionInfo::Ok {
                server_table_checksum: cursor.read_u32::<BigEndian>()?,
                seed: cursor.read_u8()?,
                encryption_key: {
                    let mut buf = vec![0; cursor.read_u8()? as usize];
                    cursor.read_exact(&mut buf)?;
                    buf
                },
            },
        })
    }
}
