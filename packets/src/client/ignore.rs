use crate::ToBytes;

use super::Codes;
use encoding::all::WINDOWS_949;
use encoding::{EncoderTrap, Encoding};

#[derive(Debug)]
pub enum Ignore {
    Request,
    AddUser(String),
    RemoveUser(String),
}

impl ToBytes for Ignore {
    const OPCODE: u8 = Codes::Ignore as _;

    fn write_payload(&self, bytes: &mut Vec<u8>) {
        match self {
            Ignore::Request => {
                bytes.push(1);
            }
            Ignore::AddUser(name) => {
                bytes.push(2);
                let name_bytes = WINDOWS_949
                    .encode(name, EncoderTrap::Replace)
                    .unwrap_or_default();
                bytes.push(name_bytes.len() as u8);
                bytes.extend_from_slice(&name_bytes);
            }
            Ignore::RemoveUser(name) => {
                bytes.push(3);
                let name_bytes = WINDOWS_949
                    .encode(name, EncoderTrap::Replace)
                    .unwrap_or_default();
                bytes.push(name_bytes.len() as u8);
                bytes.extend_from_slice(&name_bytes);
            }
        }
    }
}
