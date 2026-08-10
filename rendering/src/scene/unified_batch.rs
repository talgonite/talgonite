//! One GPU instance batch for the main scene's indexed sprites (players,
//! creatures, items), plus the shared [`SpriteScene`] they load from.
//!
//! [`SpriteScene`] owns the three sprite stores and the shared atlas, so the
//! atlas's eviction policy lives in one place: when an allocation fails it
//! evicts unused sprites across *all* classes before retrying. The batch is
//! just the instance list + handle map; every add/update/remove takes the
//! scene as its one context parameter.

use bevy_math::{Vec2, Vec3};
use formats::epf::{AnimationDirection, EpfAnimationType};
use formats::game_files::SquashfsArchive;
use formats::mpf::{MpfAnimation, MpfAnimationType};

use crate::instance::{Instance, InstanceFlag, SharedInstanceBatch};
use crate::make_quad;
use crate::scene::creatures::{
    AddCreatureResult, CreateInstanceHandle, CreatureAssetStore,
    get_instance_for_frame as get_creature_instance_for_frame,
};
use crate::scene::items::{
    ITEMS_PER_EPF_FILE, Item, ItemAssetStore, ItemInstanceHandle,
    get_instance_for_frame as get_item_instance_for_frame,
};
use crate::scene::players::{
    PlayerAssetStore, PlayerSpriteHandle, PlayerSpriteIndex, PlayerSpriteKey,
};
use crate::scene::sprite::HandleMap;
use crate::scene::sprite_atlas::SpriteAtlas;
use crate::scene::sprite_store::{SpriteStore, SpriteStoreLifecycle};
use crate::scene::utils::direction_to_orientation;

type Archive = SquashfsArchive;

/// Which class and sprite a GPU instance slot belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpriteKey {
    Player(PlayerSpriteKey),
    Creature(u16),
    Item(u16),
}

/// The shared sprite resources: the three stores plus the atlas they all
/// allocate from and upload into.
pub struct SpriteScene {
    pub players: PlayerAssetStore,
    pub creatures: CreatureAssetStore,
    pub items: ItemAssetStore,
    pub atlas: SpriteAtlas,
}

impl SpriteScene {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, archive: &Archive) -> Self {
        Self {
            players: PlayerAssetStore::new(archive),
            creatures: CreatureAssetStore::new(),
            items: ItemAssetStore::new(archive),
            atlas: SpriteAtlas::new(device, queue, archive),
        }
    }

    /// Loads a player sprite if needed, evicting unused assets across every
    /// store when the atlas is full.
    ///
    /// `SpriteScene` is the composition root: it owns the store set, so these
    /// dispatchers are the only place that knows which stores exist and can
    /// hand a loading store its peers.
    pub fn ensure_player(
        &mut self,
        key: &PlayerSpriteKey,
        queue: &wgpu::Queue,
        archive: &Archive,
    ) -> anyhow::Result<()> {
        let SpriteScene {
            players,
            creatures,
            items,
            atlas,
        } = self;
        let mut others: [&mut dyn SpriteStoreLifecycle; 2] = [creatures, items];
        players.ensure_loaded(key, atlas, queue, archive, &mut others)
    }

    /// Loads a creature sprite if needed, evicting across every store when
    /// the atlas is full.
    pub fn ensure_creature(
        &mut self,
        sprite_id: u16,
        queue: &wgpu::Queue,
        archive: &Archive,
    ) -> anyhow::Result<()> {
        let SpriteScene {
            players,
            creatures,
            items,
            atlas,
        } = self;
        let mut others: [&mut dyn SpriteStoreLifecycle; 2] = [players, items];
        creatures.ensure_loaded(&sprite_id, atlas, queue, archive, &mut others)
    }

    /// Loads an item sheet if needed, evicting across every store when the
    /// atlas is full.
    pub fn ensure_item_sheet(
        &mut self,
        sheet_index: u32,
        queue: &wgpu::Queue,
        archive: &Archive,
    ) -> anyhow::Result<()> {
        let SpriteScene {
            players,
            creatures,
            items,
            atlas,
        } = self;
        let mut others: [&mut dyn SpriteStoreLifecycle; 2] = [players, creatures];
        items.ensure_loaded(&sheet_index, atlas, queue, archive, &mut others)
    }

    /// Preloads a batch of player sprites, uploading all of them in one
    /// submit.
    pub fn preload_players(
        &mut self,
        queue: &wgpu::Queue,
        archive: &Archive,
        sprites: &[PlayerSpriteKey],
    ) -> anyhow::Result<()> {
        let SpriteScene {
            players,
            creatures,
            items,
            atlas,
        } = self;
        let mut others: [&mut dyn SpriteStoreLifecycle; 2] = [creatures, items];
        players.preload_player_sprites(queue, archive, atlas, &mut others, sprites)
    }
}

/// Builds the GPU instance for a loaded player sprite (used by both the main
/// scene batch and the portrait batches).
pub(crate) fn build_player_instance(
    scene: &SpriteScene,
    sprite: &PlayerSpriteKey,
    color: u8,
    direction: u8,
    x: f32,
    y: f32,
    entity_id: u32,
    flags: InstanceFlag,
    tint: Vec3,
) -> (Instance, u8) {
    let loaded_sprite = scene
        .players
        .loaded_sprites
        .get(sprite)
        .expect("sprite loaded before building instance");
    let (anim_dir, flip) = direction_to_orientation(direction);
    let is_towards = anim_dir == AnimationDirection::Towards;
    let stack_order = (entity_id % crate::scene::players::PLAYERS_PER_TILE as u32) as u8;
    let instance = PlayerAssetStore::get_instance_for_frame(
        &scene.players.palettes,
        loaded_sprite,
        sprite,
        EpfAnimationType::Idle,
        0,
        Vec2::new(x, y),
        is_towards,
        flip,
        color,
        flags,
        tint,
        stack_order,
        scene.atlas.palette_rows(),
    )
    .unwrap_or_default();
    (instance, stack_order)
}

pub struct UnifiedSpriteBatch {
    instances: SharedInstanceBatch,
    handles: HandleMap<SpriteKey>,
}

impl UnifiedSpriteBatch {
    pub fn new(device: &wgpu::Device, scene: &SpriteScene) -> Self {
        let vertices = make_quad(512, 512).to_vec();
        Self {
            instances: SharedInstanceBatch::new(device, vertices, scene.atlas.bind_group().clone()),
            handles: HandleMap::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.instances.len()
    }

    pub fn is_empty(&self) -> bool {
        self.instances.len() == 0
    }

    pub fn clear(&self) {
        self.instances.clear();
        self.handles.clear();
    }

    /// Clears every instance, releasing each tracked sprite's ref count.
    pub fn clear_and_unload(&self, scene: &mut SpriteScene) {
        for key in self.handles.drain() {
            match key {
                SpriteKey::Player(key) => scene.players.release_sprite(key),
                SpriteKey::Creature(id) => scene.creatures.release_sprite(id),
                SpriteKey::Item(id) => scene.items.release_sheet(id),
            }
        }
        self.instances.clear();
    }

    pub fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        self.instances.draw(render_pass);
    }

    pub fn add_player(
        &self,
        queue: &wgpu::Queue,
        scene: &mut SpriteScene,
        archive: &Archive,
        sprite: PlayerSpriteKey,
        color: u8,
        direction: u8,
        x: f32,
        y: f32,
        entity_id: u32,
        flags: InstanceFlag,
        tint: Vec3,
    ) -> anyhow::Result<PlayerSpriteHandle> {
        scene.ensure_player(&sprite, queue, archive)?;
        let (instance, stack_order) = build_player_instance(
            scene, &sprite, color, direction, x, y, entity_id, flags, tint,
        );
        let instance_index = self
            .instances
            .add(queue, instance)
            .expect("Failed to add instance to batch");
        let handle = PlayerSpriteHandle {
            key: sprite,
            index: PlayerSpriteIndex(instance_index),
            stack_order,
        };
        self.handles
            .insert(handle.index.index(), SpriteKey::Player(sprite));
        Ok(handle)
    }

    pub fn update_player(
        &self,
        queue: &wgpu::Queue,
        scene: &SpriteScene,
        handle: &PlayerSpriteHandle,
        direction: u8,
        x: f32,
        y: f32,
        color: u8,
        flags: InstanceFlag,
        tint: Vec3,
    ) -> anyhow::Result<()> {
        let loaded_sprite = scene
            .players
            .loaded_sprites
            .get(&handle.key)
            .ok_or_else(|| anyhow::anyhow!("Sprite not loaded"))?;
        let (anim_dir, flip) = direction_to_orientation(direction);
        let is_towards = anim_dir == AnimationDirection::Towards;
        let instance = PlayerAssetStore::get_instance_for_frame(
            &scene.players.palettes,
            loaded_sprite,
            &handle.key,
            EpfAnimationType::Idle,
            0,
            Vec2::new(x, y),
            is_towards,
            flip,
            color,
            flags,
            tint,
            handle.stack_order,
            scene.atlas.palette_rows(),
        )?;
        self.instances.update(queue, handle.index.index(), instance);
        Ok(())
    }

    pub fn update_player_with_animation(
        &self,
        queue: &wgpu::Queue,
        scene: &SpriteScene,
        handle: &PlayerSpriteHandle,
        direction: u8,
        x: f32,
        y: f32,
        color: u8,
        animation_type: EpfAnimationType,
        frame_index: usize,
        flags: InstanceFlag,
        tint: Vec3,
    ) -> anyhow::Result<()> {
        let loaded_sprite = scene
            .players
            .loaded_sprites
            .get(&handle.key)
            .ok_or_else(|| anyhow::anyhow!("Sprite not loaded"))?;
        let (anim_dir, flip) = direction_to_orientation(direction);
        let is_towards = anim_dir == AnimationDirection::Towards;
        let instance = PlayerAssetStore::get_instance_for_frame(
            &scene.players.palettes,
            loaded_sprite,
            &handle.key,
            animation_type,
            frame_index,
            Vec2::new(x, y),
            is_towards,
            flip,
            color,
            flags,
            tint,
            handle.stack_order,
            scene.atlas.palette_rows(),
        )
        .unwrap_or_default();
        self.instances.update(queue, handle.index.index(), instance);
        Ok(())
    }

    pub fn hide_player(&self, queue: &wgpu::Queue, handle: &PlayerSpriteHandle) {
        self.instances
            .update(queue, handle.index.index(), Instance::default());
    }

    pub fn remove_player(
        &self,
        queue: &wgpu::Queue,
        scene: &mut SpriteScene,
        handle: PlayerSpriteHandle,
    ) {
        self.instances.remove(queue, handle.index.index());
        self.handles.remove(handle.index.index());
        scene.players.release_sprite(handle.key);
    }

    pub fn supports_animation(
        &self,
        scene: &PlayerAssetStore,
        handle: &PlayerSpriteHandle,
        animation_type: EpfAnimationType,
    ) -> bool {
        scene.supports_animation(handle, animation_type)
    }

    pub fn animation_frame_count(
        &self,
        store: &PlayerAssetStore,
        handle: &PlayerSpriteHandle,
        animation_type: EpfAnimationType,
        is_towards: bool,
    ) -> Option<usize> {
        store.animation_frame_count(handle, animation_type, is_towards)
    }

    pub fn add_creature(
        &self,
        queue: &wgpu::Queue,
        scene: &mut SpriteScene,
        archive: &Archive,
        sprite_id: u16,
        direction: u8,
        x: f32,
        y: f32,
    ) -> anyhow::Result<AddCreatureResult> {
        scene.ensure_creature(sprite_id, queue, archive)?;
        let loaded_sprite = scene
            .creatures
            .loaded_sprites
            .get_mut(&sprite_id)
            .expect("creature sprite loaded above");
        let (anim_dir, flip) = direction_to_orientation(direction);
        let anim = loaded_sprite
            .meta
            .animations
            .iter()
            .find(|a| a.animation_type == MpfAnimationType::Standing)
            .ok_or_else(|| {
                anyhow::anyhow!("No standing animation found for sprite {}", sprite_id)
            })?;
        let frame_index = anim.frame_index_for_direction(anim_dir);
        let instance = get_creature_instance_for_frame(
            loaded_sprite,
            frame_index as usize,
            Vec2::new(x, y),
            flip,
            scene.atlas.palette_rows(),
        )?;

        let instance_index = self
            .instances
            .add(queue, instance)
            .ok_or_else(|| anyhow::anyhow!("Failed to add creature instance"))?;
        let handle = CreateInstanceHandle {
            index: instance_index,
            sprite_id,
        };
        self.handles
            .insert(handle.index, SpriteKey::Creature(sprite_id));
        Ok(AddCreatureResult {
            handle,
            animations: loaded_sprite.meta.animations.clone(),
        })
    }

    pub fn update_creature(
        &self,
        queue: &wgpu::Queue,
        scene: &SpriteScene,
        handle: &CreateInstanceHandle,
        x: f32,
        y: f32,
        anim: &MpfAnimation,
        anim_frame: usize,
        direction: u8,
        tint: Vec3,
    ) -> bool {
        if let Some(loaded_sprite) = scene.creatures.loaded_sprites.get(&handle.sprite_id) {
            let (anim_dir, flip) = direction_to_orientation(direction);
            let frame_index = anim.frame_index_for_direction(anim_dir) as usize + anim_frame;
            if let Ok(mut instance) = get_creature_instance_for_frame(
                loaded_sprite,
                frame_index,
                Vec2::new(x, y),
                flip,
                scene.atlas.palette_rows(),
            ) {
                instance.tint = tint;
                self.instances.update(queue, handle.index, instance);
                return true;
            }
        }
        false
    }

    pub fn remove_creature(
        &self,
        queue: &wgpu::Queue,
        scene: &mut SpriteScene,
        handle: CreateInstanceHandle,
    ) {
        self.instances.remove(queue, handle.index);
        self.handles.remove(handle.index);
        scene.creatures.release_sprite(handle.sprite_id);
    }

    pub fn add_item(
        &self,
        queue: &wgpu::Queue,
        scene: &mut SpriteScene,
        archive: &Archive,
        item: Item,
    ) -> Option<ItemInstanceHandle> {
        let sheet_index = ((item.sprite - 1) as u32 / ITEMS_PER_EPF_FILE) + 1;
        let frame_index = ((item.sprite - 1) as u32 % ITEMS_PER_EPF_FILE) as usize;
        if scene
            .ensure_item_sheet(sheet_index, queue, archive)
            .is_err()
        {
            return None;
        }

        let sheet = scene.items.loaded_sheets.get_mut(&sheet_index)?;
        if frame_index >= sheet.meta.frames.len() {
            scene.items.release_sheet(item.sprite);
            return None;
        }
        let instance = match get_item_instance_for_frame(
            &scene.items.palette_table,
            sheet,
            &item,
            frame_index,
            scene.atlas.palette_rows(),
        ) {
            Some(instance) => instance,
            None => {
                scene.items.release_sheet(item.sprite);
                return None;
            }
        };
        let idx = match self.instances.add(queue, instance) {
            Some(idx) => idx,
            None => {
                scene.items.release_sheet(item.sprite);
                return None;
            }
        };
        let handle = ItemInstanceHandle {
            index: idx,
            sprite_id: item.sprite,
        };
        self.handles
            .insert(handle.index, SpriteKey::Item(item.sprite));
        Some(handle)
    }

    pub fn update_item(
        &self,
        queue: &wgpu::Queue,
        scene: &SpriteScene,
        handle: &ItemInstanceHandle,
        item: Item,
    ) {
        let sheet_index = ((item.sprite - 1) as u32 / ITEMS_PER_EPF_FILE) + 1;
        let frame_index = ((item.sprite - 1) as u32 % ITEMS_PER_EPF_FILE) as usize;
        let Some(sheet) = scene.items.loaded_sheets.get(&sheet_index) else {
            return;
        };
        let Some(instance) = get_item_instance_for_frame(
            &scene.items.palette_table,
            sheet,
            &item,
            frame_index,
            scene.atlas.palette_rows(),
        ) else {
            return;
        };
        self.instances.update(queue, handle.index, instance);
    }

    pub fn remove_item(
        &self,
        queue: &wgpu::Queue,
        scene: &mut SpriteScene,
        handle: ItemInstanceHandle,
    ) {
        self.instances.remove(queue, handle.index);
        self.handles.remove(handle.index);
        scene.items.release_sheet(handle.sprite_id);
    }
}
