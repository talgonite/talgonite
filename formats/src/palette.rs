/// A single palette index range that is rotated to create animated color
/// effects such as water shimmer or fountain spray.
#[derive(Debug, Clone, Copy, PartialEq, Eq, oxicode::Encode, oxicode::Decode)]
pub struct AnimatedPaletteRange {
    /// First palette index in the animated range (0-based).
    pub start_index: u8,
    /// Last palette index in the animated range (0-based, inclusive).
    pub end_index: u8,
    /// Number of 100ms intervals between each rotation step.
    pub period: u16,
}

/// Animated palette definitions keyed by palette row. Rows correspond to the
/// palette texture rows used by the map renderer, so a row is only animated
/// when it is actually referenced by a tile on the current map.
#[derive(Debug, Clone, Default, PartialEq, Eq, oxicode::Encode, oxicode::Decode)]
pub struct AnimatedPaletteTable {
    pub entries: Vec<(u16, Vec<AnimatedPaletteRange>)>,
}
