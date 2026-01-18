use crate::TryFromBytes;
use byteorder::{BigEndian, ReadBytesExt};
use std::io::{Cursor, Read};

#[derive(Debug)]
pub enum LoginNotice {
    FullResponse { data: Vec<u8> },
    CheckSum { check_sum: u32 },
}

impl TryFromBytes for LoginNotice {
    fn try_from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let mut cursor = Cursor::new(bytes);
        let is_full_response = cursor.read_u8()? == 1;

        if is_full_response {
            let data = {
                let mut buf = vec![0; cursor.read_u16::<BigEndian>()? as usize];
                cursor.read_exact(&mut buf)?;
                buf
            };
            Ok(LoginNotice::FullResponse { data })
        } else {
            let check_sum = cursor.read_u32::<BigEndian>()?;
            Ok(LoginNotice::CheckSum { check_sum })
        }
    }
}
