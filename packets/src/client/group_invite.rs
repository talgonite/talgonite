use crate::ToBytes;

use super::Codes;
use encoding::all::WINDOWS_949;
use encoding::{EncoderTrap, Encoding};

#[derive(Debug)]
pub enum GroupInvite {
    CreateGroupBox {
        target_name: String,
        name: String,
        note: String,
        min_level: u8,
        max_level: u8,
        max_warriors: u8,
        max_wizards: u8,
        max_rogues: u8,
        max_priests: u8,
        max_monks: u8,
    },
    Request {
        name: String,
    },
    Forced {
        name: String,
    },
}

impl ToBytes for GroupInvite {
    const OPCODE: u8 = Codes::GroupInvite as _;

    fn write_payload(&self, bytes: &mut Vec<u8>) {
        match &self {
            GroupInvite::CreateGroupBox {
                target_name,
                name,
                note,
                min_level,
                max_level,
                max_warriors,
                max_wizards,
                max_rogues,
                max_priests,
                max_monks,
            } => {
                bytes.push(1);

                let target_name_bytes = WINDOWS_949
                    .encode(target_name, EncoderTrap::Replace)
                    .unwrap_or_default();
                bytes.push(target_name_bytes.len() as u8);
                bytes.extend_from_slice(&target_name_bytes);
                let name_bytes = WINDOWS_949
                    .encode(name, EncoderTrap::Replace)
                    .unwrap_or_default();
                bytes.push(name_bytes.len() as u8);
                bytes.extend_from_slice(&name_bytes);

                let note_bytes = WINDOWS_949
                    .encode(note, EncoderTrap::Replace)
                    .unwrap_or_default();
                bytes.push(note_bytes.len() as u8);
                bytes.extend_from_slice(&note_bytes);
                bytes.push(*min_level);
                bytes.push(*max_level);
                bytes.push(*max_warriors);
                bytes.push(*max_wizards);
                bytes.push(*max_rogues);
                bytes.push(*max_priests);
                bytes.push(*max_monks);
            }
            GroupInvite::Request { name } => {
                bytes.push(2);
                let name_bytes = WINDOWS_949
                    .encode(name, EncoderTrap::Replace)
                    .unwrap_or_default();
                bytes.push(name_bytes.len() as u8);
                bytes.extend_from_slice(&name_bytes);
            }
            GroupInvite::Forced { name } => {
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
