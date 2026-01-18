use crate::ToBytes;
use encoding::all::WINDOWS_949;
use encoding::{EncoderTrap, Encoding};

use super::Codes;

#[derive(Debug)]
pub struct SpellUse {
    pub source_slot: u8,
    pub args: SpellUseArgs,
}

#[derive(Debug, Default)]
pub enum SpellUseArgs {
    #[default]
    None,
    PromptResponse {
        response: String,
    },
    Targeted {
        target_id: u32,
        target_x: u16,
        target_y: u16,
    },
    SelfTargeted {
        source_id: u32,
    },
}

impl ToBytes for SpellUse {
    const OPCODE: u8 = Codes::SpellUse as _;

    fn write_payload(&self, bytes: &mut Vec<u8>) {
        bytes.push(self.source_slot);

        match &self.args {
            SpellUseArgs::None => {}
            SpellUseArgs::PromptResponse { response } => {
                let response_bytes = WINDOWS_949
                    .encode(response, EncoderTrap::Replace)
                    .unwrap_or_default();
                bytes.push(response_bytes.len() as u8);
                bytes.extend_from_slice(&response_bytes);
            }
            SpellUseArgs::Targeted {
                target_id,
                target_x,
                target_y,
            } => {
                bytes.extend_from_slice(&target_id.to_be_bytes());
                bytes.extend_from_slice(&target_x.to_be_bytes());
                bytes.extend_from_slice(&target_y.to_be_bytes());
            }
            SpellUseArgs::SelfTargeted { source_id } => {
                bytes.extend_from_slice(&source_id.to_be_bytes());
            }
        }
    }
}
