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

// Z-depth bands within a tile. `calculate_tile_z` places each sprite at
// (x + y + z_within_tile) / 1000, so these partition the intra-tile [0, 1]
// range to guarantee a deterministic draw order between entity types.
// Keep the bands unique and ordered: floor, items, players, creatures, walls, effects.
pub const Z_FLOOR: f32 = 0.0; // Floor tiles sit at the very back of a tile.
pub const Z_ITEMS: f32 = 0.1; // Items occupy [0, Z_ITEMS), sub-sorted by spawn order.
pub const Z_PLAYERS_BASE: f32 = 0.1; // Players start here, stacking by equipment z_priority + tile stack.
pub const Z_CREATURES: f32 = 0.21; // Creatures sit just above the player band.
pub const Z_WALLS: f32 = 0.98; // Walls sit in front of all entities on the tile.
pub const Z_EFFECTS: f32 = 1.0; // Effects render on top of everything on the tile.
