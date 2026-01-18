use super::Codes;
use crate::ToBytes;
use encoding::all::WINDOWS_949;
use encoding::{EncoderTrap, Encoding};

#[derive(Debug)]
pub struct Login {
    pub user: String,
    pub pass: String,
}

impl ToBytes for Login {
    const OPCODE: u8 = Codes::Login as _;

    fn write_payload(&self, bytes: &mut Vec<u8>) {
        let user_bytes = WINDOWS_949
            .encode(&self.user, EncoderTrap::Replace)
            .unwrap_or_default();
        bytes.push(user_bytes.len() as u8);
        bytes.extend_from_slice(&user_bytes);
        let pass_bytes = WINDOWS_949
            .encode(&self.pass, EncoderTrap::Replace)
            .unwrap_or_default();
        bytes.push(pass_bytes.len() as u8);
        bytes.extend_from_slice(&pass_bytes);

        bytes.extend_from_slice(&[
            31, 82, 135, 160, 197, 234, 232, 183, 126, 125, 110, 79, 73, 170, 1, 0,
        ]);
    }
}
