use crate::TryFromBytes;
use byteorder::ReadBytesExt;
use std::io::Cursor;

#[derive(Debug, Clone)]
pub struct DoorInstance {
    pub x: u8,
    pub y: u8,
    pub closed: bool,
    pub open_right: bool,
}

#[derive(Debug, Clone)]
pub struct Door {
    pub doors: Vec<DoorInstance>,
}

impl TryFromBytes for Door {
    fn try_from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let mut cursor = Cursor::new(bytes);
        let count = cursor.read_u8()?;
        let mut doors = Vec::with_capacity(count as usize);

        for _ in 0..count {
            let x = cursor.read_u8()?;
            let y = cursor.read_u8()?;
            let closed = cursor.read_u8()? == 1;
            let open_right = cursor.read_u8()? == 1;

            doors.push(DoorInstance {
                x,
                y,
                closed,
                open_right,
            });
        }

        Ok(Door { doors })
    }
}
