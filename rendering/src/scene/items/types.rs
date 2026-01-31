use etagere::Allocation;
use formats::epf::EpfImage;

#[derive(Debug, Clone)]
pub struct Item {
    pub id: u32,
    pub x: u16,
    pub y: u16,
    pub sprite: u16,
    pub color: u8,
    /// Network receive order for z-ordering (lower = below)
    pub spawn_order: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ItemInstanceHandle {
    pub(crate) index: usize,
    pub(crate) sprite_id: u16,
}

pub(crate) struct LoadedItemSheet {
    pub epf: EpfImage,
    pub allocations: Vec<Option<Allocation>>,
    pub ref_count: usize,
}
