//! Effect systems (spell effects, animations, etc.)

use super::super::components::*;
use crate::{
    Camera, EffectManagerState, RendererState, events::EntityEvent, game_files::GameFiles,
};
use bevy::prelude::*;
use rendering::scene::get_isometric_coordinate;

/// Attaches Effect components to entities based on server animation events.
pub fn entity_effect_system(
    mut entity_events: MessageReader<EntityEvent>,
    mut targets: Query<(Entity, &EntityId)>,
    mut commands: Commands,
) {
    for event in entity_events.read() {
        let EntityEvent::Effect(anim) = event else {
            continue;
        };

        match *anim {
            packets::server::Animation::Source {
                target_id,
                target_animation,
                source_id,
                source_animation,
                ..
            } => {
                for (entity, entity_id) in targets.iter_mut() {
                    if entity_id.id == target_id {
                        if let Some(target_animation) = target_animation {
                            tracing::info!(
                                game_id = entity_id.id,
                                role = "target",
                                effect_id = target_animation,
                                "Attaching effect"
                            );
                            commands.entity(entity).insert(Effect {
                                effect_id: target_animation,
                                z_offset: 0.0001,
                            });
                        }
                    }
                    if entity_id.id == source_id {
                        if let Some(source_animation) = source_animation {
                            tracing::info!(
                                game_id = entity_id.id,
                                role = "source",
                                effect_id = source_animation,
                                "Attaching effect"
                            );
                            commands.entity(entity).insert(Effect {
                                effect_id: source_animation,
                                z_offset: 0.0001,
                            });
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

/// Spawns effect instances on the GPU when Effect components are added.
pub fn spawn_effects_system(
    mut commands: Commands,
    renderer: Res<RendererState>,
    game_files: Res<GameFiles>,
    camera: Res<Camera>,
    mut effects_state: ResMut<EffectManagerState>,
    changed_effects: Query<
        (Entity, &Position, &Effect, Option<&EffectInstance>),
        Or<(Added<Effect>, Changed<Effect>)>,
    >,
) {
    for (entity, position, effect, existing_instance) in changed_effects.iter() {
        let needs_respawn = match existing_instance {
            Some(instance) => instance.handle.effect_id != effect.effect_id,
            None => true,
        };
        if !needs_respawn {
            continue;
        }
        tracing::info!(
            ?entity,
            effect_id = effect.effect_id,
            x = position.x,
            y = position.y,
            z_offset = effect.z_offset,
            iso_x = get_isometric_coordinate(position.x, position.y).x,
            iso_y = get_isometric_coordinate(position.x, position.y).y,
            camera_iso_x = camera.camera.position().x,
            camera_iso_y = camera.camera.position().y,
            zoom = camera.camera.zoom(),
            "Displaying effect"
        );
        if let Some(handle) = effects_state.effect_manager.spawn_effect(
            &renderer.queue,
            &game_files.inner().archive(),
            effect.effect_id,
            position.x,
            position.y,
            effect.z_offset,
        ) {
            if existing_instance.is_some() {
                commands.entity(entity).remove::<EffectInstance>();
            }
            commands.entity(entity).insert(EffectInstance {
                current_frame: 0,
                timer: Timer::from_seconds(
                    (handle.frame_interval_ms as f32) / 1000.,
                    TimerMode::Repeating,
                ),
                handle,
            });
        } else {
            tracing::warn!("Failed to spawn effect with ID {}", effect.effect_id);
        }
    }
}

/// Updates effect positions to follow their parent entities.
pub fn effect_follow_entity_system(
    mut effects_query: Query<(&mut Position, &FollowsEntity), With<Effect>>,
    target_query: Query<&Position, Without<Effect>>,
) {
    for (mut effect_pos, follows) in effects_query.iter_mut() {
        if let Ok(target_pos) = target_query.get(follows.0) {
            effect_pos.x = target_pos.x;
            effect_pos.y = target_pos.y;
        }
    }
}

/// Advances effect animations and removes completed effects.
pub fn update_effects_system(
    mut commands: Commands,
    time: Res<Time>,
    effects_state: Res<EffectManagerState>,
    mut effects_query: Query<(Entity, &Position, &Effect, &mut EffectInstance)>,
) {
    let delta = time.delta();

    for (entity, position, effect, mut instance) in effects_query.iter_mut() {
        if instance.timer.tick(delta).is_finished() {
            instance.current_frame += 1;
            instance.timer.reset();

            let frame_count = instance.handle.frame_count;
            if instance.current_frame >= frame_count {
                commands.entity(entity).remove::<(Effect, EffectInstance)>();
                continue;
            }
        }

        effects_state.effect_manager.update_effect(
            &instance.handle,
            position.x,
            position.y,
            effect.z_offset,
            instance.current_frame,
        );
    }
}
