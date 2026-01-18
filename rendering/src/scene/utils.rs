use super::constants::{TILE_HEIGHT_HALF, TILE_WIDTH_HALF};
use glam::Vec2;

pub fn get_isometric_coordinate(x: f32, y: f32) -> Vec2 {
    let iso_x = (x * (TILE_WIDTH_HALF as f32)) - (y * TILE_WIDTH_HALF as f32);
    let iso_y = (x * (TILE_HEIGHT_HALF as f32)) + (y * TILE_HEIGHT_HALF as f32);
    Vec2::new(iso_x, iso_y)
}

pub fn screen_to_iso_tile(screen: Vec2, camera_pos: Vec2, window_size: Vec2, zoom: f32) -> Vec2 {
    let offset = Vec2::new(0., TILE_HEIGHT_HALF as f32);
    let centered = (screen - window_size * 0.5) / zoom + camera_pos + offset;
    let a = centered.x / TILE_WIDTH_HALF as f32;
    let b = centered.y / TILE_HEIGHT_HALF as f32;
    Vec2::new((a + b) * 0.5, (b - a) * 0.5)
}

pub fn screen_to_iso_tile_index(
    screen: Vec2,
    camera_pos: Vec2,
    window_size: Vec2,
    zoom: f32,
) -> (i32, i32) {
    let t = screen_to_iso_tile(screen, camera_pos, window_size, zoom);
    (t.x.round() as i32, t.y.round() as i32)
}

pub fn tile_to_screen(tile: Vec2, camera_pos: Vec2, window_size: Vec2, zoom: f32) -> Vec2 {
    let iso_coords = get_isometric_coordinate(tile.x, tile.y);
    let offset = Vec2::new(0., TILE_HEIGHT_HALF as f32);
    (iso_coords - camera_pos - offset) * zoom + window_size * 0.5
}
