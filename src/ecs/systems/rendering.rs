//! Systems for syncing ECS state to GPU renderers

use super::super::animation::{Animation, AnimationMode, AnimationType};
use super::super::components::*;
use crate::resources::{
    CharacterCreatorPreviewState, CreatureAssetStoreState, CreatureBatchState, ItemAssetStoreState,
    ItemBatchState, LobbyPortraitRenderer, LobbyPortraits, MinimapCacheState, MinimapMarkerEntry,
    MinimapMarkerSyncState, MinimapRendererState, PlayerAssetStoreState, PlayerBatchState,
    PlayerPortraitState, PortraitRenderTarget,
};
use crate::{Camera, RendererState, game_files::GameFiles, settings_types::Settings};
use bevy::prelude::*;
use formats::epf::EpfAnimationType;
use glam::{Vec2, Vec3};
use rendering::{
    instance::InstanceFlag,
    scene::{
        TILE_HEIGHT_HALF, TILE_WIDTH_HALF,
        minimap::{self, MinimapMarker as MinimapMarkerInstance, MinimapMarkerLayer, MinimapTile},
        players::{Gender, PlayerBatch, PlayerPieceType, PlayerSpriteKey},
    },
};

fn player_instance_state(render_state: &PlayerRenderState) -> InstanceFlag {
    if render_state.translucent {
        InstanceFlag::Translucent
    } else {
        InstanceFlag::None
    }
}

fn preload_player_sprite_keys(
    renderer: &RendererState,
    game_files: &GameFiles,
    player_store: &mut PlayerAssetStoreState,
    sprite_keys: &[PlayerSpriteKey],
) {
    if sprite_keys.is_empty() {
        return;
    }

    let _ = player_store.store.preload_player_sprites(
        &renderer.queue,
        &game_files.inner().archive(),
        sprite_keys,
    );
}

fn build_saved_preview_sprites(
    preview: &crate::settings::CharacterPreview,
) -> Vec<(PlayerSpriteKey, u8)> {
    let gender = if preview.is_male {
        Gender::Male
    } else {
        Gender::Female
    };

    let mut slots = vec![
        (
            PlayerPieceType::Body,
            preview.body.max(1),
            preview.shield_color as u8,
        ),
        (PlayerPieceType::Face, 1, preview.shield_color as u8),
        (
            PlayerPieceType::HelmetBg,
            preview.helmet,
            preview.helmet_color as u8,
        ),
        (
            PlayerPieceType::HelmetExtra,
            preview.helmet,
            preview.helmet_color as u8,
        ),
        (
            PlayerPieceType::HelmetFg,
            preview.helmet,
            preview.helmet_color as u8,
        ),
        (
            PlayerPieceType::Boots,
            preview.boots,
            preview.boots_color as u8,
        ),
        (PlayerPieceType::Shield, preview.shield, 0),
        (PlayerPieceType::Weapon, preview.weapon, 0),
        (
            PlayerPieceType::Accessory1Bg,
            preview.accessory1,
            preview.accessory1_color as u8,
        ),
        (
            PlayerPieceType::Accessory1Fg,
            preview.accessory1,
            preview.accessory1_color as u8,
        ),
    ];

    if preview.pants_color > 0 {
        slots.push((PlayerPieceType::Pants, 1, preview.pants_color as u8));
    }

    if preview.overcoat > 0 {
        slots.push((
            PlayerPieceType::Armor,
            preview.overcoat,
            preview.overcoat_color as u8,
        ));
    } else {
        slots.push((PlayerPieceType::Arms, preview.armor, 0));
        slots.push((PlayerPieceType::Armor, preview.armor, 0));
    }

    slots
        .into_iter()
        .filter(|(slot, id, _)| *id != 0 || *slot == PlayerPieceType::Body)
        .map(|(slot, id, color)| (PlayerSpriteKey::for_piece(slot, id, gender), color))
        .collect()
}

fn build_character_creator_preview_sprites(
    preview: &crate::resources::CharacterCreatorPreviewState,
) -> Vec<(PlayerSpriteKey, u8)> {
    let gender = if preview.gender == 1 {
        Gender::Male
    } else {
        Gender::Female
    };

    [
        (PlayerPieceType::Body, 1, 0),
        (PlayerPieceType::Face, 1, 0),
        (
            PlayerPieceType::HelmetBg,
            preview.hair_style as u16,
            preview.hair_color,
        ),
        (
            PlayerPieceType::HelmetFg,
            preview.hair_style as u16,
            preview.hair_color,
        ),
        (PlayerPieceType::Arms, preview.armor_id, 0),
        (PlayerPieceType::Armor, preview.armor_id, 0),
    ]
    .into_iter()
    .filter(|(slot, id, _)| *id != 0 || *slot == PlayerPieceType::Body)
    .map(|(slot, id, color)| (PlayerSpriteKey::for_piece(slot, id, gender), color))
    .collect()
}

fn populate_player_batch_with_sprites(
    renderer: &RendererState,
    game_files: &GameFiles,
    player_store: &mut PlayerAssetStoreState,
    batch: &PlayerBatch,
    sprites: &[(PlayerSpriteKey, u8)],
    direction: u8,
) {
    let sprite_keys: Vec<PlayerSpriteKey> = sprites.iter().map(|(key, _)| *key).collect();
    preload_player_sprite_keys(renderer, game_files, player_store, &sprite_keys);

    for &(key, color) in sprites {
        let _ = batch.add_player_sprite(
            &renderer.queue,
            &mut player_store.store,
            &game_files.inner().archive(),
            key,
            color,
            direction,
            -0.7,
            -0.7,
            0,
            rendering::instance::InstanceFlag::None,
            glam::Vec3::ZERO,
        );
    }
}

/// Syncs character previews for the lobby screen.
pub fn sync_lobby_portraits(
    renderer: Res<RendererState>,
    game_files: Res<GameFiles>,
    settings: Res<Settings>,
    mut portrait_state: ResMut<LobbyPortraits>,
    lobby_renderer: ResMut<LobbyPortraitRenderer>,
    mut player_store: ResMut<PlayerAssetStoreState>,
    _win: Res<crate::slint_support::state_bridge::SlintWindow>,
) {
    // Only run if portraits are complete and we have saved credentials
    if settings
        .saved_credentials
        .iter()
        .all(|c| c.preview.is_none() || portrait_state.textures.contains_key(&c.id))
    {
        return;
    }

    let portrait_size = 64;

    for cred in &settings.saved_credentials {
        if let Some(preview) = &cred.preview {
            // Skip if already rendered
            if portrait_state.textures.contains_key(&cred.id) {
                continue;
            }

            lobby_renderer
                .batch
                .clear_and_unload(&mut player_store.store);
            let sprites = build_saved_preview_sprites(preview);
            populate_player_batch_with_sprites(
                &renderer,
                &game_files,
                &mut player_store,
                &lobby_renderer.batch,
                &sprites,
                2,
            );

            let texture = rendering::texture::Texture::create_render_texture(
                &renderer.device,
                "lobby_portrait",
                portrait_size,
                portrait_size,
                wgpu::TextureFormat::Rgba8Unorm,
            );

            render_player_batch_to_target(
                &renderer,
                &lobby_renderer.batch,
                &texture.view,
                &lobby_renderer.depth_texture.view,
                &lobby_renderer.camera,
            );

            portrait_state
                .textures
                .insert(cred.id.clone(), texture.texture);
        }
    }

    lobby_renderer
        .batch
        .clear_and_unload(&mut player_store.store);
    portrait_state.version += 1;
}

/// Syncs character preview for the character creator screen.
pub fn sync_character_creator_preview(
    renderer: Res<RendererState>,
    game_files: Res<GameFiles>,
    mut portrait_state: ResMut<CharacterCreatorPreviewState>,
    mut player_store: ResMut<PlayerAssetStoreState>,
    _win: Res<crate::slint_support::state_bridge::SlintWindow>,
) {
    if !portrait_state.dirty {
        return;
    }

    let sprites = build_character_creator_preview_sprites(&portrait_state);
    let Some(target) = portrait_state.target.as_mut() else {
        return;
    };

    render_sprites_to_portrait_target(&renderer, &game_files, &mut player_store, target, &sprites);
    portrait_state.version = target.version;
}

/// Collects all sprite keys and colors for a player entity.
pub fn collect_player_sprites(
    player: &Player,
    children: &Children,
    sprite_query: &Query<&PlayerSprite>,
) -> Vec<(PlayerSpriteKey, u8)> {
    let mut sprites = Vec::new();
    let gender = if player.is_male {
        Gender::Male
    } else {
        Gender::Female
    };

    for child in children.iter() {
        if let Ok(sprite) = sprite_query.get(child) {
            sprites.push((
                PlayerSpriteKey::for_piece(sprite.slot, sprite.id, gender),
                sprite.color,
            ));
        }
    }
    sprites
}

fn render_sprites_to_portrait_target(
    renderer: &RendererState,
    game_files: &GameFiles,
    player_store: &mut PlayerAssetStoreState,
    target: &mut PortraitRenderTarget,
    sprites: &[(PlayerSpriteKey, u8)],
) {
    target.batch.clear_and_unload(&mut player_store.store);
    populate_player_batch_with_sprites(
        renderer,
        game_files,
        player_store,
        &target.batch,
        sprites,
        1,
    );

    render_player_batch_to_target(
        renderer,
        &target.batch,
        &target.view,
        &target.depth_texture.view,
        &target.camera,
    );

    target.dirty = false;
    target.version += 1;
}

/// Syncs item entities to the GPU item renderer.
pub fn sync_items_to_renderer(
    mut commands: Commands,
    renderer: Option<Res<RendererState>>,
    game_files: Option<Res<GameFiles>>,
    items_store: Option<ResMut<ItemAssetStoreState>>,
    items_batch: Option<ResMut<ItemBatchState>>,
    added_items: Query<(Entity, &Position, &ItemSprite, &EntityId), Added<ItemSprite>>,
) {
    let (Some(renderer), Some(files), Some(mut store), Some(mut batch)) =
        (renderer, game_files, items_store, items_batch)
    else {
        return;
    };

    for (entity, position, sprite, entity_id) in added_items.iter() {
        if let Some(handle) = batch.batch.add_item(
            &renderer.queue,
            &mut store.store,
            &files.inner().archive(),
            rendering::scene::items::Item {
                id: entity_id.id,
                x: position.x as u16,
                y: position.y as u16,
                sprite: sprite.id,
                color: sprite.color,
                spawn_order: sprite.spawn_order,
            },
        ) {
            commands.entity(entity).insert(ItemInstance { handle });
        }
    }
}

/// Updates item positions and sprites on the GPU when they change.
pub fn update_items_to_renderer(
    renderer: Res<RendererState>,
    items_store: Res<ItemAssetStoreState>,
    items_batch: Res<ItemBatchState>,
    query: Query<
        (&ItemInstance, &Position, &ItemSprite, &EntityId),
        Or<(Changed<Position>, Changed<ItemSprite>)>,
    >,
) {
    for (instance, position, sprite, entity_id) in query.iter() {
        items_batch.batch.update_item(
            &renderer.queue,
            &items_store.store,
            &instance.handle,
            rendering::scene::items::Item {
                id: entity_id.id,
                x: position.x as u16,
                y: position.y as u16,
                sprite: sprite.id,
                color: sprite.color,
                spawn_order: sprite.spawn_order,
            },
        );
    }
}

/// Syncs player sprite entities to the GPU player renderer.
pub fn sync_players_to_renderer(
    mut commands: Commands,
    shared_state: Res<RendererState>,
    game_files: Res<GameFiles>,
    added_sprites: Query<(Entity, &ChildOf, &PlayerSprite), Added<PlayerSprite>>,
    player_query: Query<(
        &Position,
        &Direction,
        &Player,
        &EntityId,
        &PlayerRenderState,
        Option<&TargetingHover>,
    )>,
    mut store_state: ResMut<PlayerAssetStoreState>,
    batch_state: Res<PlayerBatchState>,
) {
    let mut sprites_to_add = Vec::new();
    for (sprite_entity, child_of, sprite) in added_sprites.iter() {
        if let Ok((position, direction, player, entity_id, render_state, targeting_hover)) =
            player_query.get(child_of.parent())
        {
            sprites_to_add.push((
                sprite_entity,
                child_of,
                sprite,
                position,
                direction,
                player,
                entity_id,
                render_state,
                targeting_hover,
            ));
        }
    }

    let sprite_keys: Vec<PlayerSpriteKey> = sprites_to_add
        .iter()
        .map(|(_, _, sprite, _, _, player, _, _, _)| {
            let gender = if player.is_male {
                Gender::Male
            } else {
                Gender::Female
            };
            PlayerSpriteKey::for_piece(sprite.slot, sprite.id, gender)
        })
        .collect();

    preload_player_sprite_keys(&shared_state, &game_files, &mut store_state, &sprite_keys);

    for (
        sprite_entity,
        _child_of,
        sprite,
        position,
        direction,
        player,
        entity_id,
        render_state,
        targeting_hover,
    ) in sprites_to_add
    {
        let gender = if player.is_male {
            Gender::Male
        } else {
            Gender::Female
        };

        let tint = targeting_hover.map(|t| t.tint).unwrap_or(Vec3::ZERO);
        let flags = player_instance_state(render_state);
        let result = batch_state.batch.add_player_sprite(
            &shared_state.queue,
            &mut store_state.store,
            &game_files.inner().archive(),
            PlayerSpriteKey::for_piece(sprite.slot, sprite.id, gender),
            sprite.color,
            *direction as u8,
            position.x,
            position.y,
            entity_id.id,
            flags,
            tint,
        );

        if let Ok(handle) = result {
            commands
                .entity(sprite_entity)
                .insert(PlayerSpriteInstance { handle });
        }
    }
}

/// Updates player sprite positions and animations on the GPU.
pub fn update_player_sprites(
    shared_state: Res<RendererState>,
    store_state: Res<PlayerAssetStoreState>,
    batch_state: Res<PlayerBatchState>,
    parent_query: Query<(
        Entity,
        &Position,
        &Direction,
        Option<&Animation>,
        Option<&crate::ecs::animation::AnimationTimer>,
        &PlayerRenderState,
        Option<&TargetingHover>,
        &Children,
        &EntityId,
    )>,
    _removed_hovers: RemovedComponents<TargetingHover>,
    children_query: Query<(&PlayerSprite, &PlayerSpriteInstance)>,
    time: Res<Time>,
) {
    for (
        _entity,
        position,
        direction,
        animation,
        animation_timer,
        render_state,
        targeting_hover,
        children,
        _entity_id,
    ) in parent_query.iter()
    {
        let (anim_type, progress) = match (animation, animation_timer) {
            (Some(anim), Some(timer)) if anim.mode != AnimationMode::Finished => {
                if let AnimationType::Player(at) = anim.anim_type {
                    (at, timer.0.fraction())
                } else {
                    (EpfAnimationType::Idle, time.elapsed_secs() % 1.0)
                }
            }
            _ => (EpfAnimationType::Idle, time.elapsed_secs() % 1.0),
        };

        let tint = targeting_hover.map(|t| t.tint).unwrap_or(Vec3::ZERO);
        let flags = player_instance_state(render_state);

        for child_entity in children.iter() {
            if let Ok((sprite, sprite_instance)) = children_query.get(child_entity) {
                let target = match (anim_type.is_emote(), sprite.slot) {
                    (true, PlayerPieceType::Emote) => Some((anim_type, progress)), // Active emote layer
                    (true, PlayerPieceType::Face) => None, // Hide face when emoting (face usually in emote)
                    (true, _) => Some((EpfAnimationType::Idle, time.elapsed_secs() % 1.0)), // Base layers go to Idle
                    (false, PlayerPieceType::Emote) => None, // Hide emote layer when not emoting
                    (false, _) => Some((anim_type, progress)), // Standard animation
                };

                if let Some((at, p)) = target {
                    if let Err(e) = batch_state.batch.update_player_sprite_with_animation(
                        &shared_state.queue,
                        &store_state.store,
                        &sprite_instance.handle,
                        *direction as u8,
                        position.x,
                        position.y,
                        sprite.color,
                        at,
                        p,
                        flags,
                        tint,
                    ) {
                        if at.is_emote() {
                            // If emote fails (e.g. facing away), just hide the emote layer
                            let _ = batch_state
                                .batch
                                .hide_player_sprite(&shared_state.queue, &sprite_instance.handle);
                        } else if !anim_type.is_emote() {
                            tracing::error!("update_player_sprite_with_animation failed: {:?}", e);
                        }
                    }
                } else {
                    let _ = batch_state
                        .batch
                        .hide_player_sprite(&shared_state.queue, &sprite_instance.handle);
                }
            }
        }
    }
}

/// Syncs creature positions and animations to the GPU.
pub fn creature_movement_sync(
    renderer: Res<RendererState>,
    query: Query<(
        &CreatureInstance,
        &Position,
        &Direction,
        &Animation,
        Option<&TargetingHover>,
        &EntityId,
    )>,
    changed_query: Query<
        Entity,
        Or<(
            Changed<Position>,
            Changed<Direction>,
            Changed<Animation>,
            Changed<TargetingHover>,
        )>,
    >,
    mut removed_hovers: RemovedComponents<TargetingHover>,
    creatures_store: Res<CreatureAssetStoreState>,
    creatures_batch: Res<CreatureBatchState>,
) {
    use formats::mpf::MpfAnimationType;

    let mut to_update = changed_query
        .iter()
        .collect::<std::collections::HashSet<_>>();
    for entity in removed_hovers.read() {
        to_update.insert(entity);
    }

    for entity in to_update {
        if let Ok((creature, pos, dir, anim, targeting_hover, _entity_id)) = query.get(entity) {
            let (actual_anim_type, actual_frame) = if anim.mode == AnimationMode::Finished {
                (MpfAnimationType::Standing, 0)
            } else if let AnimationType::Creature(at) = anim.anim_type {
                (at, anim.current_frame)
            } else {
                (MpfAnimationType::Standing, 0)
            };

            if let Some(mpf_anim) = creature.instance.get_animation(actual_anim_type) {
                let tint = targeting_hover.map(|t| t.tint).unwrap_or(Vec3::ZERO);
                creatures_batch.batch.update_creature(
                    &renderer.queue,
                    &creatures_store.store,
                    &creature.instance.handle,
                    pos.x,
                    pos.y,
                    mpf_anim,
                    actual_frame,
                    *dir as u8,
                    tint,
                );
            }
        }
    }
}

/// Syncs the local player's appearance to the portrait texture.
pub fn sync_player_portrait(
    renderer: Res<RendererState>,
    game_files: Res<GameFiles>,
    local_player_query: Query<(&Player, &Children, Option<&CreatureSprite>), With<LocalPlayer>>,
    sprite_query: Query<&PlayerSprite>,
    mut portrait_state: ResMut<PlayerPortraitState>,
    mut player_store: ResMut<PlayerAssetStoreState>,
    changed_query: Query<Entity, (With<LocalPlayer>, Or<(Changed<Children>, Changed<Player>)>)>,
    sprite_changed_query: Query<(), (With<PlayerSprite>, Changed<PlayerSprite>)>,
) {
    let mut needs_update = portrait_state.dirty;

    if !changed_query.is_empty() {
        needs_update = true;
    }

    if !needs_update {
        if let Some((_, children, _)) = local_player_query.iter().next() {
            for child in children.iter() {
                if sprite_changed_query.get(child).is_ok() {
                    needs_update = true;
                    break;
                }
            }
        }
    }

    if needs_update {
        if let Some((player, children, creature_sprite)) = local_player_query.iter().next() {
            if creature_sprite.is_some() {
                portrait_state.dirty = false;
                return;
            }

            let sprites = collect_player_sprites(player, children, &sprite_query);
            render_sprites_to_portrait_target(
                &renderer,
                &game_files,
                &mut player_store,
                &mut portrait_state.target,
                &sprites,
            );
        }
    }
}

/// Helper to render a player batch to a specific texture target.
pub fn render_player_batch_to_target(
    renderer: &RendererState,
    batch: &PlayerBatch,
    color_view: &wgpu::TextureView,
    depth_view: &wgpu::TextureView,
    camera: &rendering::scene::CameraState,
) {
    let mut encoder = renderer
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Player Export Render Encoder"),
        });

    {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Player Export Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: color_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(0.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });

        render_pass.set_pipeline(&renderer.scene.pipeline);
        render_pass.set_bind_group(1, &camera.camera_bind_group, &[]);
        batch.render(&mut render_pass);
    }

    renderer.queue.submit([encoder.finish()]);
}

/// Syncs any player's appearance (local or other) to the profile portrait texture.
pub fn sync_profile_portrait(
    renderer: Res<RendererState>,
    game_files: Res<GameFiles>,
    profile_state: Res<crate::webui::plugin::PlayerProfileState>,
    local_player_query: Query<(&Player, &Children, Option<&CreatureSprite>), With<LocalPlayer>>,
    other_players_query: Query<
        (&Player, &Children, &EntityId, Option<&CreatureSprite>),
        Without<LocalPlayer>,
    >,
    sprite_query: Query<&PlayerSprite>,
    mut portrait_state: ResMut<crate::resources::ProfilePortraitState>,
    mut player_store: ResMut<PlayerAssetStoreState>,
    mut last_entity_id: Local<Option<u32>>,
) {
    let mut target_entity = None;

    if let Some(eid) = profile_state.entity_id {
        // Find other player by server ID
        for (player, children, id, creature_sprite) in other_players_query.iter() {
            if id.id == eid {
                target_entity = Some((player, children, creature_sprite.is_some()));
                break;
            }
        }
    } else {
        // Use local player
        if let Some((player, children, creature_sprite)) = local_player_query.iter().next() {
            target_entity = Some((player, children, creature_sprite.is_some()));
        }
    }

    // Force update if the entity we're looking at changed
    if profile_state.entity_id != *last_entity_id {
        portrait_state.dirty = true;
        *last_entity_id = profile_state.entity_id;
    }

    if portrait_state.dirty {
        if let Some((player, children, uses_creature_sprite)) = target_entity {
            if uses_creature_sprite {
                portrait_state.dirty = false;
                return;
            }

            let sprites = collect_player_sprites(player, children, &sprite_query);
            render_sprites_to_portrait_target(
                &renderer,
                &game_files,
                &mut player_store,
                &mut portrait_state.target,
                &sprites,
            );
        }
    }
}

pub fn sync_minimap_to_renderer(
    renderer: Res<RendererState>,
    camera: Res<Camera>,
    minimap_state: Option<ResMut<MinimapRendererState>>,
    minimap_cache: Option<ResMut<MinimapCacheState>>,
    minimap_markers: Option<ResMut<MinimapMarkerSyncState>>,
    map_query: Query<&GameMap>,
    marker_query: Query<(Entity, &Position, &crate::ecs::components::MinimapMarker)>,
    changed_marker_query: Query<
        (Entity, &Position, &crate::ecs::components::MinimapMarker),
        Or<(
            Added<crate::ecs::components::MinimapMarker>,
            Changed<Position>,
            Changed<crate::ecs::components::MinimapMarker>,
        )>,
    >,
    mut removed_markers: RemovedComponents<crate::ecs::components::MinimapMarker>,
    collision_table: Option<Res<crate::ecs::collision::WallCollisionTable>>,
    map_collision: Option<Res<crate::ecs::collision::MapCollisionData>>,
) {
    let Some(mut minimap_state) = minimap_state else {
        return;
    };

    let (Some(mut minimap_cache), Some(mut minimap_markers)) = (minimap_cache, minimap_markers)
    else {
        minimap_state.renderer.clear();
        return;
    };

    let Ok(map) = map_query.single() else {
        minimap_state.renderer.clear();
        return;
    };

    let layout = minimap_state.config.layout;
    let layout_changed = minimap_state.renderer.layout() != layout;
    if layout_changed {
        minimap_state.renderer.set_layout(layout);
    }

    let zoom = minimap_state.config.zoom;
    if (minimap_state.camera.zoom() - zoom).abs() > f32::EPSILON {
        minimap_state.camera.set_zoom(&renderer.queue, zoom);
    }

    let camera_position = minimap_camera_position(camera.camera.position(), layout);
    if minimap_state.camera.position() != camera_position {
        minimap_state.camera.set_screen_offset(
            &renderer.queue,
            camera_position.x,
            camera_position.y,
        );
    }

    let map_changed = minimap_cache.map_id != map.map_id
        || minimap_cache.map_width != map.width
        || minimap_cache.map_height != map.height;
    if map_changed {
        *minimap_cache = MinimapCacheState::new(map.map_id, map.width, map.height);
    }

    let hidden_atlas_index = atlas_index_for_tile_value(0b0000);
    let topology_dirty = minimap_cache.topology_dirty;

    if topology_dirty {
        let lattice_width = map.width as usize;
        let lattice_height = map.height as usize;
        let mut tile_atlas_indices = Vec::with_capacity(lattice_width * lattice_height);

        for lattice_y in 0..lattice_height {
            for lattice_x in 0..lattice_width {
                let atlas_index = minimap_lattice_index(
                    lattice_x,
                    lattice_y,
                    map.width,
                    map.height,
                    |sample_x, sample_y| {
                        crate::ecs::collision::can_walk_to(
                            sample_x,
                            sample_y,
                            collision_table.as_deref(),
                            map_collision.as_deref(),
                        )
                    },
                );
                tile_atlas_indices.push(atlas_index);
            }
        }

        minimap_cache.tile_atlas_indices = tile_atlas_indices;
        minimap_cache.topology_dirty = false;
    }

    if map_changed || layout_changed || topology_dirty {
        let lattice_width = map.width as usize;
        let lattice_height = map.height as usize;
        let mut tiles = Vec::with_capacity(lattice_width * lattice_height);

        for lattice_y in 0..lattice_height {
            for lattice_x in 0..lattice_width {
                let index = lattice_y * lattice_width + lattice_x;
                let atlas_index = minimap_cache
                    .tile_atlas_indices
                    .get(index)
                    .copied()
                    .unwrap_or(hidden_atlas_index);
                if atlas_index == hidden_atlas_index {
                    continue;
                }

                let tile = Vec2::new(lattice_x as f32, lattice_y as f32);
                tiles.push(MinimapTile {
                    position: minimap::minimap_tile_position(tile, Vec2::ZERO, layout),
                    atlas_index,
                });
            }
        }

        minimap_state
            .renderer
            .rebuild_tiles(&renderer.device, tiles);
    }

    for entity in removed_markers.read() {
        if let Some(marker) = minimap_markers.handles.remove(&entity) {
            minimap_state
                .renderer
                .remove_marker(&renderer.queue, marker.handle);
        }
    }

    if map_changed || layout_changed {
        for (entity, position, marker) in marker_query.iter() {
            upsert_minimap_marker(
                &renderer.queue,
                &minimap_state.renderer,
                &mut minimap_markers,
                entity,
                position,
                marker,
                Vec2::ZERO,
                layout,
            );
        }
    } else {
        for (entity, position, marker) in changed_marker_query.iter() {
            upsert_minimap_marker(
                &renderer.queue,
                &minimap_state.renderer,
                &mut minimap_markers,
                entity,
                position,
                marker,
                Vec2::ZERO,
                layout,
            );
        }
    }
}

fn upsert_minimap_marker(
    queue: &wgpu::Queue,
    renderer: &rendering::scene::minimap::MinimapRenderer,
    marker_state: &mut MinimapMarkerSyncState,
    entity: Entity,
    position: &Position,
    marker: &crate::ecs::components::MinimapMarker,
    center: Vec2,
    layout: minimap::MinimapLayout,
) {
    let marker_position =
        minimap::minimap_marker_position(Vec2::new(position.x, position.y), center, layout);
    let instance = MinimapMarkerInstance {
        position: marker_position,
    };

    let existing_handle = marker_state.handles.get(&entity).map(|entry| entry.handle);
    let existing_kind = marker_state.handles.get(&entity).map(|entry| entry.kind);

    if let Some(existing_handle) = existing_handle {
        let expected_layer = match marker.kind {
            crate::ecs::components::MinimapMarkerKind::Player => MinimapMarkerLayer::Player,
            crate::ecs::components::MinimapMarkerKind::Creature => MinimapMarkerLayer::Creature,
        };

        if existing_handle.layer() == expected_layer && existing_kind == Some(marker.kind) {
            renderer.update_marker(queue, existing_handle, instance);
            return;
        }

        renderer.remove_marker(queue, existing_handle);
    }

    let handle = match marker.kind {
        crate::ecs::components::MinimapMarkerKind::Player => {
            renderer.add_player_marker(queue, instance)
        }
        crate::ecs::components::MinimapMarkerKind::Creature => {
            renderer.add_creature_marker(queue, instance)
        }
    };

    if let Some(handle) = handle {
        marker_state.handles.insert(
            entity,
            MinimapMarkerEntry {
                handle,
                kind: marker.kind,
            },
        );
    } else {
        marker_state.handles.remove(&entity);
    }
}

fn minimap_lattice_index(
    lattice_x: usize,
    lattice_y: usize,
    map_width: u8,
    map_height: u8,
    is_walkable: impl FnMut(u8, u8) -> bool,
) -> u8 {
    minimap_lattice_index_from_walkability(lattice_x, lattice_y, map_width, map_height, is_walkable)
}

// Atlas ordering for the 4-bit minimap mask produced by the corner-based 2x2
// tile sampler below.
const SIMPLE_TILE_VALUES: [u8; 16] = [
    0b1101, 0b1010, 0b0100, 0b1100, 0b0110, 0b1000, 0b0000, 0b0001, 0b1011, 0b0011, 0b0010, 0b0101,
    0b1111, 0b1110, 0b1001, 0b0111,
];

const MINIMAP_MASK_TOP_LEFT: u8 = 0b1000;
const MINIMAP_MASK_TOP_RIGHT: u8 = 0b0100;
const MINIMAP_MASK_BOTTOM_LEFT: u8 = 0b0010;
const MINIMAP_MASK_BOTTOM_RIGHT: u8 = 0b0001;

fn atlas_index_for_tile_value(tile_value: u8) -> u8 {
    SIMPLE_TILE_VALUES
        .iter()
        .position(|candidate| *candidate == tile_value)
        .expect("tile value must map to an atlas index") as u8
}

fn minimap_camera_position(camera_position: Vec2, layout: minimap::MinimapLayout) -> Vec2 {
    let world_x = (camera_position.x / TILE_WIDTH_HALF as f32
        + camera_position.y / TILE_HEIGHT_HALF as f32)
        * 0.5;
    let world_y = (camera_position.y / TILE_HEIGHT_HALF as f32
        - camera_position.x / TILE_WIDTH_HALF as f32)
        * 0.5;

    minimap::minimap_tile_position(Vec2::new(world_x, world_y), Vec2::ZERO, layout)
}

// Render one minimap tile per world tile, but choose that tile by sampling the
// corner at (x + 0.5, y + 0.5), which pulls in the surrounding 2x2 world tiles.

fn minimap_lattice_index_from_walkability(
    lattice_x: usize,
    lattice_y: usize,
    map_width: u8,
    map_height: u8,
    mut is_walkable: impl FnMut(u8, u8) -> bool,
) -> u8 {
    if map_width == 0 || map_height == 0 {
        return atlas_index_for_tile_value(0);
    }

    let is_blocked_sample =
        |sample_x: i32, sample_y: i32, is_walkable: &mut dyn FnMut(u8, u8) -> bool| {
            !is_walkable(
                sample_x.min(map_width as i32 - 1).max(0) as u8,
                sample_y.min(map_height as i32 - 1).max(0) as u8,
            )
        };

    let world_x = lattice_x as i32;
    let world_y = lattice_y as i32;

    let mut dual_mask = 0_u8;
    if is_blocked_sample(world_x, world_y, &mut is_walkable) {
        dual_mask |= MINIMAP_MASK_TOP_LEFT;
    }
    if is_blocked_sample(world_x + 1, world_y, &mut is_walkable) {
        dual_mask |= MINIMAP_MASK_TOP_RIGHT;
    }
    if is_blocked_sample(world_x, world_y + 1, &mut is_walkable) {
        dual_mask |= MINIMAP_MASK_BOTTOM_LEFT;
    }
    if is_blocked_sample(world_x + 1, world_y + 1, &mut is_walkable) {
        dual_mask |= MINIMAP_MASK_BOTTOM_RIGHT;
    }

    atlas_index_for_tile_value(dual_mask)
}

#[cfg(test)]
mod tests {
    use super::{SIMPLE_TILE_VALUES, minimap, minimap_lattice_index_from_walkability};
    use glam::Vec2;

    fn minimap_lattice_from_blocked_grid(width: usize, blocked: &[u8]) -> Vec<u8> {
        let blocked_rows = blocked.chunks_exact(width).collect::<Vec<_>>();
        let walkable = blocked_rows
            .iter()
            .map(|row| row.iter().map(|cell| *cell != 0).collect::<Vec<_>>())
            .collect::<Vec<_>>();
        let walkable_rows = walkable.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let map_height = walkable_rows.len() as u8;
        let map_width = walkable_rows.first().map_or(0, |row| row.len()) as u8;
        let lattice_width = width;
        let lattice_height = walkable_rows.len();
        let mut indices = Vec::with_capacity(lattice_width * lattice_height);

        for lattice_y in 0..lattice_height {
            for lattice_x in 0..lattice_width {
                indices.push(minimap_lattice_index_from_walkability(
                    lattice_x,
                    lattice_y,
                    map_width,
                    map_height,
                    |sx, sy| walkable_rows[sy as usize][sx as usize],
                ));
            }
        }

        indices
    }

    #[test]
    fn lattice_size_matches_world_tile_count() {
        let blocked = [0, 0, 0, 0, 0, 0, 0, 0, 0];
        let lattice = minimap_lattice_from_blocked_grid(3, &blocked);

        assert_eq!(lattice.len(), 9);
    }

    #[test]
    fn simple_tile_values_cover_every_mask_once() {
        let mut sorted = SIMPLE_TILE_VALUES;
        sorted.sort_unstable();

        assert_eq!(
            sorted,
            [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]
        );
    }

    #[test]
    fn minimap_tile_position_uses_world_origin() {
        let layout = crate::resources::MinimapViewConfig::default().layout;

        assert_eq!(
            minimap::minimap_tile_position(Vec2::new(1.0, 1.0), Vec2::ZERO, layout),
            Vec2::new(0.0, layout.tile_size.y)
        );
    }

    #[test]
    fn minimap_marker_position_keeps_half_tile_offset() {
        let layout = crate::resources::MinimapViewConfig::default().layout;

        assert_eq!(
            minimap::minimap_marker_position(Vec2::new(0.0, 0.0), Vec2::ZERO, layout),
            Vec2::new(0.0, -(layout.tile_size.y * 0.5))
        );
    }
}
