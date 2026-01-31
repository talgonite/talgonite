//! Map loading and rendering systems

use super::super::components::*;
use crate::{Camera, MapRendererState, RendererState, events::MapEvent, game_files::GameFiles};
use bevy::prelude::*;
use rendering::scene::map::renderer::MapRenderer;
use tracing::info;

/// Handles map events: loading, clearing, light levels, and doors.
pub fn map_system(
    mut commands: Commands,
    mut map_events: MessageReader<MapEvent>,
    archive: Res<GameFiles>,
    scoped_q: Query<Entity, With<MapScoped>>,
    map_entities: Query<&GameMap>,
    renderer: Option<Res<RendererState>>,
    mut camera: Option<ResMut<Camera>>,
    settings: Res<crate::settings::Settings>,
    mut map_renderer_state: Option<ResMut<MapRendererState>>,
    mut map_collision: Option<ResMut<crate::ecs::collision::MapCollisionData>>,
    mut tile_counters: ResMut<crate::resources::ItemTileCounters>,
) {
    let mut local_map_renderer: Option<MapRenderer> = None;
    // Track if we cleared the map this frame - if so, don't skip SetInfo even if
    // the old GameMap entity still appears in queries (despawn is deferred)
    let mut cleared_this_frame = false;

    for event in map_events.read() {
        match event {
            MapEvent::Clear => {
                handle_map_clear(
                    &mut commands,
                    &scoped_q,
                    &mut local_map_renderer,
                    &mut tile_counters,
                );
                cleared_this_frame = true;
            }
            MapEvent::SetInfo(map_info, map_bytes) => {
                // Check if we're already on this map (happens during refresh)
                // Skip this check if we just cleared the map this frame, since the
                // old GameMap entity is still visible due to deferred despawning
                if !cleared_this_frame {
                    if let Some(current_map) = map_entities.iter().next() {
                        if current_map.map_id == map_info.map_id {
                            info!(
                                "Skipping SetInfo for map_id {} - already on this map (likely refresh)",
                                map_info.map_id
                            );
                            continue;
                        }
                    }
                }

                local_map_renderer = handle_map_set_info(
                    &mut commands,
                    &archive,
                    renderer.as_deref(),
                    &settings,
                    map_info,
                    map_bytes,
                );
            }
            MapEvent::SetLightLevel(kind) => {
                handle_light_level(camera.as_deref_mut(), renderer.as_deref(), kind);
            }
            MapEvent::SetDoors(door_data) => {
                handle_doors(
                    renderer.as_deref(),
                    local_map_renderer.as_mut(),
                    map_renderer_state.as_deref_mut(),
                    door_data,
                );
                if let Some(map_collision) = map_collision.as_deref_mut() {
                    for door in &door_data.doors {
                        map_collision.set_door(door.x, door.y, door.closed);
                    }
                }
            }
        }
    }

    if let Some(map_renderer) = local_map_renderer {
        commands.insert_resource(MapRendererState { map_renderer });
    }
}

fn handle_map_clear(
    commands: &mut Commands,
    scoped_q: &Query<Entity, With<MapScoped>>,
    local_map_renderer: &mut Option<MapRenderer>,
    tile_counters: &mut crate::resources::ItemTileCounters,
) {
    info!("Map change pending: clearing current map entities");
    let mut count = 0;
    for e in scoped_q.iter() {
        commands.entity(e).despawn();
        count += 1;
    }
    info!("Despawned {} MapScoped entities", count);
    commands.remove_resource::<MapRendererState>();
    commands.remove_resource::<crate::ecs::collision::MapCollisionData>();
    tile_counters.counters.clear();
    *local_map_renderer = None;
}

fn handle_map_set_info(
    commands: &mut Commands,
    archive: &Res<GameFiles>,
    renderer: Option<&RendererState>,
    settings: &Res<crate::settings::Settings>,
    map_info: &packets::server::MapInfo,
    map_bytes: &std::sync::Arc<[u8]>,
) -> Option<MapRenderer> {
    info!(
        map_id = map_info.map_id,
        name = %map_info.name,
        size = map_bytes.len(),
        "Map change: preparing map (sync)"
    );

    let archive_ref = archive.inner().archive();
    let prepared_map = MapRenderer::prepare_map(
        archive_ref,
        (*map_bytes).to_vec(),
        map_info.width,
        map_info.height,
        false,
        settings.graphics.xray_size != crate::settings_types::XRaySize::Off,
    );

    // Parse collision data
    let map_collision = crate::ecs::collision::MapCollisionData::from_map_bytes(
        map_bytes,
        map_info.width,
        map_info.height,
        &prepared_map.wall_heights,
    );
    commands.insert_resource(map_collision);

    // Bind map to renderer
    let local_map_renderer = if let Some(renderer) = renderer {
        Some(MapRenderer::bind_map(
            &renderer.device,
            &renderer.queue,
            prepared_map,
        ))
    } else {
        Some(MapRenderer::empty())
    };

    // Spawn map entity (scoped)
    commands.spawn((
        MapBundle {
            map: GameMap {
                map_id: map_info.map_id,
                width: map_info.width,
                height: map_info.height,
                name: map_info.name.clone(),
            },
            loaded: MapLoaded,
        },
        InGameScoped,
        MapScoped,
    ));

    local_map_renderer
}

fn handle_light_level(
    camera: Option<&mut Camera>,
    renderer: Option<&RendererState>,
    kind: &packets::server::LightLevelKind,
) {
    use packets::server::LightLevelKind;

    tracing::info!("Setting light level to {:?}", kind);

    let (r, g, b) = match kind {
        LightLevelKind::DarkestA => (-0.02745098, -0.011764706, -0.02745098),
        LightLevelKind::DarkerB => (-0.011764706, -0.011764706, -0.011764706),
        LightLevelKind::DarkB => (-0.011764706, -0.011764706, -0.011764706),
        LightLevelKind::LighterA => (-0.011764706, -0.011764706, -0.011764706),
        LightLevelKind::LightestA => (-0.011764706, -0.011764706, -0.011764706),
        _ => (0.0, 0.0, 0.0),
    };

    if let (Some(camera), Some(renderer)) = (camera, renderer) {
        camera.camera.set_tint(&renderer.queue, r, g, b);
    }
}

fn handle_doors(
    renderer: Option<&RendererState>,
    local_map_renderer: Option<&mut MapRenderer>,
    map_renderer_state: Option<&mut MapRendererState>,
    door_data: &packets::server::Door,
) {
    let Some(renderer) = renderer else {
        return;
    };

    if let Some(map_renderer) = local_map_renderer {
        for door in &door_data.doors {
            tracing::debug!(
                "Setting door state at ({}, {}): closed={} (local)",
                door.x,
                door.y,
                door.closed
            );
            map_renderer.set_wall_toggle_state(&renderer.queue, door.x, door.y, door.closed);
        }
    } else if let Some(map_state) = map_renderer_state {
        for door in &door_data.doors {
            tracing::debug!(
                "Setting door state at ({}, {}): closed={} (resource)",
                door.x,
                door.y,
                door.closed
            );
            map_state.map_renderer.set_wall_toggle_state(
                &renderer.queue,
                door.x,
                door.y,
                door.closed,
            );
        }
    } else {
        tracing::warn!("Received SetDoors but no map renderer available (local or resource)");
    }
}

/// Updates map tile animations each frame.
pub fn map_animation_system(
    map_renderer_state: Option<ResMut<MapRendererState>>,
    renderer_state: Res<RendererState>,
) {
    if let Some(mut map_state) = map_renderer_state {
        map_state
            .map_renderer
            .update_animations(&renderer_state.queue);
    }
}
