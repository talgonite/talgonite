use crate::scene::{TILE_WIDTH_HALF, get_isometric_coordinate};
use glam::Vec2;

#[derive(Clone, Copy)]
pub enum WallSide {
    Left,
    Right,
}

impl WallSide {
    pub fn get_offset(&self) -> f32 {
        match self {
            WallSide::Left => -28.,
            WallSide::Right => 0.,
        }
    }

    pub fn get_position(&self, x: f32, y: f32, height: f32) -> Vec2 {
        let ox = self.get_offset();
        get_isometric_coordinate(x + 1., y) + Vec2::new(ox - (TILE_WIDTH_HALF as f32), -height)
    }
}

#[derive(Clone, Copy)]
pub struct Wall {
    pub id: u16,
    pub side: WallSide,
}

impl Wall {
    pub fn show(&self) -> bool {
        (self.id % 10000) > 2
    }

    pub fn palette_index(&self) -> u16 {
        self.id + 1
    }
}
