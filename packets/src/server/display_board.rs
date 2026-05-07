use crate::{TryFromBytes, TryFromCursor};
use anyhow::{Context, anyhow};
use byteorder::{BigEndian, ReadBytesExt};
use encoding::all::WINDOWS_949;
use encoding::{DecoderTrap, Encoding};
use num_enum::TryFromPrimitive;
use std::io::{Cursor, Read};

#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive)]
#[repr(u8)]
enum BoardOrResponseType {
    BoardList = 1,
    PublicBoard = 2,
    PublicPost = 3,
    MailBoard = 4,
    MailPost = 5,
    SubmitPostResponse = 6,
    DeletePostResponse = 7,
    HighlightPostResponse = 8,
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
    pub is_unread: bool,
}

impl TryFromCursor for PostInfo {
    fn try_from_cursor(cursor: &mut Cursor<&[u8]>) -> anyhow::Result<Self> {
        let is_unread = cursor.read_u8().context("reading post unread flag")? != 0;
        let post_id = cursor.read_i16::<BigEndian>().context("reading post id")?;
        let author_len = cursor.read_u8().context("reading post author length")? as usize;
        let mut author_buf = vec![0; author_len];
        cursor
            .read_exact(&mut author_buf)
            .with_context(|| format!("reading post author ({} bytes)", author_len))?;
        let author = WINDOWS_949
            .decode(&author_buf, DecoderTrap::Replace)
            .map_err(|e| anyhow!("Failed to decode post author: {}", e))?;

        let month_of_year = cursor.read_u8().context("reading post month_of_year")?;
        let day_of_month = cursor.read_u8().context("reading post day_of_month")?;

        let subject_len = cursor.read_u8().context("reading post subject length")? as usize;
        let mut subject_buf = vec![0; subject_len];
        cursor
            .read_exact(&mut subject_buf)
            .with_context(|| format!("reading post subject ({} bytes)", subject_len))?;
        let subject = WINDOWS_949
            .decode(&subject_buf, DecoderTrap::Replace)
            .map_err(|e| anyhow!("Failed to decode post subject: {}", e))?;

        Ok(PostInfo {
            post_id,
            author,
            month_of_year,
            day_of_month,
            subject,
            is_unread: is_unread,
        })
    }
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
        message: String,
    },
    MailBoard {
        board: BoardInfo,
    },
    MailPost {
        enable_prev_btn: bool,
        post: PostInfo,
        message: String,
    },
    SubmitResponse {
        success: bool,
        response_message: String,
    },
    DeleteResponse {
        success: bool,
        response_message: String,
    },
    MarkUnreadResponse {
        success: bool,
        response_message: String,
    },
}

impl TryFromBytes for DisplayBoard {
    fn try_from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let mut cursor = Cursor::new(bytes);
        let type_byte = cursor.read_u8().context("reading board packet variant")?;
        let board_type = type_byte
            .try_into()
            .map_err(|_| anyhow!("Invalid board or response type: {}", type_byte))?;

        let decode_string_u8 =
            |cursor: &mut Cursor<&[u8]>, label: &str| -> anyhow::Result<String> {
                let len = cursor
                    .read_u8()
                    .with_context(|| format!("reading {} length", label))?
                    as usize;
                let mut buf = vec![0; len];
                cursor
                    .read_exact(&mut buf)
                    .with_context(|| format!("reading {} ({} bytes)", label, len))?;
                WINDOWS_949
                    .decode(&buf, DecoderTrap::Replace)
                    .map_err(|e| anyhow!("Failed to decode {}: {}", label, e))
            };

        let read_string_u16 = |cursor: &mut Cursor<&[u8]>, label: &str| -> anyhow::Result<String> {
            let len = cursor
                .read_u16::<BigEndian>()
                .with_context(|| format!("reading {} length", label))?
                as usize;
            let mut buf = vec![0; len];
            cursor
                .read_exact(&mut buf)
                .with_context(|| format!("reading {} ({} bytes)", label, len))?;
            WINDOWS_949
                .decode(&buf, DecoderTrap::Replace)
                .map_err(|e| anyhow!("Failed to decode {}: {}", label, e))
        };

        let payload = match board_type {
            BoardOrResponseType::BoardList => {
                let count = cursor
                    .read_u16::<BigEndian>()
                    .context("reading board list count")?;
                let mut boards = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    let board_id = cursor.read_u16::<BigEndian>().context("reading board id")?;
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
                let _ = cursor
                    .read_u8()
                    .context("reading public board unknown flag")?; // unknown flag
                let board_id = cursor
                    .read_u16::<BigEndian>()
                    .context("reading public board id")?;
                let name = decode_string_u8(&mut cursor, "board name")?;
                let posts_count = cursor
                    .read_i8()
                    .context("reading public board post count")?;
                let posts_to_read = posts_count.max(0_i8) as usize;
                let mut posts = Vec::with_capacity(posts_to_read);
                for _ in 0..posts_to_read {
                    posts.push(PostInfo::try_from_cursor(&mut cursor)?);
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
                let enable_prev_btn = cursor
                    .read_u8()
                    .context("reading public post previous button flag")?
                    != 0;
                let post = PostInfo::try_from_cursor(&mut cursor)?;
                let message = read_string_u16(&mut cursor, "post message")?;
                DisplayBoard::PublicPost {
                    enable_prev_btn,
                    post,
                    message,
                }
            }
            BoardOrResponseType::MailBoard => {
                let _ = cursor
                    .read_u8()
                    .context("reading mail board unknown flag")?; // unknown flag
                let board_id = cursor
                    .read_u16::<BigEndian>()
                    .context("reading mail board id")?;
                let name = decode_string_u8(&mut cursor, "board name")?;
                let post_count = cursor.read_i8().context("reading mail board post count")?;
                let posts_to_read = post_count.max(0_i8) as usize;
                let mut posts = Vec::with_capacity(posts_to_read);
                for _ in 0..posts_to_read {
                    posts.push(PostInfo::try_from_cursor(&mut cursor)?);
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
                let enable_prev_btn = cursor
                    .read_u8()
                    .context("reading mail post previous button flag")?
                    != 0;
                cursor.read_u8().context("reading mail post unknown flag")?; // unknown
                let post = PostInfo::try_from_cursor(&mut cursor)?;
                let message = read_string_u16(&mut cursor, "post message")?;
                DisplayBoard::MailPost {
                    enable_prev_btn,
                    post,
                    message,
                }
            }
            BoardOrResponseType::SubmitPostResponse => {
                let success = cursor
                    .read_u8()
                    .context("reading submit post response success flag")?
                    != 0;
                let response_message = decode_string_u8(&mut cursor, "response message")?;
                DisplayBoard::SubmitResponse {
                    success,
                    response_message,
                }
            }
            BoardOrResponseType::DeletePostResponse => {
                let success = cursor
                    .read_u8()
                    .context("reading delete post response success flag")?
                    != 0;
                let response_message = decode_string_u8(&mut cursor, "response message")?;
                DisplayBoard::DeleteResponse {
                    success,
                    response_message,
                }
            }
            BoardOrResponseType::HighlightPostResponse => {
                let success = cursor
                    .read_u8()
                    .context("reading highlight post response success flag")?
                    != 0;
                let response_message = decode_string_u8(&mut cursor, "response message")?;
                DisplayBoard::MarkUnreadResponse {
                    success,
                    response_message,
                }
            }
        };

        Ok(payload)
    }
}
