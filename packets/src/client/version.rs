use crate::ToBytes;

use super::Codes;

#[derive(Debug)]
pub struct Version {
    pub version: u16,
}

impl ToBytes for Version {
    const OPCODE: u8 = Codes::Version as _;

    fn write_payload(&self, bytes: &mut Vec<u8>) {
        bytes.extend_from_slice(&self.version.to_be_bytes());
        bytes.extend_from_slice(&[0x4C, 0x4B, 0x00]);
    }
}
