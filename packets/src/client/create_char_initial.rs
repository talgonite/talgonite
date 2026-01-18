use crate::ToBytes;

use super::Codes;
use encoding::all::WINDOWS_949;
use encoding::{EncoderTrap, Encoding};

#[derive(Debug)]
pub struct CreateCharInitial {
    pub name: String,
    pub password: String,
}

impl ToBytes for CreateCharInitial {
    const OPCODE: u8 = Codes::CreateCharInitial as _;

    fn write_payload(&self, bytes: &mut Vec<u8>) {
        let name_bytes = WINDOWS_949
            .encode(&self.name, EncoderTrap::Replace)
            .unwrap_or_default();
        bytes.push(name_bytes.len() as u8);
        bytes.extend_from_slice(&name_bytes);

        let password_bytes = WINDOWS_949
            .encode(&self.password, EncoderTrap::Replace)
            .unwrap_or_default();
        bytes.push(password_bytes.len() as u8);
        bytes.extend_from_slice(&password_bytes);
    }
}
