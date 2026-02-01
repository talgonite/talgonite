//! Systems for syncing ECS state to GPU renderers

use super::super::animation::{Animation, AnimationMode, AnimationType};
use super::super::components::*;
use crate::resources::LobbyPortraits;
use crate::resources::PlayerPortraitState;
use crate::{
    CreatureAssetStoreState, CreatureBatchState, ItemAssetStoreState, ItemBatchState,
    PlayerAssetStoreState, PlayerBatchState, RendererState, game_files::GameFiles,
    settings_types::Settings,
};
use bevy::prelude::*;
use formats::epf::EpfAnimationType;
use glam::Vec3;
use rendering::{
    instance::InstanceFlag,
    scene::players::{Gender, PlayerBatch, PlayerPieceType, PlayerSpriteKey},
};

/// Syncs character previews for the lobby screen.
pub fn sync_lobby_portraits(
    renderer: Res<RendererState>,
    game_files: Res<GameFiles>,
    settings: Res<Settings>,
    mut portrait_state: ResMut<LobbyPortraits>,
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
    let batch = rendering::scene::players::PlayerBatch::new(&renderer.device, &player_store.store);

    let depth_texture = rendering::texture::Texture::create_depth_texture(
        &renderer.device,
        portrait_size,
        portrait_size,
        "lobby_portrait_depth",
    );
    let mut camera = rendering::scene::CameraState::new(
        glam::UVec2::new(portrait_size, portrait_size),
        &renderer.device,
        1.0,
    );
    camera.set_screen_offset(&renderer.queue, 0.0, -42.0);

    for cred in &settings.saved_credentials {
        if let Some(preview) = &cred.preview {
            // Skip if already rendered
            if portrait_state.textures.contains_key(&cred.id) {
                continue;
            }

            batch.clear_and_unload(&mut player_store.store);

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
                (PlayerPieceType::Face, 1, 0), // Standard face
                (
                    PlayerPieceType::HelmetBg,
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
                slots.push((PlayerPieceType::Pants, preview.pants_color as u16, 1));
            }

            if preview.overcoat > 0 {
                slots.push((
                    PlayerPieceType::Armor,
                    preview.overcoat,
                    preview.overcoat_color as u8,
                ));
            } else {
                slots.push((PlayerPieceType::Arms, preview.armor, 0));
                slots.push((PlayerPieceType::Armor, preview.armor, 0)); // Assuming same sprite for now, common for basic armors
            }

            for (slot, id, color) in slots {
                if id != 0 || slot == PlayerPieceType::Body {
                    let _ = batch.add_player_sprite(
                        &renderer.queue,
                        &mut player_store.store,
                        &game_files.inner().archive(),
                        PlayerSpriteKey {
                            gender,
                            slot,
                            sprite_id: id,
                        },
                        color,
                        2, // Down
                        0.0,
                        0.0,
                        0, // No stacking for preview
                        InstanceFlag::None,
                        glam::Vec3::ZERO,
                    );
                }
            }

            let texture = rendering::texture::Texture::create_render_texture(
                &renderer.device,
                "lobby_portrait",
                portrait_size,
                portrait_size,
                wgpu::TextureFormat::Rgba8Unorm,
            );

            render_player_batch_to_target(
                &renderer,
                &batch,
                &texture.view,
                &depth_texture.view,
                &camera,
            );

            portrait_state
                .textures
                .insert(cred.id.clone(), texture.texture);
        }
    }

    batch.clear_and_unload(&mut player_store.store);
    portrait_state.version += 1;
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
                PlayerSpriteKey {
                    gender,
                    slot: sprite.slot,
                    sprite_id: sprite.id,
                },
                sprite.color,
            ));
        }
    }
    sprites
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
        Option<&TargetingHover>,
    )>,
    mut store_state: ResMut<PlayerAssetStoreState>,
    batch_state: Res<PlayerBatchState>,
) {
    let mut sprites_to_add = Vec::new();
    for (sprite_entity, child_of, sprite) in added_sprites.iter() {
        if let Ok((position, direction, player, entity_id, targeting_hover)) =
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
                targeting_hover,
            ));
        }
    }

    for (
        sprite_entity,
        _child_of,
        sprite,
        position,
        direction,
        player,
        entity_id,
        targeting_hover,
    ) in sprites_to_add
    {
        let gender = if player.is_male {
            Gender::Male
        } else {
            Gender::Female
        };

        let tint = targeting_hover.map(|t| t.tint).unwrap_or(Vec3::ZERO);
        let result = batch_state.batch.add_player_sprite(
            &shared_state.queue,
            &mut store_state.store,
            &game_files.inner().archive(),
            PlayerSpriteKey {
                gender,
                slot: sprite.slot,
                sprite_id: sprite.id,
            },
            sprite.color,
            *direction as u8,
            position.x,
            position.y,
            entity_id.id,
            InstanceFlag::None,
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
        &Position,
        &Direction,
        Option<&Animation>,
        Option<&TargetingHover>,
        &Children,
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
    children_query: Query<(&PlayerSprite, &PlayerSpriteInstance)>,
) {
    let mut to_update = changed_query
        .iter()
        .collect::<std::collections::HashSet<_>>();
    for entity in removed_hovers.read() {
        to_update.insert(entity);
    }

    for entity in to_update {
        if let Ok((position, direction, animation, targeting_hover, children, _entity_id)) =
            parent_query.get(entity)
        {
            let (anim_type, frame_index) = match animation {
                Some(anim) if anim.mode == AnimationMode::Finished => (EpfAnimationType::Idle, 0),
                Some(anim) => match anim.anim_type {
                    AnimationType::Player(at) => (at, anim.current_frame),
                    _ => (EpfAnimationType::Idle, 0),
                },
                None => (EpfAnimationType::Idle, 0),
            };

            let tint = targeting_hover.map(|t| t.tint).unwrap_or(Vec3::ZERO);

            for child_entity in children.iter() {
                if let Ok((sprite, sprite_instance)) = children_query.get(child_entity) {
                    let target = match (anim_type.is_emote(), sprite.slot) {
                        (true, PlayerPieceType::Emote) => Some((anim_type, frame_index)), // Active emote layer
                        (true, PlayerPieceType::Face) => None, // Hide face when emoting (face usually in emote)
                        (true, _) => Some((EpfAnimationType::Idle, 0)), // Base layers go to Idle
                        (false, PlayerPieceType::Emote) => None, // Hide emote layer when not emoting
                        (false, _) => Some((anim_type, frame_index)), // Standard animation
                    };

                    if let Some((at, fi)) = target {
                        if let Err(e) = batch_state.batch.update_player_sprite_with_animation(
                            &shared_state.queue,
                            &store_state.store,
                            &sprite_instance.handle,
                            *direction as u8,
                            position.x,
                            position.y,
                            sprite.color,
                            at,
                            fi,
                            InstanceFlag::None,
                            tint,
                        ) {
                            if at.is_emote() {
                                // If emote fails (e.g. facing away), just hide the emote layer
                                let _ = batch_state.batch.hide_player_sprite(
                                    &shared_state.queue,
                                    &sprite_instance.handle,
                                );
                            } else if !anim_type.is_emote() {
                                tracing::error!(
                                    "update_player_sprite_with_animation failed: {:?}",
                                    e
                                );
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
    local_player_query: Query<(&Player, &Children), With<LocalPlayer>>,
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
        if let Some((_, children)) = local_player_query.iter().next() {
            for child in children.iter() {
                if sprite_changed_query.get(child).is_ok() {
                    needs_update = true;
                    break;
                }
            }
        }
    }

    if needs_update {
        if let Some((player, children)) = local_player_query.iter().next() {
            portrait_state
                .batch
                .clear_and_unload(&mut player_store.store);

            let sprites = collect_player_sprites(player, children, &sprite_query);
            for (key, color) in sprites {
                let _ = portrait_state.batch.add_player_sprite(
                    &renderer.queue,
                    &mut player_store.store,
                    &game_files.inner().archive(),
                    key,
                    color,
                    1, // "Towards" direction
                    0.0,
                    0.0,
                    0, // No stacking for portrait
                    rendering::instance::InstanceFlag::None,
                    glam::Vec3::ZERO,
                );
            }

            // Perform the render pass immediately
            render_player_batch_to_target(
                &renderer,
                &portrait_state.batch,
                &portrait_state.view,
                &portrait_state.depth_texture.view,
                &portrait_state.camera,
            );

            portrait_state.dirty = false;
            portrait_state.version += 1;
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
    local_player_query: Query<(&Player, &Children), With<LocalPlayer>>,
    other_players_query: Query<(&Player, &Children, &EntityId), Without<LocalPlayer>>,
    sprite_query: Query<&PlayerSprite>,
    mut portrait_state: ResMut<crate::resources::ProfilePortraitState>,
    mut player_store: ResMut<PlayerAssetStoreState>,
    mut last_entity_id: Local<Option<u32>>,
) {
    let mut target_entity = None;

    if let Some(eid) = profile_state.entity_id {
        // Find other player by server ID
        for (player, children, id) in other_players_query.iter() {
            if id.id == eid {
                target_entity = Some((player, children));
                break;
            }
        }
    } else {
        // Use local player
        if let Some((player, children)) = local_player_query.iter().next() {
            target_entity = Some((player, children));
        }
    }

    // Force update if the entity we're looking at changed
    if profile_state.entity_id != *last_entity_id {
        portrait_state.dirty = true;
        *last_entity_id = profile_state.entity_id;
    }

    if portrait_state.dirty {
        if let Some((player, children)) = target_entity {
            portrait_state
                .batch
                .clear_and_unload(&mut player_store.store);

            let sprites = collect_player_sprites(player, children, &sprite_query);
            for (key, color) in sprites {
                let _ = portrait_state.batch.add_player_sprite(
                    &renderer.queue,
                    &mut player_store.store,
                    &game_files.inner().archive(),
                    key,
                    color,
                    1, // "Towards" direction
                    0.0,
                    0.0,
                    0, // No stacking for portrait
                    rendering::instance::InstanceFlag::None,
                    glam::Vec3::ZERO,
                );
            }

            // Perform the render pass immediately
            render_player_batch_to_target(
                &renderer,
                &portrait_state.batch,
                &portrait_state.view,
                &portrait_state.depth_texture.view,
                &portrait_state.camera,
            );

            portrait_state.dirty = false;
            portrait_state.version += 1;
        }
    }
}
