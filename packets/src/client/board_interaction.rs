use crate::ToBytes;
use encoding::all::WINDOWS_949;
use encoding::{EncoderTrap, Encoding};
use num_enum::IntoPrimitive;

use super::Codes;

#[derive(Debug, Clone, Copy, PartialEq, Eq, IntoPrimitive)]
#[repr(u8)]
enum InteractionCode {
    ListBoards = 1,
    ViewBoard = 2,
    ViewPost = 3,
    NewPost = 4,
    Delete = 5,
    SendMail = 6,
    MarkUnread = 7,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, IntoPrimitive)]
#[repr(u8)]
pub enum BoardNavigation {
    Previous = 0xFF,
    Next = 1,
}

#[derive(Debug, Clone)]
pub enum BoardInteraction {
    ListBoards,
    ViewBoard {
        board_id: u16,
        start_post_id: i16,
    },
    ViewPost {
        board_id: u16,
        post_id: i16,
        navigation: Option<BoardNavigation>,
    },
    NewPost {
        board_id: u16,
        subject: String,
        message: String,
    },
    Delete {
        board_id: u16,
        post_id: i16,
    },
    SendMail {
        board_id: u16,
        to: String,
        subject: String,
        message: String,
    },
    MarkUnread {
        board_id: u16,
        post_id: i16,
    },
}

impl ToBytes for BoardInteraction {
    const OPCODE: u8 = Codes::BoardInteraction as _;

    fn write_payload(&self, bytes: &mut Vec<u8>) {
        match &self {
            BoardInteraction::ListBoards => {
                bytes.push(InteractionCode::ListBoards.into());
            }
            BoardInteraction::ViewBoard {
                board_id,
                start_post_id,
            } => {
                bytes.push(InteractionCode::ViewBoard.into());
                bytes.extend_from_slice(&board_id.to_be_bytes());
                bytes.extend_from_slice(&start_post_id.to_be_bytes());
                bytes.push(0xF0); // unknown
            }
            BoardInteraction::ViewPost {
                board_id,
                post_id,
                navigation,
            } => {
                bytes.push(InteractionCode::ViewPost.into());
                bytes.extend_from_slice(&board_id.to_be_bytes());
                bytes.extend_from_slice(&post_id.to_be_bytes());
                if let Some(nav) = navigation {
                    bytes.push((*nav).into());
                } else {
                    bytes.push(0);
                }
            }
            BoardInteraction::NewPost {
                board_id,
                subject,
                message,
            } => {
                bytes.push(InteractionCode::NewPost.into());
                bytes.extend_from_slice(&board_id.to_be_bytes());
                let subject_bytes = WINDOWS_949
                    .encode(subject, EncoderTrap::Replace)
                    .unwrap_or_default();
                bytes.push(subject_bytes.len() as u8);
                bytes.extend_from_slice(&subject_bytes);
                let message_bytes = WINDOWS_949
                    .encode(message, EncoderTrap::Replace)
                    .unwrap_or_default();
                bytes.extend_from_slice(&(message_bytes.len() as u16).to_be_bytes());
                bytes.extend_from_slice(&message_bytes);
            }
            BoardInteraction::Delete { board_id, post_id } => {
                bytes.push(InteractionCode::Delete.into());
                bytes.extend_from_slice(&board_id.to_be_bytes());
                bytes.extend_from_slice(&post_id.to_be_bytes());
            }
            BoardInteraction::SendMail {
                board_id,
                to,
                subject,
                message,
            } => {
                bytes.push(InteractionCode::SendMail.into());
                bytes.extend_from_slice(&board_id.to_be_bytes());
                let to_bytes = WINDOWS_949
                    .encode(to, EncoderTrap::Replace)
                    .unwrap_or_default();
                bytes.push(to_bytes.len() as u8);
                bytes.extend_from_slice(&to_bytes);
                let subject_bytes = WINDOWS_949
                    .encode(subject, EncoderTrap::Replace)
                    .unwrap_or_default();
                bytes.push(subject_bytes.len() as u8);
                bytes.extend_from_slice(&subject_bytes);
                let message_bytes = WINDOWS_949
                    .encode(message, EncoderTrap::Replace)
                    .unwrap_or_default();
                bytes.extend_from_slice(&(message_bytes.len() as u16).to_be_bytes());
                bytes.extend_from_slice(&message_bytes);
            }
            BoardInteraction::MarkUnread { board_id, post_id } => {
                bytes.push(InteractionCode::MarkUnread.into());
                bytes.extend_from_slice(&board_id.to_be_bytes());
                bytes.extend_from_slice(&post_id.to_be_bytes());
            }
        }
    }
}
