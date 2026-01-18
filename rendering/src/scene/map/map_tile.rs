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
    pub fn read_from_reader<R: Read>(reader: &mut R) -> Self {
        let floor = reader.read_u16::<LE>().unwrap();
        let wall_left = reader.read_u16::<LE>().unwrap();
        let wall_right = reader.read_u16::<LE>().unwrap();

        MapTile {
            floor: FloorTile { id: floor },
            wall_left: Wall {
                id: wall_left,
                side: WallSide::Left,
            },
            wall_right: Wall {
                id: wall_right,
                side: WallSide::Right,
            },
        }
    }
}
