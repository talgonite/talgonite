use super::constants::{TILE_HEIGHT_HALF, TILE_WIDTH_HALF};
use glam::Vec2;

pub fn get_isometric_coordinate(x: f32, y: f32) -> Vec2 {
    let iso_x = (x * (TILE_WIDTH_HALF as f32)) - (y * TILE_WIDTH_HALF as f32);
    let iso_y = (x * (TILE_HEIGHT_HALF as f32)) + (y * TILE_HEIGHT_HALF as f32);
    Vec2::new(iso_x, iso_y)
}

pub fn screen_to_iso_tile(screen: Vec2, camera_pos: Vec2, window_size: Vec2, zoom: f32) -> Vec2 {
    let offset = Vec2::new(0., TILE_HEIGHT_HALF as f32);
    let centered = (screen - (window_size * 0.5).floor()) / zoom + camera_pos + offset;
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
    (t.x.floor() as i32, t.y.floor() as i32)
}

pub fn tile_to_screen(tile: Vec2, camera_pos: Vec2, window_size: Vec2, zoom: f32) -> Vec2 {
    let iso_coords = get_isometric_coordinate(tile.x, tile.y);
    let offset = Vec2::new(0., TILE_HEIGHT_HALF as f32);
    (iso_coords - camera_pos - offset) * zoom + window_size * 0.5
}

/// Calculate Z depth for a tile at (x, y) with an intra-tile offset.
///
/// The z value is used for depth testing to ensure proper draw order in isometric view.
/// Closer tiles (higher x+y) get higher z values.
///
/// `z_within_tile` should be in range [0, 1] for most cases:
/// - 0.0 = furthest back within the tile (floors, backgrounds)
/// - 0.5 = middle (items, creatures)
/// - 1.0 = closest to camera within the tile (walls, effects)
pub fn calculate_tile_z(x: f32, y: f32, z_within_tile: f32) -> f32 {
    // Current setup: Clear Z to 0.0, use Greater comparison.
    // Higher x+y is closer to camera, so it must have higher Z.
    (x + y + z_within_tile) / 1000.0
}
