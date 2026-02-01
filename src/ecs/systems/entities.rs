//! Entity spawning and despawning systems

use super::super::animation::{Animation, AnimationBundle, AnimationMode, AnimationType};
use super::super::components::*;
use crate::{
    RendererState,
    events::{EntityEvent, SessionEvent},
    game_files::GameFiles,
};
use bevy::prelude::*;
use formats::mpf::MpfAnimationType;
use glam::Vec2;
use packets::server::display_player::DisplayArgs;
use rendering::scene::players::PlayerPieceType;
use wgpu::naga::FastHashSet;

/// Local state for tracking the local player ID
#[derive(Default)]
pub struct PlayerId {
    id: Option<u32>,
}

/// Removes duplicate entities when a new entity with the same ID is added.
/// This handles reconnection scenarios where stale entities might remain.
pub fn dedupe_entities_by_id(
    mut commands: Commands,
    new_entities_query: Query<(Entity, &EntityId), Added<EntityId>>,
    all_entities_query: Query<(Entity, &EntityId)>,
) {
    for (entity, id) in new_entities_query.iter() {
        for (other_entity, other_id) in all_entities_query.iter() {
            if other_id.id == id.id && other_entity != entity {
                commands.entity(other_entity).despawn();
            }
        }
    }
}

/// Spawns entities from network events (players, NPCs, items).
pub fn spawn_entities_system(
    mut commands: Commands,
    mut entity_events: MessageReader<EntityEvent>,
    mut session_events: MessageReader<SessionEvent>,
    mut local_id: Local<PlayerId>,
    existing_players: Query<(Entity, &EntityId, &Position), (With<Player>, Without<LocalPlayer>)>,
    entity_query: Query<(Entity, &EntityId)>,
    mut settings: ResMut<crate::settings::Settings>,
    current_session: Option<Res<crate::CurrentSession>>,
    mut show_profile: MessageWriter<crate::slint_plugin::ShowSelfProfileEvent>,
    mut tile_counters: ResMut<crate::resources::ItemTileCounters>,
) {
    // Handle session events first to set local player ID
    for event in session_events.read() {
        match event {
            SessionEvent::PlayerId(id) => {
                local_id.id = Some(*id);
                // Retroactively mark existing entity as local player
                for (entity, ent_id, _pos) in existing_players.iter() {
                    if ent_id.id == *id {
                        commands
                            .entity(entity)
                            .insert((LocalPlayer, CameraTarget, UnconfirmedWalks::default()));
                        break;
                    }
                }
            }
            SessionEvent::WorldMap(_)
            | SessionEvent::DisplayMenu(_)
            | SessionEvent::DisplayDialog(_)
            | SessionEvent::SelfProfile(_)
            | SessionEvent::OtherProfile(_)
            | SessionEvent::WorldList(_) => {}
        }
    }

    // Two-pass: first find which event index is the "latest" for each entity ID,
    // then process in forward order so spawn_order reflects arrival time
    let events: Vec<_> = entity_events.read().collect();

    // Build map of entity_id -> last event index that contains it
    let mut latest_event_for_id: FastHashSet<(u32, usize)> = FastHashSet::default();
    for (event_idx, event) in events.iter().enumerate() {
        match event {
            EntityEvent::DisplayEntities(entities) => {
                for info in &entities.entities {
                    let id = match info {
                        packets::server::EntityInfo::Item { id, .. } => *id,
                        packets::server::EntityInfo::Creature { id, .. } => *id,
                    };
                    latest_event_for_id.retain(|(eid, _)| *eid != id);
                    latest_event_for_id.insert((id, event_idx));
                }
            }
            EntityEvent::DisplayPlayer(player) => {
                latest_event_for_id.retain(|(eid, _)| *eid != player.id);
                latest_event_for_id.insert((player.id, event_idx));
            }
            EntityEvent::Remove(remove) => {
                latest_event_for_id.retain(|(eid, _)| *eid != remove.source_id);
                latest_event_for_id.insert((remove.source_id, event_idx));
            }
            _ => {}
        }
    }

    // Process in forward order, only spawning from the latest event for each ID
    for (event_idx, event) in events.iter().enumerate() {
        match event {
            EntityEvent::DisplayEntities(entities) => {
                spawn_display_entities(
                    &mut commands,
                    &entities.entities,
                    event_idx,
                    &latest_event_for_id,
                    &mut tile_counters,
                );
            }
            EntityEvent::DisplayPlayer(player) => {
                // Skip if this isn't the latest event for this player ID
                if !latest_event_for_id.contains(&(player.id, event_idx)) {
                    continue;
                }
                let is_local = local_id.id.map(|id| id == player.id).unwrap_or(false);
                spawn_display_player(
                    &mut commands,
                    player,
                    local_id.id,
                    &mut settings,
                    current_session.as_deref(),
                );
                if is_local {
                    show_profile.write(crate::slint_plugin::ShowSelfProfileEvent::SelfUpdate);
                }
            }
            EntityEvent::Remove(remove) => {
                // Skip if this isn't the latest event for this ID
                if !latest_event_for_id.contains(&(remove.source_id, event_idx)) {
                    continue;
                }

                for (entity, entity_id) in entity_query.iter() {
                    if entity_id.id == remove.source_id {
                        commands.entity(entity).despawn();
                        break;
                    }
                }
            }
            _ => {}
        }
    }
}

fn spawn_display_entities(
    commands: &mut Commands,
    entities: &[packets::server::EntityInfo],
    event_idx: usize,
    latest_event_for_id: &FastHashSet<(u32, usize)>,
    tile_counters: &mut crate::resources::ItemTileCounters,
) {
    for entity_info in entities {
        let target_id = match entity_info {
            packets::server::EntityInfo::Item { id, .. } => *id,
            packets::server::EntityInfo::Creature { id, .. } => *id,
        };

        // Skip if this isn't the latest event for this entity ID
        if !latest_event_for_id.contains(&(target_id, event_idx)) {
            continue;
        }

        match entity_info {
            packets::server::EntityInfo::Item {
                x,
                y,
                id,
                sprite,
                color,
            } => {
                let spawn_order = tile_counters.next_order(*x, *y);
                commands.spawn((
                    ItemBundle {
                        entity_id: EntityId { id: *id },
                        position: Position {
                            x: *x as f32,
                            y: *y as f32,
                        },
                        sprite: ItemSprite {
                            id: *sprite,
                            color: *color,
                            spawn_order,
                        },
                    },
                    InGameScoped,
                    MapScoped,
                    Hitbox::default(),
                ));
            }
            packets::server::EntityInfo::Creature {
                x,
                y,
                id,
                sprite,
                direction,
                entity_type,
                name,
            } => {
                commands.spawn((
                    NPCBundle {
                        entity_id: EntityId { id: *id },
                        npc: NPC {
                            name: name.clone().unwrap_or_default(),
                            entity_type: entity_type.clone(),
                        },
                        sprite: CreatureSprite { id: *sprite },
                        direction: Direction::from(*direction),
                        position: Position {
                            x: *x as f32,
                            y: *y as f32,
                        },
                    },
                    InGameScoped,
                    MapScoped,
                    Hitbox::screen_space(Vec2::new(-0.45, -1.25), Vec2::new(0.45, 0.65)),
                    HoverName {
                        name: name.clone().unwrap_or_default(),
                        color: glam::Vec4::new(0.7, 0.7, 1.0, 1.0),
                    },
                ));
            }
        }
    }
}

fn spawn_display_player(
    commands: &mut Commands,
    player: &packets::server::display_player::DisplayPlayer,
    local_id: Option<u32>,
    settings: &mut ResMut<crate::settings::Settings>,
    current_session: Option<&crate::CurrentSession>,
) {
    let is_male = match &player.args {
        DisplayArgs::Normal { is_male, .. } => *is_male,
        DisplayArgs::Dead { is_male, .. } => *is_male,
        _ => true,
    };

    let mut player_entity = commands.spawn((
        PlayerBundle {
            player: Player {
                name: player.name.clone(),
                is_male,
            },
            position: Position {
                x: player.x as f32,
                y: player.y as f32,
            },
            direction: Direction::from(player.direction),
            entity_id: EntityId { id: player.id },
        },
        InGameScoped,
        MapScoped,
        Hitbox::screen_space(Vec2::new(-0.25, -1.25), Vec2::new(0.25, 0.65)),
    ));

    let is_local = Some(player.id) == local_id;
    if is_local {
        player_entity.insert((LocalPlayer, CameraTarget, UnconfirmedWalks::default()));
    } else {
        player_entity.insert(HoverName::new(player.name.clone()));
    }

    match &player.args {
        DisplayArgs::Normal {
            head_sprite,
            body_sprite: body_sprite_raw,
            pants_color,
            armor_sprite1,
            boots_sprite,
            armor_sprite2,
            shield_sprite,
            weapon_sprite,
            head_color,
            boots_color,
            accessory_color1,
            accessory_sprite1,
            accessory_color2,
            accessory_sprite2,
            accessory_color3,
            accessory_sprite3,
            overcoat_sprite,
            overcoat_color,
            body_color,
            face_sprite,
            is_male: _, // handled above
            ..
        } => {
            if is_local {
                // Update character preview in settings
                if let Some(session) = current_session {
                    let body_id = if is_male {
                        (body_sprite_raw / 16 + 1) / 2
                    } else {
                        (body_sprite_raw / 16) / 2
                    };

                    let preview = crate::settings::CharacterPreview {
                        is_male,
                        body: body_id as u16,
                        helmet: *head_sprite,
                        helmet_color: *head_color as u32,
                        boots: *boots_sprite as u16,
                        boots_color: *boots_color as u32,
                        armor: *armor_sprite1,
                        pants_color: *pants_color as u32,
                        shield: *shield_sprite as u16,
                        shield_color: *body_color as u32,
                        weapon: *weapon_sprite,
                        weapon_color: 0,
                        accessory1: *accessory_sprite1,
                        accessory1_color: *accessory_color1 as u32,
                        overcoat: *overcoat_sprite,
                        overcoat_color: *overcoat_color as u32,
                    };
                    settings.update_character_preview(
                        &session.server_url,
                        &session.username,
                        preview,
                    );
                }
            }

            spawn_player_sprites(
                &mut player_entity,
                *head_sprite,
                *head_color,
                *pants_color,
                *body_color,
                *armor_sprite1,
                *boots_sprite,
                *armor_sprite2,
                *shield_sprite,
                *weapon_sprite,
                *boots_color,
                *accessory_color1,
                *accessory_sprite1,
                *accessory_color2,
                *accessory_sprite2,
                *accessory_color3,
                *accessory_sprite3,
                *overcoat_sprite,
                *overcoat_color,
                *face_sprite,
            );
        }
        DisplayArgs::Sprite {
            sprite,
            head_color,
            boots_color: _,
        } => {
            player_entity.with_children(|parent| {
                parent.spawn(PlayerSprite {
                    id: *sprite,
                    slot: PlayerPieceType::Body,
                    color: *head_color,
                });
            });
        }
        DisplayArgs::Dead {
            head_sprite,
            body_sprite,
            is_transparent: _,
            face_sprite: _,
            is_male: _,
        } => {
            player_entity.with_children(|parent| {
                parent.spawn(PlayerSprite {
                    id: *body_sprite as u16,
                    slot: PlayerPieceType::Body,
                    color: 0,
                });
                parent.spawn(PlayerSprite {
                    id: *head_sprite,
                    slot: PlayerPieceType::HelmetFg,
                    color: 0,
                });
            });
        }
        DisplayArgs::Hidden => {}
    }
}

/// Helper to attach player equipment sprites as children
fn spawn_player_sprites(
    player_entity: &mut EntityCommands,
    head_sprite: u16,
    head_color: u8,
    pants_color: u8,
    body_color: u8,
    armor_sprite1: u16,
    boots_sprite: u8,
    armor_sprite2: u16,
    shield_sprite: u8,
    weapon_sprite: u16,
    boots_color: u8,
    accessory_color1: u8,
    accessory_sprite1: u16,
    accessory_color2: u8,
    accessory_sprite2: u16,
    accessory_color3: u8,
    accessory_sprite3: u16,
    overcoat_sprite: u16,
    overcoat_color: u8,
    face_sprite: u8,
) {
    player_entity.with_children(|parent| {
        parent.spawn(PlayerSprite {
            id: 1,
            slot: PlayerPieceType::Body,
            color: body_color,
        });

        if face_sprite > 0 {
            parent.spawn(PlayerSprite {
                id: face_sprite as u16,
                slot: PlayerPieceType::Face,
                color: head_color,
            });
        }

        parent.spawn(PlayerSprite {
            id: 1,
            slot: PlayerPieceType::Emote,
            color: 0,
        });

        if pants_color > 0 {
            parent.spawn(PlayerSprite {
                id: pants_color as u16,
                slot: PlayerPieceType::Pants,
                color: 1,
            });
        }
        parent.spawn(PlayerSprite {
            id: head_sprite,
            slot: PlayerPieceType::HelmetBg,
            color: head_color,
        });
        parent.spawn(PlayerSprite {
            id: head_sprite,
            slot: PlayerPieceType::HelmetFg,
            color: head_color,
        });
        if weapon_sprite > 0 {
            parent.spawn(PlayerSprite {
                id: weapon_sprite,
                slot: PlayerPieceType::Weapon,
                color: 0,
            });
        }
        if shield_sprite > 0 && shield_sprite != 255 {
            parent.spawn(PlayerSprite {
                id: shield_sprite as u16,
                slot: PlayerPieceType::Shield,
                color: 0,
            });
        }
        if overcoat_sprite > 0 {
            parent.spawn(PlayerSprite {
                id: overcoat_sprite,
                slot: PlayerPieceType::Armor,
                color: overcoat_color,
            });
        } else {
            parent.spawn(PlayerSprite {
                id: armor_sprite1,
                slot: PlayerPieceType::Arms,
                color: 0,
            });
            parent.spawn(PlayerSprite {
                id: armor_sprite2,
                slot: PlayerPieceType::Armor,
                color: 0,
            });
        }
        if boots_sprite > 0 {
            parent.spawn(PlayerSprite {
                id: boots_sprite as u16,
                slot: PlayerPieceType::Boots,
                color: boots_color,
            });
        }
        if accessory_sprite1 > 0 {
            parent.spawn(PlayerSprite {
                id: accessory_sprite1 as u16,
                slot: PlayerPieceType::Accessory1Bg,
                color: accessory_color1,
            });
            parent.spawn(PlayerSprite {
                id: accessory_sprite1 as u16,
                slot: PlayerPieceType::Accessory1Fg,
                color: accessory_color1,
            });
        }
        if accessory_sprite2 > 0 {
            parent.spawn(PlayerSprite {
                id: accessory_sprite2 as u16,
                slot: PlayerPieceType::Accessory2Bg,
                color: accessory_color2,
            });
            parent.spawn(PlayerSprite {
                id: accessory_sprite2 as u16,
                slot: PlayerPieceType::Accessory2Fg,
                color: accessory_color2,
            });
        }
        if accessory_sprite3 > 0 {
            parent.spawn(PlayerSprite {
                id: accessory_sprite3 as u16,
                slot: PlayerPieceType::Accessory3Bg,
                color: accessory_color3,
            });
            parent.spawn(PlayerSprite {
                id: accessory_sprite3 as u16,
                slot: PlayerPieceType::Accessory3Fg,
                color: accessory_color3,
            });
        }
    });
}

/// Marks newly added creatures for async loading onto the GPU renderer.
pub fn queue_creatures_for_loading(
    mut commands: Commands,
    added_creatures: Query<(Entity, &Position, &CreatureSprite, &Direction), Added<CreatureSprite>>,
) {
    for (entity, _position, _sprite, _direction) in added_creatures.iter() {
        commands.entity(entity).insert(CreatureLoadRequested);
    }
}

/// Loads creature sprites from game files onto the GPU.
/// Processes a limited batch per frame to avoid stalls.
pub fn creature_load_system(
    mut commands: Commands,
    shared_state: Res<RendererState>,
    game_files: Res<GameFiles>,
    mut creatures_store: ResMut<crate::CreatureAssetStoreState>,
    mut creatures_batch: ResMut<crate::CreatureBatchState>,
    mut to_load: Query<
        (Entity, &Position, &CreatureSprite, &Direction),
        With<CreatureLoadRequested>,
    >,
) {
    const MAX_LOADS_PER_FRAME: usize = 8;

    let mut processed = 0usize;
    for (entity, position, sprite, direction) in to_load.iter_mut() {
        if processed >= MAX_LOADS_PER_FRAME {
            break;
        }

        let result = creatures_batch.batch.add_creature(
            &shared_state.queue,
            &mut creatures_store.store,
            &game_files.inner().archive(),
            sprite.id,
            *direction as u8,
            position.x,
            position.y,
        );

        match result {
            Ok(result) => {
                let mut idle_animation =
                    if let Some(standing_anim) = result.get_animation(MpfAnimationType::Standing) {
                        Some(Animation::new(
                            AnimationMode::LoopStandard,
                            AnimationType::Creature(MpfAnimationType::Standing),
                            0.5,
                            standing_anim.frame_count as usize,
                        ))
                    } else {
                        None
                    };

                idle_animation = idle_animation.map(|anim| {
                    result.animations.iter().fold(anim, |acc, anim_info| {
                        match anim_info.animation_type {
                            MpfAnimationType::Extra(ratio) => Animation::new(
                                AnimationMode::LoopExtra {
                                    ratio: ratio as f32 / 100.0,
                                    standard_end: acc.end_index,
                                    extra_end: anim_info.frame_count as usize - 1,
                                },
                                AnimationType::Creature(MpfAnimationType::Extra(ratio)),
                                0.5,
                                anim_info.frame_count as usize,
                            ),
                            _ => acc,
                        }
                    })
                });

                if let Some(extra_anim) = idle_animation {
                    commands
                        .entity(entity)
                        .insert(AnimationBundle::from_animation(extra_anim));
                }

                commands
                    .entity(entity)
                    .insert(CreatureInstance { instance: result })
                    .remove::<CreatureLoadRequested>();
                processed += 1;
            }
            Err(e) => {
                tracing::error!("Failed to load creature sprite ID {}: {:?}", sprite.id, e);
                break;
            }
        }
    }
}

/// Updates or adds health bars from network events.
pub fn health_bar_system(
    mut commands: Commands,
    mut entity_events: MessageReader<EntityEvent>,
    mut health_bars: Query<(Entity, &EntityId, Option<&mut HealthBar>)>,
    mut audio_events: MessageWriter<crate::events::AudioEvent>,
) {
    for event in entity_events.read() {
        if let EntityEvent::HealthBar(packet) = event {
            // Find entity with matching ID
            for (entity, ent_id, health_bar) in health_bars.iter_mut() {
                if ent_id.id == packet.source_id {
                    if let Some(mut bar) = health_bar {
                        bar.percent = packet.health_percent;
                        bar.timer = Timer::from_seconds(5.0, TimerMode::Once);
                    } else {
                        commands.entity(entity).insert(HealthBar {
                            percent: packet.health_percent,
                            timer: Timer::from_seconds(5.0, TimerMode::Once),
                        });
                    }

                    if let Some(sound_id) = packet.sound {
                        audio_events.write(crate::events::AudioEvent::PlaySound(
                            packets::server::Sound::Sound(sound_id),
                        ));
                    }
                    break;
                }
            }
        }
    }
}

/// Removes expired health bars.
pub fn expire_health_bars(
    mut commands: Commands,
    time: Res<Time>,
    mut health_bars: Query<(Entity, &mut HealthBar)>,
) {
    for (entity, mut bar) in health_bars.iter_mut() {
        bar.timer.tick(time.delta());
        if bar.timer.is_finished() {
            commands.entity(entity).remove::<HealthBar>();
        }
    }
}
