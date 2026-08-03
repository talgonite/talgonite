//! Generic sprite batching shared by the item, creature, and player renderers.
//!
//! Each entity type keeps its own store (asset loading, animation, palette) and
//! its own batch API (`add`/`update`/`remove` with entity-specific arguments),
//! but they all share the same GPU machinery: a [`SharedInstanceBatch`] plus a
//! handle map that tracks which GPU instance index belongs to which sprite key.
//!
//! [`SpriteBatch`] provides that shared machinery and the common lifecycle
//! (`clear`, `clear_and_unload`, `remove`), so entity batches only need to
//! implement their own load/animate/instance-build logic on top.

use std::sync::Mutex;

use rustc_hash::FxHashMap;

use crate::instance::SharedInstanceBatch;
use crate::scene::Instance;
use crate::vertex::Vertex;

/// Thread-safe map from GPU instance index to sprite key.
pub struct HandleMap<K> {
    map: Mutex<FxHashMap<usize, K>>,
}

impl<K> HandleMap<K> {
    pub fn new() -> Self {
        Self {
            map: Mutex::new(FxHashMap::default()),
        }
    }

    pub fn insert(&self, index: usize, key: K) {
        self.map.lock().unwrap().insert(index, key);
    }

    pub fn remove(&self, index: usize) -> Option<K> {
        self.map.lock().unwrap().remove(&index)
    }

    pub fn get(&self, index: usize) -> Option<K>
    where
        K: Copy,
    {
        self.map.lock().unwrap().get(&index).copied()
    }

    /// Remove and return every tracked key.
    pub fn drain(&self) -> Vec<K> {
        let mut map = self.map.lock().unwrap();
        map.drain().map(|(_, key)| key).collect()
    }

    pub fn clear(&self) {
        self.map.lock().unwrap().clear();
    }
}

impl<K> Default for HandleMap<K> {
    fn default() -> Self {
        Self::new()
    }
}

/// Generic GPU sprite batch: a [`SharedInstanceBatch`] plus a [`HandleMap`].
///
/// Entity batches (players/items/creatures) own one of these and add their
/// entity-specific logic on top.
pub struct SpriteBatch<K> {
    instances: SharedInstanceBatch,
    handles: HandleMap<K>,
}

impl<K> SpriteBatch<K> {
    pub fn new(device: &wgpu::Device, vertices: Vec<Vertex>, bind_group: wgpu::BindGroup) -> Self {
        Self {
            instances: SharedInstanceBatch::new(device, vertices, bind_group),
            handles: HandleMap::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.instances.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn clear(&self) {
        self.instances.clear();
        self.handles.clear();
    }

    /// Clear every instance, calling `unload` for each tracked sprite key so the
    /// store can release its ref-counted assets.
    pub fn clear_and_unload(&self, mut unload: impl FnMut(&K)) {
        for key in self.handles.drain() {
            unload(&key);
        }
        self.instances.clear();
    }

    /// Allocate a new instance slot and write the instance to the GPU.
    pub fn add_instance(&self, queue: &wgpu::Queue, instance: Instance) -> Option<usize> {
        self.instances.add(queue, instance)
    }

    pub fn update_instance(&self, queue: &wgpu::Queue, index: usize, instance: Instance) {
        self.instances.update(queue, index, instance);
    }

    /// Remove the instance (and its handle map entry), returning the tracked
    /// sprite key so the caller can release its asset reference.
    pub fn remove_instance(&self, queue: &wgpu::Queue, index: usize) -> Option<K> {
        self.instances.remove(queue, index);
        self.handles.remove(index)
    }

    pub fn insert_handle(&self, index: usize, key: K) {
        self.handles.insert(index, key);
    }

    pub fn handle(&self, index: usize) -> Option<K>
    where
        K: Copy,
    {
        self.handles.get(index)
    }

    pub fn draw<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        self.instances.draw(render_pass);
    }
}
