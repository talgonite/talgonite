use crate::ToBytes;
use crate::types::EntityType;

use super::Codes;
use encoding::all::WINDOWS_949;
use encoding::{EncoderTrap, Encoding};

#[derive(Debug, Clone)]
pub enum DialogInteractionArgs {
    None,
    MenuResponse { option: u8 },
    TextResponse { args: Vec<String> },
}

#[derive(Debug, Clone)]
pub struct DialogInteraction {
    pub entity_type: EntityType,
    pub entity_id: u32,
    pub pursuit_id: u16,
    pub dialog_id: u16,
    pub args: DialogInteractionArgs,
}

impl ToBytes for DialogInteraction {
    const OPCODE: u8 = Codes::DialogInteraction as _;

    fn write_payload(&self, bytes: &mut Vec<u8>) {
        let mut p = Vec::with_capacity(32);
        p.push(self.entity_type.into());
        p.extend_from_slice(&self.entity_id.to_be_bytes());
        p.extend_from_slice(&self.pursuit_id.to_be_bytes());
        p.extend_from_slice(&self.dialog_id.to_be_bytes());

        match &self.args {
            DialogInteractionArgs::None => {
                p.push(0);
            }
            DialogInteractionArgs::MenuResponse { option } => {
                p.push(1);
                p.push(*option);
            }
            DialogInteractionArgs::TextResponse { args } => {
                p.push(2);
                for arg in args {
                    let encoded = WINDOWS_949
                        .encode(arg, EncoderTrap::Strict)
                        .unwrap_or_default();
                    let len = encoded.len() as u8;
                    p.push(len);
                    p.extend_from_slice(&encoded);
                }
            }
        }
        bytes.extend_from_slice(&crate::dialog_encrypt(&p));
    }
}
