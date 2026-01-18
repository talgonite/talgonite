use crate::TryFromBytes;
use anyhow::anyhow;
use byteorder::{BigEndian, ReadBytesExt};
use std::io::{Cursor, Read};

#[derive(Debug)]
pub struct ForceClientPacket {
    pub client_op_code: u8,
    pub data: Vec<u8>,
}

impl TryFromBytes for ForceClientPacket {
    fn try_from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let mut cursor = Cursor::new(bytes);
        let client_op_code = cursor.read_u8()?;
        let data = {
            let len_raw = cursor.read_u16::<BigEndian>()?;
            let data_len = len_raw
                .checked_sub(1)
                .ok_or_else(|| anyhow!("ForceClientPacket length too small: {}", len_raw))? as usize;
            let mut buf = vec![0; data_len];
            cursor.read_exact(&mut buf)?;
            buf
        };
        Ok(ForceClientPacket {
            client_op_code,
            data,
        })
    }
}
