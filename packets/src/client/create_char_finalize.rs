use crate::ToBytes;

use super::Codes;

#[derive(Debug)]
pub struct CreateCharFinalize {
    pub hair_style: u8,
    pub gender: u8,
    pub hair_color: u8,
}

impl ToBytes for CreateCharFinalize {
    const OPCODE: u8 = Codes::CreateCharFinalize as _;

    fn write_payload(&self, bytes: &mut Vec<u8>) {
        bytes.push(self.hair_style);
        bytes.push(self.gender);
        bytes.push(self.hair_color);
    }
}
