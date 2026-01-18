use crate::ToBytes;
use crate::types::EntityType;

use super::Codes;
use encoding::all::WINDOWS_949;
use encoding::{EncoderTrap, Encoding};

#[derive(Debug)]
pub enum MenuInteractionArgs {
    Topics(Vec<String>),
    Slot(u8),
}

#[derive(Debug)]
pub struct MenuInteraction {
    pub entity_type: EntityType,
    pub entity_id: u32,
    pub pursuit_id: u16,
    pub args: MenuInteractionArgs,
}

impl ToBytes for MenuInteraction {
    const OPCODE: u8 = Codes::MenuInteraction as _;

    fn write_payload(&self, bytes: &mut Vec<u8>) {
        let mut p = Vec::with_capacity(32);
        p.push(self.entity_type.into());
        p.extend_from_slice(&self.entity_id.to_be_bytes());
        p.extend_from_slice(&self.pursuit_id.to_be_bytes());
        match &self.args {
            MenuInteractionArgs::Slot(slot) => p.push(*slot),
            MenuInteractionArgs::Topics(topics) => {
                for topic in topics {
                    let topic_bytes = WINDOWS_949
                        .encode(topic, EncoderTrap::Replace)
                        .unwrap_or_default();
                    p.push(topic_bytes.len() as u8);
                    p.extend_from_slice(&topic_bytes);
                }
            }
        }
        bytes.extend_from_slice(&crate::dialog_encrypt(&p));
    }
}
