use crate::{ToBytes, TryFromBytes};
use byteorder::{BigEndian, ReadBytesExt};
use encoding::all::WINDOWS_949;
use encoding::{DecoderTrap, EncoderTrap, Encoding};
use std::io::{Cursor, Read};

use super::Codes;
// (reused above) EncoderTrap/DecoderTrap imported once

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum BoardRequestType {
    BoardList = 1,
    ViewBoard = 2,
    ViewPost = 3,
    NewPost = 4,
    Delete = 5,
    SendMail = 6,
    Highlight = 7,
}

impl BoardRequestType {
    pub fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            1 => Some(BoardRequestType::BoardList),
            2 => Some(BoardRequestType::ViewBoard),
            3 => Some(BoardRequestType::ViewPost),
            4 => Some(BoardRequestType::NewPost),
            5 => Some(BoardRequestType::Delete),
            6 => Some(BoardRequestType::SendMail),
            7 => Some(BoardRequestType::Highlight),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i8)]
pub enum BoardControls {
    None = 0,
    Previous = -1,
    Next = 1,
}

impl BoardControls {
    pub fn from_byte(byte: i8) -> Self {
        match byte {
            -1 => BoardControls::Previous,
            1 => BoardControls::Next,
            _ => BoardControls::None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum BoardInteractionArgs {
    BoardList,
    ViewBoard {
        board_id: u16,
        start_post_id: i16,
    },
    ViewPost {
        board_id: u16,
        post_id: i16,
        controls: BoardControls,
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
    Highlight {
        board_id: u16,
        post_id: i16,
    },
}

#[derive(Debug, Clone)]
pub struct BoardInteraction {
    pub request_type: BoardRequestType,
    pub args: BoardInteractionArgs,
}

impl ToBytes for BoardInteraction {
    const OPCODE: u8 = Codes::BoardInteraction as _;

    fn write_payload(&self, bytes: &mut Vec<u8>) {
        bytes.push(self.request_type as u8);

        match &self.args {
            BoardInteractionArgs::BoardList => {
                // No additional data
            }
            BoardInteractionArgs::ViewBoard {
                board_id,
                start_post_id,
            } => {
                bytes.extend_from_slice(&board_id.to_be_bytes());
                bytes.extend_from_slice(&start_post_id.to_be_bytes());
                bytes.push(240); // Unknown byte from C# code
            }
            BoardInteractionArgs::ViewPost {
                board_id,
                post_id,
                controls,
            } => {
                bytes.extend_from_slice(&board_id.to_be_bytes());
                bytes.extend_from_slice(&post_id.to_be_bytes());
                bytes.push(*controls as i8 as u8);
            }
            BoardInteractionArgs::NewPost {
                board_id,
                subject,
                message,
            } => {
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
            BoardInteractionArgs::Delete { board_id, post_id } => {
                bytes.extend_from_slice(&board_id.to_be_bytes());
                bytes.extend_from_slice(&post_id.to_be_bytes());
            }
            BoardInteractionArgs::SendMail {
                board_id,
                to,
                subject,
                message,
            } => {
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
            BoardInteractionArgs::Highlight { board_id, post_id } => {
                bytes.extend_from_slice(&board_id.to_be_bytes());
                bytes.extend_from_slice(&post_id.to_be_bytes());
            }
        }
    }
}

impl TryFromBytes for BoardInteraction {
    fn try_from_bytes(bytes: &[u8]) -> anyhow::Result<Self>
    where
        Self: Sized,
    {
        let mut cursor = Cursor::new(bytes);
        let board_request_type_byte = cursor.read_u8()?;
        let request_type = BoardRequestType::from_byte(board_request_type_byte)
            .expect("Invalid board request type");

        let args = match request_type {
            BoardRequestType::BoardList => BoardInteractionArgs::BoardList,
            BoardRequestType::ViewBoard => {
                let board_id = cursor.read_u16::<BigEndian>()?;
                let start_post_id = cursor.read_i16::<BigEndian>()?;
                // Skip the unknown byte (240)
                let _ = cursor.read_u8();
                BoardInteractionArgs::ViewBoard {
                    board_id,
                    start_post_id,
                }
            }
            BoardRequestType::ViewPost => {
                let board_id = cursor.read_u16::<BigEndian>()?;
                let post_id = cursor.read_i16::<BigEndian>()?;
                let controls_byte = cursor.read_i8()?;
                let controls = BoardControls::from_byte(controls_byte);
                BoardInteractionArgs::ViewPost {
                    board_id,
                    post_id,
                    controls,
                }
            }
            BoardRequestType::NewPost => {
                let board_id = cursor.read_u16::<BigEndian>()?;
                let subject_len = cursor.read_u8()?;
                let mut subject_buf = vec![0; subject_len as usize];
                cursor.read_exact(&mut subject_buf)?;
                let subject = WINDOWS_949
                    .decode(&subject_buf, DecoderTrap::Replace)
                    .unwrap_or_default();

                let message_len = cursor.read_u16::<BigEndian>()?;
                let mut message_buf = vec![0; message_len as usize];
                cursor.read_exact(&mut message_buf)?;
                let message = WINDOWS_949
                    .decode(&message_buf, DecoderTrap::Replace)
                    .unwrap_or_default();

                BoardInteractionArgs::NewPost {
                    board_id,
                    subject,
                    message,
                }
            }
            BoardRequestType::Delete => {
                let board_id = cursor.read_u16::<BigEndian>()?;
                let post_id = cursor.read_i16::<BigEndian>()?;
                BoardInteractionArgs::Delete { board_id, post_id }
            }
            BoardRequestType::SendMail => {
                let board_id = cursor.read_u16::<BigEndian>()?;

                let to_len = cursor.read_u8()?;
                let mut to_buf = vec![0; to_len as usize];
                cursor.read_exact(&mut to_buf)?;
                let to = WINDOWS_949
                    .decode(&to_buf, DecoderTrap::Replace)
                    .unwrap_or_default();

                let subject_len = cursor.read_u8()?;
                let mut subject_buf = vec![0; subject_len as usize];
                cursor.read_exact(&mut subject_buf)?;
                let subject = WINDOWS_949
                    .decode(&subject_buf, DecoderTrap::Replace)
                    .unwrap_or_default();

                let message_len = cursor.read_u16::<BigEndian>()?;
                let mut message_buf = vec![0; message_len as usize];
                cursor.read_exact(&mut message_buf)?;
                let message = WINDOWS_949
                    .decode(&message_buf, DecoderTrap::Replace)
                    .unwrap_or_default();

                BoardInteractionArgs::SendMail {
                    board_id,
                    to,
                    subject,
                    message,
                }
            }
            BoardRequestType::Highlight => {
                let board_id = cursor.read_u16::<BigEndian>()?;
                let post_id = cursor.read_i16::<BigEndian>()?;
                BoardInteractionArgs::Highlight { board_id, post_id }
            }
        };

        Ok(BoardInteraction { request_type, args })
    }
}
