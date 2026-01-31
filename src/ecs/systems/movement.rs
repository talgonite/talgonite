//! Movement and physics systems

use super::super::animation::{AnimationBundle, AnimationMode, AnimationType};
use super::super::components::*;
use crate::{
    ecs::collision::{MapCollisionData, WallCollisionTable},
    events::{AudioEvent, EntityEvent, PlayerAction},
};
use bevy::prelude::*;
use formats::{epf::EpfAnimationType, mpf::MpfAnimationType};
use packets::{
    client,
    server::{ClientWalkResponseArgs, Sound},
    types::BodyAnimationKind,
};

/// Handles local player movement from input events.
/// Performs collision detection against walls and other entities.
pub fn player_movement_system(
    mut player_actions: MessageReader<PlayerAction>,
    map_query: Query<&GameMap>,
    mut player_query: Query<
        (
            Entity,
            &mut Position,
            &mut Direction,
            &mut UnconfirmedWalks,
            Option<&MovementTween>,
        ),
        With<LocalPlayer>,
    >,
    entity_positions: Query<&Position, (Or<(With<NPC>, With<Player>)>, Without<LocalPlayer>)>,
    mut commands: Commands,
    collision_table: Option<Res<WallCollisionTable>>,
    map_collision: Option<Res<MapCollisionData>>,
    outbox: Option<Res<crate::network::PacketOutbox>>,
) {
    let Ok((entity, mut position, mut facing, mut unconfirmed, tween)) = player_query.single_mut()
    else {
        return;
    };
    let Ok(map) = map_query.single() else {
        return;
    };

    // Handle walk requests from input
    for event in player_actions.read() {
        match event {
            PlayerAction::Walk {
                direction,
                source: _,
            } => {
                if handle_walk_request(
                    entity,
                    *direction,
                    map,
                    &mut position,
                    &mut facing,
                    tween,
                    &entity_positions,
                    &mut commands,
                    collision_table.as_deref(),
                    map_collision.as_deref(),
                ) {
                    unconfirmed.0.push_back(*direction);

                    if let Some(outbox) = &outbox {
                        outbox.send(&client::ClientWalk {
                            direction: (*direction).into(),
                            step_count: 1,
                        });
                    }
                }
            }
            PlayerAction::Turn {
                direction,
                source: _,
            } => {
                if let Some(outbox) = &outbox {
                    outbox.send(&client::Turn {
                        direction: (*direction).into(),
                    });
                }
            }
        }
    }
}

fn handle_walk_request(
    entity: Entity,
    direction: Direction,
    map: &GameMap,
    position: &mut Position,
    facing: &mut Direction,
    tween: Option<&MovementTween>,
    entity_positions: &Query<&Position, (Or<(With<NPC>, With<Player>)>, Without<LocalPlayer>)>,
    commands: &mut Commands,
    collision_table: Option<&WallCollisionTable>,
    map_collision: Option<&MapCollisionData>,
) -> bool {
    let (dx, dy) = direction.delta();

    let new_dir = Direction::from(direction);
    if *facing != new_dir {
        *facing = new_dir;
    }

    let (start_x, start_y) = if let Some(tween) = tween {
        (tween.end_x, tween.end_y)
    } else {
        (position.x, position.y)
    };

    let target_x = (start_x as i16 + dx).max(0).min(map.width as i16 - 1) as f32;
    let target_y = (start_y as i16 + dy).max(0).min(map.height as i16 - 1) as f32;

    // Check wall collision at target tile
    let target_x_int = target_x as u8;
    let target_y_int = target_y as u8;

    if !crate::ecs::collision::can_walk_to(
        target_x_int,
        target_y_int,
        collision_table,
        map_collision,
    ) {
        return false;
    }

    // Check entity collision (other players and NPCs)
    let is_tile_occupied = entity_positions
        .iter()
        .any(|pos| (pos.x - target_x).abs() < 0.5 && (pos.y - target_y).abs() < 0.5);
    if is_tile_occupied {
        return false;
    }

    // Skip creating a zero-length tween if clamped (edge of map / no movement)
    if (target_x - start_x).abs() < f32::EPSILON && (target_y - start_y).abs() < f32::EPSILON {
        return false;
    }

    commands.entity(entity).insert((
        AnimationBundle::new(
            AnimationMode::OneShot,
            AnimationType::Player(EpfAnimationType::Walk),
            0.10,
            5,
        ),
        MovementTween {
            start_x,
            start_y,
            end_x: target_x,
            end_y: target_y,
            elapsed: 0.0,
            duration: 0.5,
        },
    ));

    true
}

/// Handles creature movement events from the server.
pub fn entity_motion_system(
    mut commands: Commands,
    mut entity_events: MessageReader<EntityEvent>,
    mut moved_query: Query<
        (
            Entity,
            &mut Direction,
            &EntityId,
            Option<&CreatureInstance>,
            Option<&Player>,
        ),
        Without<LocalPlayer>,
    >,
) {
    for event in entity_events.read() {
        match event {
            EntityEvent::Walk(evt) => {
                let mut found = false;
                for (entity, mut direction, entity_id, instance, player) in moved_query.iter_mut() {
                    if entity_id.id != evt.source_id {
                        continue;
                    }

                    let (dx, dy) = match evt.direction {
                        0 => (0, -1),
                        1 => (1, 0),
                        2 => (0, 1),
                        3 => (-1, 0),
                        _ => {
                            tracing::warn!("Invalid direction {} in walk event", evt.direction);
                            continue;
                        }
                    };

                    let new_dir = Direction::from(evt.direction);
                    if *direction != new_dir {
                        *direction = new_dir;
                    }

                    commands.entity(entity).insert(MovementTween {
                        start_x: evt.old_point.0 as f32,
                        start_y: evt.old_point.1 as f32,
                        end_x: (evt.old_point.0 as i32 + dx) as f32,
                        end_y: (evt.old_point.1 as i32 + dy) as f32,
                        elapsed: 0.0,
                        duration: 0.5,
                    });

                    if let Some(instance) = instance {
                        if let Some(walk) = instance.instance.get_animation(MpfAnimationType::Walk)
                        {
                            if let Some(standing) =
                                instance.instance.get_animation(MpfAnimationType::Standing)
                            {
                                commands.entity(entity).insert(AnimationBundle::new(
                                    AnimationMode::OneShotThenLoop {
                                        loop_anim: AnimationType::Creature(
                                            MpfAnimationType::Standing,
                                        ),
                                        loop_frame_count: standing.frame_count as usize,
                                        loop_frame_duration: 0.5,
                                    },
                                    AnimationType::Creature(MpfAnimationType::Walk),
                                    0.125,
                                    walk.frame_count as usize,
                                ));
                            }
                        }
                    } else if let Some(_player) = player {
                        commands.entity(entity).insert(AnimationBundle::new(
                            AnimationMode::OneShot,
                            AnimationType::Player(EpfAnimationType::Walk),
                            0.10,
                            5,
                        ));
                    }

                    found = true;
                    break;
                }

                if !found {
                    tracing::warn!(
                        "EntityEvent::Walk: No entity found with source_id {} (target: {:?}, dir: {})",
                        evt.source_id,
                        evt.old_point,
                        evt.direction
                    );
                }
            }
            EntityEvent::Turn(turn) => {
                for (_, mut direction, entity_id, _, _) in moved_query.iter_mut() {
                    if entity_id.id == turn.source_id {
                        let new_dir = Direction::from(turn.direction);
                        if *direction != new_dir {
                            *direction = new_dir;
                        }
                        break;
                    }
                }
            }
            _ => {}
        }
    }
}

/// Handles body animation events (attacks, spells, etc.)
pub fn player_animation_start_system(
    mut entity_events: MessageReader<EntityEvent>,
    mut players: Query<(Entity, &EntityId), With<Player>>,
    children: Query<&Children>,
    player_sprites: Query<(
        &super::super::components::PlayerSprite,
        &super::super::components::PlayerSpriteInstance,
    )>,
    player_batch: Option<Res<crate::PlayerBatchState>>,
    player_store: Option<Res<crate::PlayerAssetStoreState>>,
    mut npcs: Query<(Entity, &EntityId, &CreatureInstance)>,
    mut audio_events: MessageWriter<AudioEvent>,
    mut commands: Commands,
) {
    use rendering::scene::players::PlayerPieceType;

    for event in entity_events.read() {
        let EntityEvent::Animate(anim) = event else {
            continue;
        };

        if let Some(sound) = anim.sound {
            audio_events.write(AudioEvent::PlaySound(Sound::Sound(sound)));
        }

        // Handle player animations
        for (entity, entity_id) in players.iter_mut() {
            if entity_id.id != anim.source_id {
                continue;
            }

            let (anim_type, frame_count) = match anim.kind {
                BodyAnimationKind::Assail => (EpfAnimationType::Attack, 2),
                BodyAnimationKind::Punch => (EpfAnimationType::PunchAttack, 2),
                BodyAnimationKind::Kick => (EpfAnimationType::KickAttack, 3),
                BodyAnimationKind::PriestCast => (EpfAnimationType::SpellChant, 3),
                BodyAnimationKind::HandsUp => (EpfAnimationType::ArmsUpChant, 1),
                BodyAnimationKind::RoundHouseKick => (EpfAnimationType::LongKickAttack, 4),
                BodyAnimationKind::Stab => (EpfAnimationType::StabAttack, 2),
                BodyAnimationKind::DoubleStab => (EpfAnimationType::DoubleStabAttack, 2),
                _ => {
                    tracing::info!("Unhandled BodyAnimationKind: {:?}", anim.kind);
                    continue;
                }
            };

            if let (Some(pb), Some(ps), Ok(child_entities)) = (
                player_batch.as_ref(),
                player_store.as_ref(),
                children.get(entity),
            ) {
                let mut armor_supports = true;

                for child_entity in child_entities.iter() {
                    if let Ok((sprite, sprite_instance)) = player_sprites.get(child_entity) {
                        if sprite.slot == PlayerPieceType::Armor {
                            armor_supports = pb.batch.supports_animation(
                                &ps.store,
                                &sprite_instance.handle,
                                anim_type,
                            );
                            break;
                        }
                    }
                }

                if !armor_supports {
                    tracing::debug!(
                        "Skipping animation {:?} for player ID {} - armor does not support it",
                        anim_type,
                        entity_id.id
                    );
                    continue;
                }
            }

            tracing::info!(
                "Starting animation {:?} for player ID {}",
                anim,
                entity_id.id
            );
            commands.entity(entity).insert(AnimationBundle::new(
                AnimationMode::OneShot,
                AnimationType::Player(anim_type),
                anim.animation_speed as f32 / 100.,
                frame_count,
            ));
        }

        // Handle NPC animations
        for (entity, entity_id, instance) in npcs.iter_mut() {
            if entity_id.id != anim.source_id {
                continue;
            }

            let mpf_anim = match anim.kind {
                BodyAnimationKind::Assail => {
                    instance.instance.get_animation(MpfAnimationType::Attack)
                }
                _ => None,
            };

            let (anim_type, frame_count) = match mpf_anim {
                Some(a) => (MpfAnimationType::Attack, a.frame_count as usize),
                None => {
                    tracing::info!("Unhandled BodyAnimationKind for NPC: {:?}", anim.kind);
                    continue;
                }
            };

            let idle_anim = instance.instance.get_animation(MpfAnimationType::Standing);
            tracing::info!(
                "Starting animation {:?} for NPC ID {}",
                anim_type,
                entity_id.id
            );
            commands.entity(entity).insert(AnimationBundle::new(
                match idle_anim {
                    Some(idle) => AnimationMode::OneShotThenLoop {
                        loop_anim: AnimationType::Creature(MpfAnimationType::Standing),
                        loop_frame_count: idle.frame_count as usize,
                        loop_frame_duration: 0.5,
                    },
                    _ => AnimationMode::OneShot,
                },
                AnimationType::Creature(anim_type),
                5.0 / anim.animation_speed as f32,
                frame_count,
            ));
        }
    }
}

/// Advances movement tweens using linear interpolation.
/// Removes the tween component when complete.
pub fn movement_tween_system(
    time: Res<Time>,
    mut query: Query<(Entity, &mut Position, &mut MovementTween)>,
    mut commands: Commands,
) {
    for (entity, mut pos, mut tween) in query.iter_mut() {
        tween.elapsed += time.delta().as_secs_f32();
        let raw_t = (tween.elapsed / tween.duration).clamp(0.0, 1.0);

        // Linear interpolation for constant walking speed
        let t = raw_t;

        pos.x = tween.start_x + (tween.end_x - tween.start_x) * t;
        pos.y = tween.start_y + (tween.end_y - tween.start_y) * t;

        if tween.elapsed >= tween.duration {
            // Land exactly on the intended tile (eliminate FP drift)
            pos.x = tween.end_x;
            pos.y = tween.end_y;
            commands.entity(entity).remove::<MovementTween>();
        }
    }
}

/// Handles movement reconciliation for the local player.
/// Snaps back on rejection and replays unconfirmed steps.
pub fn player_reconciliation_system(
    mut entity_events: MessageReader<EntityEvent>,
    mut map_events: MessageReader<crate::events::MapEvent>,
    mut player_query: Query<
        (
            Entity,
            &mut Position,
            &mut UnconfirmedWalks,
            Option<&mut MovementTween>,
        ),
        With<LocalPlayer>,
    >,
    mut commands: Commands,
) {
    let Ok((entity, mut position, mut unconfirmed, mut active_tween)) = player_query.single_mut()
    else {
        return;
    };

    // Purge on map change
    for event in map_events.read() {
        if let crate::events::MapEvent::Clear = event {
            unconfirmed.0.clear();
        }
    }

    for event in entity_events.read() {
        match event {
            EntityEvent::PlayerLocation(location) => {
                position.x = location.x as f32;
                position.y = location.y as f32;
                unconfirmed.0.clear();
                commands.entity(entity).remove::<MovementTween>();
            }
            EntityEvent::PlayerWalkResponse(response) => {
                unconfirmed.0.pop_front();

                if let ClientWalkResponseArgs::Rejected = response.args {
                    // Snap to server's "from" position (state before the rejected step)
                    position.x = response.from.0 as f32;
                    position.y = response.from.1 as f32;

                    // Replay unconfirmed steps
                    let count = unconfirmed.0.len();
                    for (idx, &dir) in unconfirmed.0.iter().enumerate() {
                        let (dx, dy) = dir.delta();

                        if idx < count - 1 {
                            // Teleport for intermediate steps
                            position.x += dx as f32;
                            position.y += dy as f32;
                        } else if let Some(ref mut tween) = active_tween {
                            // Final step: update tween
                            tween.start_x = position.x;
                            tween.start_y = position.y;
                            tween.end_x = position.x + dx as f32;
                            tween.end_y = position.y + dy as f32;
                            // Position will be updated by movement_tween_system this frame
                        } else {
                            // Final step: snap
                            position.x += dx as f32;
                            position.y += dy as f32;
                        }
                    }

                    if unconfirmed.0.is_empty() {
                        commands.entity(entity).remove::<MovementTween>();
                    }
                }
            }
            _ => {}
        }
    }
}
