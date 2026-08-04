use crate::scene::map::{
    floor::FloorTile,
    wall::{Wall, WallSide},
};
use byteorder::{LE, ReadBytesExt};
use std::io::Read;

#[derive(Clone, Copy)]
pub struct MapTile {
    pub floor: FloorTile,
    pub wall_left: Wall,
    pub wall_right: Wall,
}

impl MapTile {
    /// Size of one tile in the raw map format (floor + two walls, each a u16).
    pub const BYTES_PER_TILE: usize = 6;

    /// Reads one tile, returning `None` when the map data is truncated.
    pub fn read_from_reader<R: Read>(reader: &mut R) -> Option<Self> {
        let floor = reader.read_u16::<LE>().ok()?;
        let wall_left = reader.read_u16::<LE>().ok()?;
        let wall_right = reader.read_u16::<LE>().ok()?;

        Some(MapTile {
            floor: FloorTile { id: floor },
            wall_left: Wall {
                id: wall_left,
                side: WallSide::Left,
            },
            wall_right: Wall {
                id: wall_right,
                side: WallSide::Right,
            },
        })
    }
}
