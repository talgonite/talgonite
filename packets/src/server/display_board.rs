use crate::TryFromBytes;
use anyhow::anyhow;
use byteorder::{BigEndian, ReadBytesExt};
use encoding::all::WINDOWS_949;
use encoding::{DecoderTrap, Encoding};
use num_enum::TryFromPrimitive;
use std::io::{Cursor, Read};

#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive)]
#[repr(u8)]
pub enum BoardOrResponseType {
    BoardList = 0,
    PublicBoard = 1,
    PublicPost = 2,
    MailBoard = 3,
    MailPost = 4,
    SubmitPostResponse = 5,
    DeletePostResponse = 6,
    HighlightPostResponse = 7,
}

#[derive(Debug, Clone)]
pub struct BoardInfo {
    pub board_id: u16,
    pub name: String,
    pub posts: Vec<PostInfo>,
}

#[derive(Debug, Clone)]
pub struct PostInfo {
    pub post_id: i16,
    pub author: String,
    pub month_of_year: u8,
    pub day_of_month: u8,
    pub subject: String,
    pub message: String,
    pub is_highlighted: bool,
}

#[derive(Debug, Clone)]
pub enum DisplayBoard {
    BoardList {
        boards: Vec<BoardInfo>,
    },
    PublicBoard {
        board: BoardInfo,
    },
    PublicPost {
        enable_prev_btn: bool,
        post: PostInfo,
    },
    MailBoard {
        board: BoardInfo,
    },
    MailPost {
        enable_prev_btn: bool,
        post: PostInfo,
    },
    SubmitPostResponse {
        success: bool,
        response_message: String,
    },
    DeletePostResponse {
        success: bool,
        response_message: String,
    },
    HighlightPostResponse {
        success: bool,
        response_message: String,
    },
}

impl TryFromBytes for DisplayBoard {
    fn try_from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let mut cursor = Cursor::new(bytes);
        let type_byte = cursor.read_u8()?;
        let board_type = type_byte
            .try_into()
            .map_err(|_| anyhow!("Invalid board or response type: {}", type_byte))?;

        let decode_string_u8 =
            |cursor: &mut Cursor<&[u8]>, label: &str| -> anyhow::Result<String> {
                let len = cursor.read_u8()? as usize;
                let mut buf = vec![0; len];
                cursor.read_exact(&mut buf)?;
                WINDOWS_949
                    .decode(&buf, DecoderTrap::Replace)
                    .map_err(|e| anyhow!("Failed to decode {}: {}", label, e))
            };

        let decode_string_u16 =
            |cursor: &mut Cursor<&[u8]>, label: &str| -> anyhow::Result<String> {
                let len = cursor.read_u16::<BigEndian>()? as usize;
                let mut buf = vec![0; len];
                cursor.read_exact(&mut buf)?;
                WINDOWS_949
                    .decode(&buf, DecoderTrap::Replace)
                    .map_err(|e| anyhow!("Failed to decode {}: {}", label, e))
            };

        let read_post =
            |cursor: &mut Cursor<&[u8]>, has_message: bool| -> anyhow::Result<PostInfo> {
                let is_highlighted = cursor.read_u8()? != 0;
                let post_id = cursor.read_i16::<BigEndian>()?;
                let author = decode_string_u8(cursor, "post author")?;
                let month_of_year = cursor.read_u8()?;
                let day_of_month = cursor.read_u8()?;
                let subject = decode_string_u8(cursor, "post subject")?;
                let message = if has_message {
                    decode_string_u16(cursor, "post message")?
                } else {
                    String::new()
                };
                Ok(PostInfo {
                    post_id,
                    author,
                    month_of_year,
                    day_of_month,
                    subject,
                    message,
                    is_highlighted,
                })
            };

        let payload = match board_type {
            BoardOrResponseType::BoardList => {
                let count = cursor.read_u16::<BigEndian>()?;
                let mut boards = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    let board_id = cursor.read_u16::<BigEndian>()?;
                    let name = decode_string_u8(&mut cursor, "board name")?;
                    boards.push(BoardInfo {
                        board_id,
                        name,
                        posts: Vec::new(),
                    });
                }
                DisplayBoard::BoardList { boards }
            }
            BoardOrResponseType::PublicBoard => {
                let _ = cursor.read_u8()?; // unknown flag
                let board_id = cursor.read_u16::<BigEndian>()?;
                let name = decode_string_u8(&mut cursor, "board name")?;
                let posts_count = cursor.read_i8()?;
                let posts_to_read = posts_count.max(0_i8) as usize;
                let mut posts = Vec::with_capacity(posts_to_read);
                for _ in 0..posts_to_read {
                    posts.push(read_post(&mut cursor, false)?);
                }
                DisplayBoard::PublicBoard {
                    board: BoardInfo {
                        board_id,
                        name,
                        posts,
                    },
                }
            }
            BoardOrResponseType::PublicPost => {
                let enable_prev_btn = cursor.read_u8()? != 0;
                cursor.read_u8()?; // unknown
                let post = read_post(&mut cursor, true)?;
                DisplayBoard::PublicPost {
                    enable_prev_btn,
                    post,
                }
            }
            BoardOrResponseType::MailBoard => {
                let _ = cursor.read_u8()?; // unknown flag
                let board_id = cursor.read_u16::<BigEndian>()?;
                let name = decode_string_u8(&mut cursor, "board name")?;
                let post_count = cursor.read_i8()?;
                let posts_to_read = post_count.max(0_i8) as usize;
                let mut posts = Vec::with_capacity(posts_to_read);
                for _ in 0..posts_to_read {
                    posts.push(read_post(&mut cursor, false)?);
                }
                DisplayBoard::MailBoard {
                    board: BoardInfo {
                        board_id,
                        name,
                        posts,
                    },
                }
            }
            BoardOrResponseType::MailPost => {
                let enable_prev_btn = cursor.read_u8()? != 0;
                cursor.read_u8()?; // unknown
                let post = read_post(&mut cursor, true)?;
                DisplayBoard::MailPost {
                    enable_prev_btn,
                    post,
                }
            }
            BoardOrResponseType::SubmitPostResponse => {
                let success = cursor.read_u8()? != 0;
                let response_message = decode_string_u8(&mut cursor, "response message")?;
                DisplayBoard::SubmitPostResponse {
                    success,
                    response_message,
                }
            }
            BoardOrResponseType::DeletePostResponse => {
                let success = cursor.read_u8()? != 0;
                let response_message = decode_string_u8(&mut cursor, "response message")?;
                DisplayBoard::DeletePostResponse {
                    success,
                    response_message,
                }
            }
            BoardOrResponseType::HighlightPostResponse => {
                let success = cursor.read_u8()? != 0;
                let response_message = decode_string_u8(&mut cursor, "response message")?;
                DisplayBoard::HighlightPostResponse {
                    success,
                    response_message,
                }
            }
        };

        Ok(payload)
    }
}
