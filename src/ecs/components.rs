use bevy::ecs::lifecycle::HookContext;
use bevy::ecs::world::DeferredWorld;
use bevy::prelude::*;
use bevy::tasks::Task;
use rendering::scene::items::ItemInstanceHandle;
use rendering::scene::map::renderer::PreparedMap;
use rendering::scene::{
    EffectHandle,
    creatures::AddCreatureResult,
    players::{PlayerPieceType, PlayerSpriteHandle},
};

#[derive(Component, PartialEq)]
pub struct Position {
    pub x: f32,
    pub y: f32,
}

// Tween (interpolated) movement between two tile positions.
// Added to entities (creatures & players) on walk; lasts 500ms.
#[derive(Component, Debug)]
pub struct MovementTween {
    pub start_x: f32,
    pub start_y: f32,
    pub end_x: f32,
    pub end_y: f32,
    pub elapsed: f32,
    pub duration: f32, // seconds
}

#[derive(Debug, Clone)]
pub enum PathTarget {
    Tile { x: u8, y: u8 },
}

#[derive(Component, Debug)]
pub struct PathfindingState {
    pub target: PathTarget,
    pub retry_timer: Option<Timer>,
}

#[repr(u8)]
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Direction {
    Up = 0,
    Right = 1,
    Down = 2,
    Left = 3,
}

impl From<u8> for Direction {
    fn from(value: u8) -> Self {
        match value {
            0 => Direction::Up,
            1 => Direction::Right,
            2 => Direction::Down,
            3 => Direction::Left,
            _ => Direction::Down,
        }
    }
}

#[derive(Component)]
pub struct LocalPlayer;

#[derive(Component)]
pub struct NPC {
    pub name: String,
    pub entity_type: packets::server::VisibleEntityType,
}

#[derive(Component)]
pub struct Player {
    pub name: String,
    pub is_male: bool,
}

#[derive(Bundle)]
pub struct ItemBundle {
    pub entity_id: EntityId,
    pub position: Position,
    pub sprite: ItemSprite,
}

#[derive(Component, Debug)]
pub struct ItemSprite {
    pub id: u16,
    pub color: u8,
}

#[derive(Component)]
#[component(on_remove = cleanup_item_instance)]
pub struct ItemInstance {
    pub handle: ItemInstanceHandle,
}

#[derive(Component, Debug)]
pub struct PlayerSprite {
    pub id: u16,
    pub slot: PlayerPieceType,
    pub color: u8,
}

#[derive(Component)]
#[component(on_remove = cleanup_player_sprite_instance)]
pub struct PlayerSpriteInstance {
    pub handle: PlayerSpriteHandle,
}

#[derive(Bundle)]
pub struct PlayerBundle {
    pub player: Player,
    pub position: Position,
    pub direction: Direction,
    pub entity_id: EntityId,
}

#[derive(Bundle)]
pub struct NPCBundle {
    pub entity_id: EntityId,
    pub npc: NPC,
    pub position: Position,
    pub sprite: CreatureSprite,
    pub direction: Direction,
}

#[derive(Component)]
pub struct CreatureSprite {
    pub id: u16,
}

#[derive(Component)]
pub struct EntityId {
    pub id: u32,
}

#[derive(Component)]
pub struct GameCamera;

#[derive(Bundle)]
pub struct CameraBundle {
    pub camera: GameCamera,
    pub position: Position,
}

#[derive(Component)]
pub struct CameraTarget;

#[derive(Component)]
pub struct GameMap {
    pub map_id: u16,
    pub width: u8,
    pub height: u8,
    pub name: String,
}

#[derive(Component)]
pub struct MapLoaded;

#[derive(Bundle)]
pub struct MapBundle {
    pub map: GameMap,
    pub loaded: MapLoaded,
}

#[derive(Component, Debug, Clone)]
pub struct HealthBar {
    pub percent: u8,
    pub timer: Timer,
}

#[derive(Component)]
#[component(on_remove = cleanup_creature_instance)]
pub struct CreatureInstance {
    pub instance: AddCreatureResult,
}

// Mark newly spawned creatures for staged loading onto the renderer
#[derive(Component)]
pub struct CreatureLoadRequested;

#[derive(Component)]
pub struct CreatureLoading(pub Task<Result<AddCreatureResult, String>>);

#[derive(Component)]
pub struct MapLoadingTask(pub Task<MapPrepared>);

#[derive(Message)]
pub struct MapPrepared {
    pub map_id: u16,
    pub width: u8,
    pub height: u8,
    pub name: String,
    pub prepared_map: PreparedMap,
}

// Marker component: entity is tied to the currently loaded map and should be
// despawned wholesale on map change. Attach this to all runtime world
// entities (players, NPCs, items, map, etc.) so we can wipe with a single
// query instead of multiple type-specific queries.
#[derive(Component)]
pub struct MapScoped;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitboxType {
    /// Hitbox in isometric tile space (min/max are tile offsets from entity position)
    Isometric,
    /// Hitbox in screen space (min/max are pixel offsets from entity screen position)
    ScreenSpace,
}

#[derive(Component, Debug, Clone)]
pub struct Hitbox {
    pub min: Vec2,
    pub max: Vec2,
    pub hitbox_type: HitboxType,
}

impl Default for Hitbox {
    fn default() -> Self {
        Self {
            min: Vec2::new(0.0, 0.0),
            max: Vec2::new(1.0, 1.0),
            hitbox_type: HitboxType::Isometric,
        }
    }
}

impl Hitbox {
    pub fn isometric(min: Vec2, max: Vec2) -> Self {
        Self {
            min,
            max,
            hitbox_type: HitboxType::Isometric,
        }
    }

    pub fn screen_space(min: Vec2, max: Vec2) -> Self {
        Self {
            min,
            max,
            hitbox_type: HitboxType::ScreenSpace,
        }
    }

    pub fn check_hit(
        &self,
        entity_pos: Vec2,
        test_tile: Vec2,
        test_screen: Vec2,
        camera_pos: Vec2,
        window_size: Vec2,
        zoom: f32,
    ) -> bool {
        match self.hitbox_type {
            HitboxType::Isometric => {
                let bounds_min = entity_pos + self.min;
                let bounds_max = entity_pos + self.max;

                test_tile.x >= bounds_min.x
                    && test_tile.x < bounds_max.x
                    && test_tile.y >= bounds_min.y
                    && test_tile.y < bounds_max.y
            }
            HitboxType::ScreenSpace => {
                let entity_screen = rendering::scene::utils::tile_to_screen(
                    entity_pos,
                    camera_pos,
                    window_size,
                    zoom,
                );
                let bounds_min = entity_screen
                    + self.min
                        * Vec2::new(
                            rendering::scene::TILE_WIDTH as f32,
                            rendering::scene::TILE_HEIGHT as f32,
                        );
                let bounds_max = entity_screen
                    + self.max
                        * Vec2::new(
                            rendering::scene::TILE_WIDTH as f32,
                            rendering::scene::TILE_HEIGHT as f32,
                        );

                test_screen.x >= bounds_min.x
                    && test_screen.x < bounds_max.x
                    && test_screen.y >= bounds_min.y
                    && test_screen.y < bounds_max.y
            }
        }
    }
}

#[derive(Component)]
pub struct TargetingHover {
    pub tint: Vec3,
}

impl Default for TargetingHover {
    fn default() -> Self {
        Self {
            tint: Vec3::new(0.1, 0.15, 0.3),
        }
    }
}

#[derive(Component)]
/// Displays a name above the entity when hovered.
/// Rendered via immediate mode (cleared every frame), so no cleanup hook is required.
pub struct HoverName {
    pub name: String,
    pub color: glam::Vec4,
}

impl HoverName {
    pub fn new(name: String) -> Self {
        Self {
            name,
            color: glam::Vec4::new(1.0, 1.0, 1.0, 1.0),
        }
    }
}

/// Shared data structure for world labels (rendered by Slint)
#[derive(Clone)]
pub struct WorldLabel {
    pub text: String,
    pub y_offset: f32,
    pub color: glam::Vec4,
    pub is_speech: bool,
}

/// Hover label component - entity name shown on hover
#[derive(Component, Clone)]
pub struct HoverLabel {
    pub text: String,
    pub color: glam::Vec4,
}

impl HoverLabel {
    pub fn new(text: impl Into<String>, color: glam::Vec4) -> Self {
        Self {
            text: text.into(),
            color,
        }
    }

    pub fn to_world_label(&self) -> WorldLabel {
        WorldLabel {
            text: self.text.clone(),
            y_offset: -80.0,
            color: self.color,
            is_speech: false,
        }
    }
}

/// Speech bubble component - normal/shout messages
#[derive(Component, Clone)]
pub struct SpeechBubble {
    pub text: String,
    pub timer: Timer,
    pub is_shout: bool,
}

impl SpeechBubble {
    pub fn new(text: impl Into<String>, duration_secs: f32, is_shout: bool) -> Self {
        Self {
            text: text.into(),
            timer: Timer::new(
                std::time::Duration::from_secs_f32(duration_secs),
                TimerMode::Once,
            ),
            is_shout,
        }
    }

    pub fn to_world_label(&self) -> WorldLabel {
        WorldLabel {
            text: self.text.clone(),
            y_offset: -70.0,
            color: if self.is_shout {
                glam::Vec4::new(1.0, 1.0, 0.0, 1.0) // Yellow for shouts
            } else {
                glam::Vec4::new(1.0, 1.0, 1.0, 1.0) // White for normal
            },
            is_speech: true,
        }
    }
}

/// Chant label component - spell chants (light blue text)
#[derive(Component, Clone)]
pub struct ChantLabel {
    pub text: String,
    pub timer: Timer,
}

impl ChantLabel {
    pub fn new(text: impl Into<String>, duration_secs: f32) -> Self {
        Self {
            text: text.into(),
            timer: Timer::new(
                std::time::Duration::from_secs_f32(duration_secs),
                TimerMode::Once,
            ),
        }
    }

    pub fn to_world_label(&self) -> WorldLabel {
        WorldLabel {
            text: self.text.clone(),
            y_offset: -95.0,
            color: glam::Vec4::new(0.5, 0.7, 1.0, 1.0),
            is_speech: false,
        }
    }
}

// --- Component removal hooks ---
// These run automatically when the component is removed or the entity despawns.
fn cleanup_player_sprite_instance(mut world: DeferredWorld, ctx: HookContext) {
    let entity = ctx.entity;
    let handle = if let Some(comp) = world.get::<PlayerSpriteInstance>(entity) {
        comp.handle
    } else {
        return;
    };
    // Limit immutable borrow scope
    let queue_ptr = if let Some(renderer) = world.get_resource::<crate::RendererState>() {
        &renderer.queue as *const _
    } else {
        return;
    };

    // Use UnsafeWorldCell to get both resources simultaneously to avoid borrow checker conflicts in DeferredWorld
    let cell = world.as_unsafe_world_cell();
    let mut store_state = unsafe { cell.get_resource_mut::<crate::PlayerAssetStoreState>() };
    let batch_state = unsafe { cell.get_resource::<crate::PlayerBatchState>() };

    let (Some(store), Some(batch)) = (store_state.as_mut(), batch_state) else {
        return;
    };

    unsafe {
        batch
            .batch
            .remove_player_sprite(&*queue_ptr, &mut store.store, handle);
    }
}

fn cleanup_creature_instance(mut world: DeferredWorld, ctx: HookContext) {
    let entity = ctx.entity;
    let handle = if let Some(comp) = world.get::<CreatureInstance>(entity) {
        comp.instance.handle
    } else {
        return;
    };
    let queue_ptr = if let Some(renderer) = world.get_resource::<crate::RendererState>() {
        &renderer.queue as *const _
    } else {
        return;
    };

    // Use UnsafeWorldCell to get both resources simultaneously to avoid borrow checker conflicts in DeferredWorld
    let cell = world.as_unsafe_world_cell();
    let mut store_state = unsafe { cell.get_resource_mut::<crate::CreatureAssetStoreState>() };
    let mut batch_state = unsafe { cell.get_resource_mut::<crate::CreatureBatchState>() };

    let (Some(store), Some(batch)) = (store_state.as_mut(), batch_state.as_mut()) else {
        return;
    };

    unsafe {
        batch
            .batch
            .remove_creature(&*queue_ptr, &mut store.store, handle);
    }
}

fn cleanup_item_instance(world: DeferredWorld, ctx: HookContext) {
    let entity = ctx.entity;
    let Some(instance) = world.get::<ItemInstance>(entity) else {
        return;
    };
    let Some(renderer) = world.get_resource::<crate::RendererState>() else {
        return;
    };
    let Some(items_batch) = world.get_resource::<crate::ItemBatchState>() else {
        return;
    };
    items_batch
        .batch
        .remove_item(&renderer.queue, instance.handle);
}

#[derive(Component)]
pub struct Effect {
    pub effect_id: u16,
    pub z_offset: f32,
}

#[derive(Component)]
#[component(on_remove = cleanup_effect_instance)]
pub struct EffectInstance {
    pub handle: EffectHandle,
    pub current_frame: usize,
    pub timer: Timer,
}

#[derive(Component)]
pub struct FollowsEntity(pub Entity);

fn cleanup_effect_instance(mut world: DeferredWorld, ctx: HookContext) {
    let entity = ctx.entity;
    let handle = if let Some(inst) = world.get::<EffectInstance>(entity) {
        inst.handle.clone()
    } else {
        return;
    };
    let queue_ptr = if let Some(renderer) = world.get_resource::<crate::RendererState>() {
        &renderer.queue as *const _
    } else {
        return;
    };
    let Some(mut effects) = world.get_resource_mut::<crate::EffectManagerState>() else {
        return;
    };
    unsafe {
        effects.effect_manager.remove_effect(&*queue_ptr, &handle);
    }
}
