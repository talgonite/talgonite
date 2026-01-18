use crate::ToBytes;

use super::Codes;
use encoding::all::WINDOWS_949;
use encoding::{EncoderTrap, Encoding};

#[derive(Debug)]
pub struct PasswordChange {
    pub name: String,
    pub current_password: String,
    pub new_password: String,
}

impl ToBytes for PasswordChange {
    const OPCODE: u8 = Codes::PasswordChange as _;

    fn write_payload(&self, bytes: &mut Vec<u8>) {
        let name_bytes = WINDOWS_949
            .encode(&self.name, EncoderTrap::Replace)
            .unwrap_or_default();
        bytes.extend_from_slice(&(name_bytes.len() as u8).to_be_bytes());
        bytes.extend_from_slice(&name_bytes);

        let current_password_bytes = WINDOWS_949
            .encode(&self.current_password, EncoderTrap::Replace)
            .unwrap_or_default();
        bytes.extend_from_slice(&(current_password_bytes.len() as u8).to_be_bytes());
        bytes.extend_from_slice(&current_password_bytes);

        let new_password_bytes = WINDOWS_949
            .encode(&self.new_password, EncoderTrap::Replace)
            .unwrap_or_default();
        bytes.extend_from_slice(&(new_password_bytes.len() as u8).to_be_bytes());
        bytes.extend_from_slice(&new_password_bytes);
    }
}
