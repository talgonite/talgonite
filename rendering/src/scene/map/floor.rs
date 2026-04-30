use glam::Vec2;

use crate::scene::{TILE_HEIGHT_HALF, TILE_WIDTH_HALF, get_isometric_coordinate};

#[derive(Clone, Copy)]
pub struct FloorTile {
    pub id: u16,
}

impl FloorTile {
    pub fn show(&self) -> bool {
        self.id > 0
    }

    pub fn tile_id(&self) -> u16 {
        self.id - 1
    }

    pub fn palette_index(&self) -> u16 {
        self.id + 1
    }

    pub fn get_position(x: f32, y: f32) -> Vec2 {
        get_isometric_coordinate(x, y)
            + Vec2::new(-(TILE_WIDTH_HALF as f32), -(TILE_HEIGHT_HALF as f32))
    }
}
