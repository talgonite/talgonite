use crate::{ToBytes, TryFromBytes};
use byteorder::{BigEndian, ReadBytesExt};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use std::io::Cursor;

use super::Codes;

#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum ExchangeRequestType {
    StartExchange = 0,
    AddItem = 1,
    AddStackableItem = 2,
    SetGold = 3,
    Cancel = 4,
    Accept = 5,
}

#[derive(Debug, Clone)]
pub enum ExchangeInteractionArgs {
    StartExchange,
    AddItem { source_slot: u8 },
    AddStackableItem { source_slot: u8, item_count: u8 },
    SetGold { gold_amount: i32 },
    Cancel,
    Accept,
}

impl ExchangeInteractionArgs {
    pub fn request_type(&self) -> ExchangeRequestType {
        match self {
            ExchangeInteractionArgs::StartExchange => ExchangeRequestType::StartExchange,
            ExchangeInteractionArgs::AddItem { .. } => ExchangeRequestType::AddItem,
            ExchangeInteractionArgs::AddStackableItem { .. } => {
                ExchangeRequestType::AddStackableItem
            }
            ExchangeInteractionArgs::SetGold { .. } => ExchangeRequestType::SetGold,
            ExchangeInteractionArgs::Cancel => ExchangeRequestType::Cancel,
            ExchangeInteractionArgs::Accept => ExchangeRequestType::Accept,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExchangeInteraction {
    pub other_player_id: u32,
    pub args: ExchangeInteractionArgs,
}

impl ToBytes for ExchangeInteraction {
    const OPCODE: u8 = Codes::ExchangeInteraction as _;

    fn write_payload(&self, bytes: &mut Vec<u8>) {
        bytes.push(self.args.request_type() as u8);
        bytes.extend_from_slice(&self.other_player_id.to_be_bytes());

        match &self.args {
            ExchangeInteractionArgs::AddItem { source_slot } => {
                bytes.push(*source_slot);
            }
            ExchangeInteractionArgs::AddStackableItem {
                source_slot,
                item_count,
            } => {
                bytes.push(*source_slot);
                bytes.push(*item_count);
            }
            ExchangeInteractionArgs::SetGold { gold_amount } => {
                bytes.extend_from_slice(&gold_amount.to_be_bytes());
            }
            _ => { /* No additional data */ }
        }
    }
}

impl TryFromBytes for ExchangeInteraction {
    fn try_from_bytes(bytes: &[u8]) -> anyhow::Result<Self>
    where
        Self: Sized,
    {
        let mut cursor = Cursor::new(bytes);
        let request_type: ExchangeRequestType = cursor.read_u8()?.try_into()?;
        let other_player_id = cursor.read_u32::<BigEndian>()?;

        let args = match request_type {
            ExchangeRequestType::StartExchange => ExchangeInteractionArgs::StartExchange,
            ExchangeRequestType::AddItem => {
                let source_slot = cursor.read_u8()?;
                ExchangeInteractionArgs::AddItem { source_slot }
            }
            ExchangeRequestType::AddStackableItem => {
                let source_slot = cursor.read_u8()?;
                let item_count = cursor.read_u8()?;
                ExchangeInteractionArgs::AddStackableItem {
                    source_slot,
                    item_count,
                }
            }
            ExchangeRequestType::SetGold => {
                let gold_amount = cursor.read_i32::<BigEndian>()?;
                ExchangeInteractionArgs::SetGold { gold_amount }
            }
            ExchangeRequestType::Cancel => ExchangeInteractionArgs::Cancel,
            ExchangeRequestType::Accept => ExchangeInteractionArgs::Accept,
        };

        Ok(ExchangeInteraction {
            other_player_id,
            args,
        })
    }
}
