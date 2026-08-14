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
        let len_raw = cursor.read_u16::<BigEndian>()?;
        let client_op_code = cursor.read_u8()?;
        let data_len = len_raw
            .checked_sub(1)
            .ok_or_else(|| anyhow!("ForceClientPacket length too small: {}", len_raw))?
            as usize;
        let mut data = vec![0; data_len];
        cursor.read_exact(&mut data)?;
        Ok(ForceClientPacket {
            client_op_code,
            data,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_force_client_packet_parse() {
        // [u16 BE length][u8 client opcode][data]: length = data.len + 1
        let bytes = [0x00, 0x06, 0x4A, 0x00, 0x00, 0x00, 0x30, 0x39];
        let pkt = ForceClientPacket::try_from_bytes(&bytes).unwrap();
        assert_eq!(pkt.client_op_code, 0x4A);
        assert_eq!(pkt.data, vec![0x00, 0x00, 0x00, 0x30, 0x39]);
    }
}
