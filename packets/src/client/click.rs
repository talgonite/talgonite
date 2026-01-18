use crate::ToBytes;

use super::Codes;

#[derive(Debug)]
pub enum Click {
    TargetId(u32),
    TargetPoint((u16, u16)),
}

impl ToBytes for Click {
    const OPCODE: u8 = Codes::Click as _;

    fn write_payload(&self, bytes: &mut Vec<u8>) {
        match self {
            Click::TargetId(id) => {
                bytes.push(1);
                bytes.extend_from_slice(&id.to_be_bytes());
            }
            Click::TargetPoint((x, y)) => {
                bytes.push(3);
                bytes.extend_from_slice(&x.to_be_bytes());
                bytes.extend_from_slice(&y.to_be_bytes());
            }
        }
    }
}
