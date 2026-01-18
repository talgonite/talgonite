// Constants used throughout the scene module

// Tile rendering constants
pub const TILE_WIDTH: u32 = 56;
pub const TILE_WIDTH_HALF: u32 = 28;
pub const TILE_HEIGHT: u32 = 27;
pub const TILE_HEIGHT_HALF: u32 = 14;

// New tile atlas/page configuration
pub const TILEMAP_TILES_PER_ROW: u32 = 128; // 128 tiles wide
pub const TILEMAP_TILES_PER_PAGE_ROWS: u32 = 5; // 5 tiles high per page
pub const TILEMAP_PAGE_WIDTH: u32 = TILEMAP_TILES_PER_ROW * TILE_WIDTH; // 7168
pub const TILEMAP_PAGE_HEIGHT: u32 = TILEMAP_TILES_PER_PAGE_ROWS * TILE_HEIGHT; // 135

// Single large atlas the renderer uses
pub const TILEMAP_WIDTH: u32 = TILEMAP_PAGE_WIDTH; // 7168
pub const TILEMAP_HEIGHT: u32 = 4671; // 173 rows of tiles (173 * 27)

pub const TILEMAP_TILE_WIDTH: f32 = TILE_WIDTH as f32 / TILEMAP_WIDTH as f32;
pub const TILEMAP_TILE_HEIGHT: f32 = TILE_HEIGHT as f32 / TILEMAP_HEIGHT as f32;
pub const TILEMAP_COLUMNS: u32 = TILEMAP_TILES_PER_ROW; // 128
