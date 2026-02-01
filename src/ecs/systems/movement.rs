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
                if let Some(start_pos) = handle_walk_request(
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
                    unconfirmed.pending.push_back(UnconfirmedStep {
                        direction: *direction,
                        expected_from: start_pos,
                    });

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
) -> Option<Vec2> {
    let delta = direction.vec2_delta();

    if *facing != direction {
        *facing = direction;
    }

    let start_pos = tween.map(|t| t.end).unwrap_or_else(|| position.to_vec2());

    let target_pos = (start_pos + delta).clamp(
        Vec2::ZERO,
        Vec2::new(map.width as f32 - 1.0, map.height as f32 - 1.0),
    );

    // Check wall collision at target tile
    let target_tile = target_pos.as_uvec2();

    if !crate::ecs::collision::can_walk_to(
        target_tile.x as u8,
        target_tile.y as u8,
        collision_table,
        map_collision,
    ) {
        return None;
    }

    // Check entity collision (other players and NPCs)
    let is_tile_occupied = entity_positions
        .iter()
        .any(|pos| pos.to_vec2().distance_squared(target_pos) < 0.25);
    if is_tile_occupied {
        return None;
    }

    // Skip creating a zero-length tween if clamped (edge of map / no movement)
    if start_pos.distance_squared(target_pos) < f32::EPSILON {
        return None;
    }

    commands.entity(entity).insert((
        AnimationBundle::new(
            AnimationMode::OneShot,
            AnimationType::Player(EpfAnimationType::Walk),
            0.10,
            5,
        ),
        MovementTween {
            start: start_pos,
            end: target_pos,
            elapsed: 0.0,
            duration: 0.5,
        },
    ));

    Some(start_pos)
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

                    let delta = Direction::from(evt.direction).vec2_delta();

                    let new_dir = Direction::from(evt.direction);
                    if *direction != new_dir {
                        *direction = new_dir;
                    }

                    let start_pos = Vec2::new(evt.old_point.0 as f32, evt.old_point.1 as f32);
                    let end_pos = start_pos + delta;

                    commands.entity(entity).insert(MovementTween {
                        start: start_pos,
                        end: end_pos,
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
                // Emote set 1
                BodyAnimationKind::Smile => (EpfAnimationType::Smile, 1),
                BodyAnimationKind::Cry => (EpfAnimationType::Cry, 1),
                BodyAnimationKind::Frown => (EpfAnimationType::Sad, 1),
                BodyAnimationKind::Wink => (EpfAnimationType::Wink, 1),
                BodyAnimationKind::Surprise => (EpfAnimationType::Stunned, 1),
                BodyAnimationKind::Tongue => (EpfAnimationType::Raz, 1),
                BodyAnimationKind::Pleasant => (EpfAnimationType::Surprise, 1),
                BodyAnimationKind::Snore => (EpfAnimationType::Sleepy, 2),
                BodyAnimationKind::Mouth => (EpfAnimationType::Yawn, 2),
                BodyAnimationKind::BlowKiss => (EpfAnimationType::BlowKiss, 2),
                BodyAnimationKind::Wave => (EpfAnimationType::Wave, 2),
                // Emote set 2
                BodyAnimationKind::Silly => (EpfAnimationType::BalloonElder, 1),
                BodyAnimationKind::Cute => (EpfAnimationType::BalloonJoy, 1),
                BodyAnimationKind::Yelling => (EpfAnimationType::BalloonSlick, 1),
                BodyAnimationKind::Mischievous => (EpfAnimationType::BalloonScheme, 1),
                BodyAnimationKind::Evil => (EpfAnimationType::BalloonLaser, 1),
                BodyAnimationKind::Horror => (EpfAnimationType::BalloonGloom, 1),
                BodyAnimationKind::PuppyDog => (EpfAnimationType::BalloonAwe, 1),
                BodyAnimationKind::StoneFaced => (EpfAnimationType::BalloonShadow, 1),
                BodyAnimationKind::Tears => (EpfAnimationType::BalloonSob, 3),
                BodyAnimationKind::FiredUp => (EpfAnimationType::BalloonFire, 3),
                BodyAnimationKind::Confused => (EpfAnimationType::BalloonDizzy, 3),
                // Emote set 3
                BodyAnimationKind::RockOn => (EpfAnimationType::SymbolRock, 1),
                BodyAnimationKind::Peace => (EpfAnimationType::SymbolScissors, 1),
                BodyAnimationKind::Stop => (EpfAnimationType::SymbolPaper, 1),
                BodyAnimationKind::Ouch => (EpfAnimationType::SymbolScramble, 1),
                BodyAnimationKind::Impatient => (EpfAnimationType::SymbolSilence, 3),
                BodyAnimationKind::Shock => (EpfAnimationType::Mask, 1),
                BodyAnimationKind::Pleasure => (EpfAnimationType::Blush, 1),
                BodyAnimationKind::Love => (EpfAnimationType::SymbolLove, 1),
                BodyAnimationKind::SweatDrop => (EpfAnimationType::SymbolSweat, 1),
                BodyAnimationKind::Whistle => (EpfAnimationType::SymbolMusic, 1),
                BodyAnimationKind::Irritation => (EpfAnimationType::SymbolAngry, 1),
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

                if !anim_type.is_emote() && !armor_supports {
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
        let t = (tween.elapsed / tween.duration).clamp(0.0, 1.0);

        *pos = tween.start.lerp(tween.end, t).into();

        if tween.elapsed >= tween.duration {
            *pos = tween.end.into();
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
            unconfirmed.pending.clear();
            unconfirmed.recent_deltas.clear();
        }
    }

    for event in entity_events.read() {
        match event {
            EntityEvent::PlayerLocation(location) => {
                position.x = location.x as f32;
                position.y = location.y as f32;
                unconfirmed.pending.clear();
                unconfirmed.recent_deltas.clear();
                commands.entity(entity).remove::<MovementTween>();
            }
            EntityEvent::PlayerWalkResponse(response) => {
                let confirmed_step = unconfirmed.pending.pop_front();

                if let ClientWalkResponseArgs::Accepted(_) = response.args {
                    if let Some(step) = confirmed_step {
                        let response_from =
                            Vec2::new(response.from.0 as f32, response.from.1 as f32);
                        let drift = response_from - step.expected_from;

                        if drift.length_squared() > 1e-6 {
                            unconfirmed.recent_deltas.push_back(drift);
                            if unconfirmed.recent_deltas.len() > 3 {
                                unconfirmed.recent_deltas.pop_front();
                            }

                            if unconfirmed.recent_deltas.len() >= 2 {
                                let first = unconfirmed.recent_deltas[0];
                                if unconfirmed
                                    .recent_deltas
                                    .iter()
                                    .all(|&d| d.distance_squared(first) < 1e-6)
                                {
                                    *position += first;

                                    if let Some(ref mut tween) = active_tween {
                                        tween.start += first;
                                        tween.end += first;
                                    }

                                    for pending in &mut unconfirmed.pending {
                                        pending.expected_from += first;
                                    }

                                    unconfirmed.recent_deltas.clear();
                                }
                            }
                        } else {
                            unconfirmed.recent_deltas.clear();
                        }
                    }
                }

                if let ClientWalkResponseArgs::Rejected = response.args {
                    unconfirmed.recent_deltas.clear();

                    // Snap to server's "from" position (state before the rejected step)
                    let mut current_pos = Vec2::new(response.from.0 as f32, response.from.1 as f32);
                    *position = current_pos.into();

                    // Replay unconfirmed steps
                    let count = unconfirmed.pending.len();
                    for (idx, step) in unconfirmed.pending.iter_mut().enumerate() {
                        let delta = step.direction.vec2_delta();
                        step.expected_from = current_pos;

                        if idx < count - 1 {
                            // Teleport for intermediate steps
                            current_pos += delta;
                        } else if let Some(ref mut active_tween) = active_tween {
                            // Final step: update tween
                            active_tween.start = current_pos;
                            active_tween.end = current_pos + delta;
                        } else {
                            // Final step: snap
                            current_pos += delta;
                        }
                    }

                    if active_tween.is_none() {
                        *position = current_pos.into();
                    }

                    if unconfirmed.pending.is_empty() {
                        commands.entity(entity).remove::<MovementTween>();
                    }
                }
            }
            _ => {}
        }
    }
}
