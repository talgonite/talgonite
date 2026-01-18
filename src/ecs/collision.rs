use bevy::prelude::*;
use rendering::scene::map::map_tile::MapTile;
use rendering::scene::map::door_data;
use std::io::Cursor;
use std::collections::HashMap;

#[derive(Resource)]
pub struct WallCollisionTable {
    data: Vec<u8>,
}

impl WallCollisionTable {
    pub fn from_sotp_bytes(bytes: Vec<u8>) -> Self {
        Self { data: bytes }
    }

    pub fn is_blocking(&self, wall_id: u16) -> bool {
        if wall_id == 0 {
            return false;
        }
        let index = (wall_id - 1) as usize;
        self.data.get(index).map_or(false, |&byte| byte == 0x0F)
    }
}

#[derive(Resource)]
pub struct MapCollisionData {
    walls: Vec<(u16, u16)>,
    width: u8,
    height: u8,
}

impl MapCollisionData {
    pub fn from_map_bytes(map_bytes: &[u8], width: u8, height: u8) -> Self {
        let mut cursor = Cursor::new(map_bytes);
        let mut walls = Vec::with_capacity((width as usize) * (height as usize));

        for _y in 0..height {
            for _x in 0..width {
                let tile = MapTile::read_from_reader(&mut cursor);
                walls.push((tile.wall_left.id, tile.wall_right.id));
            }
        }

        Self {
            walls,
            width,
            height,
        }
    }

    pub fn get_walls_at(&self, x: u8, y: u8) -> Option<(u16, u16)> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let index = (y as usize) * (self.width as usize) + (x as usize);
        self.walls.get(index).copied()
    }

    pub fn set_door(&mut self, x: u8, y: u8, closed: bool) {
        let Some((wall_left, wall_right)) = self.get_walls_at(x, y) else {
            return;
        };

        let door_pairs = door_data::get_door_tile_toggle_pairs();
        let mut closed_to_open: HashMap<u16, u16> = HashMap::new();
        let mut open_to_closed: HashMap<u16, u16> = HashMap::new();

        for pair in &door_pairs {
            closed_to_open.insert(pair.closed_tile, pair.open_tile);
            open_to_closed.insert(pair.open_tile, pair.closed_tile);
        }

        let new_left = if closed {
            open_to_closed.get(&wall_left).copied().unwrap_or(wall_left)
        } else {
            closed_to_open.get(&wall_left).copied().unwrap_or(wall_left)
        };

        let new_right = if closed {
            open_to_closed.get(&wall_right).copied().unwrap_or(wall_right)
        } else {
            closed_to_open.get(&wall_right).copied().unwrap_or(wall_right)
        };

        self.set_walls_at(x, y, new_left, new_right);
    }

    fn set_walls_at(&mut self, x: u8, y: u8, wall_left: u16, wall_right: u16) {
        if x >= self.width || y >= self.height {
            return;
        }
        let index = (y as usize) * (self.width as usize) + (x as usize);
        if let Some(walls) = self.walls.get_mut(index) {
            *walls = (wall_left, wall_right);
        }
    }
}

pub fn can_walk_to(
    target_x: u8,
    target_y: u8,
    collision_table: Option<&WallCollisionTable>,
    map_collision: Option<&MapCollisionData>,
) -> bool {
    if let (Some(collision_table), Some(map_collision)) = (collision_table, map_collision) {
        if let Some((wall_left, wall_right)) = map_collision.get_walls_at(target_x, target_y) {
            if collision_table.is_blocking(wall_left) || collision_table.is_blocking(wall_right) {
                return false;
            }
        }
    }
    true
}
