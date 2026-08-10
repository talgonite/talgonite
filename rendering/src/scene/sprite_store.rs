//! Trait for sprite stores that share one atlas.
//!
//! Each sprite class (players, creatures, items) implements [`SpriteStore`]
//! and owns its own loading, caching, and instance-building rules. The only
//! shared machinery is atlas allocation: a loading store receives its peers
//! so [`allocate_chunks`] can evict unused assets across *every* store when
//! the atlas is full.

use etagere::Allocation;
use formats::game_files::SquashfsArchive;
use formats::sheets::SheetChunk;
use wgpu;

use crate::scene::sprite_atlas::SpriteAtlas;

type Archive = SquashfsArchive;

/// Uniform cache lifecycle shared by every sprite store. Kept object-safe so
/// a store can evict its peers generically.
pub trait SpriteStoreLifecycle {
    /// Class name for diagnostics (e.g. `"players"`).
    fn label(&self) -> &'static str;

    /// Number of cached assets.
    fn cached_count(&self) -> usize;

    /// Evict every cached asset with no live references, returning their
    /// atlas slots to the shared atlas.
    fn evict_unused(&mut self, atlas: &mut SpriteAtlas, queue: &wgpu::Queue);
}

/// A store that loads and caches one sprite class's assets. Loading is
/// class-specific (key type, sheet format, cache layout), so it lives here on
/// the concrete store rather than on the scene or batch.
pub trait SpriteStore: SpriteStoreLifecycle {
    /// What identifies one cached asset (a player part, a creature id, an
    /// item sheet index).
    type Key: Copy + Eq + std::hash::Hash;

    /// The sheet metadata type this store decodes.
    type Sheet: oxicode::Decode;

    /// Loads `key` if it is not cached and takes a reference. Atlas slots are
    /// allocated through [`allocate_chunks`], which evicts `others` (and this
    /// store) when the atlas is full.
    fn ensure_loaded(
        &mut self,
        key: &Self::Key,
        atlas: &mut SpriteAtlas,
        queue: &wgpu::Queue,
        archive: &Archive,
        others: &mut [&mut dyn SpriteStoreLifecycle],
    ) -> anyhow::Result<()>;
}

/// Allocates one atlas slot per chunk, evicting unused assets from `store`
/// and every store in `others` when the atlas is full, then retrying once.
/// Undoes any partial allocations on failure.
pub(crate) fn allocate_chunks(
    atlas: &mut SpriteAtlas,
    queue: &wgpu::Queue,
    store: &mut dyn SpriteStoreLifecycle,
    others: &mut [&mut dyn SpriteStoreLifecycle],
    chunks: &[SheetChunk],
) -> Option<Vec<Allocation>> {
    let mut allocations: Vec<Allocation> = Vec::with_capacity(chunks.len());
    for chunk in chunks {
        let mut allocation = atlas.allocate_slot(chunk.width as usize, chunk.height as usize);
        if allocation.is_none() {
            store.evict_unused(atlas, queue);
            for other in others.iter_mut() {
                other.evict_unused(atlas, queue);
            }
            allocation = atlas.allocate_slot(chunk.width as usize, chunk.height as usize);
        }
        let Some(allocation) = allocation else {
            for allocation in &allocations {
                atlas.deallocate(allocation.id);
            }
            return None;
        };
        allocations.push(allocation);
    }
    Some(allocations)
}
