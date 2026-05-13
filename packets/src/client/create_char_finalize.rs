use crate::ToBytes;
use num_enum::{IntoPrimitive, TryFromPrimitive};

use super::Codes;

#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum CharGender {
    Male = 1,
    Female = 2,
}

#[derive(Debug)]
pub struct CreateCharFinalize {
    pub hair_style: u8,
    pub gender: CharGender,
    pub hair_color: u8,
}

impl ToBytes for CreateCharFinalize {
    const OPCODE: u8 = Codes::CreateCharFinalize as _;

    fn write_payload(&self, bytes: &mut Vec<u8>) {
        bytes.push(self.hair_style);
        bytes.push(self.gender.into());
        bytes.push(self.hair_color);
    }
}
