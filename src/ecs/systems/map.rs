//! Map loading and rendering systems

use super::super::components::*;
use crate::{
    Camera, DarknessState, MapRendererState, MinimapCacheState, MinimapRendererState,
    RendererState, WeatherState, events::MapEvent, game_files::GameFiles, lighting::LightMetadata,
    metafile_store::MetafileStore,
};
use bevy::prelude::*;
use rendering::scene::map::renderer::MapRenderer;
use tracing::info;

/// Handles map events: loading, clearing, light levels, and doors.
pub fn map_system(
    mut commands: Commands,
    mut map_events: MessageReader<MapEvent>,
    archive: Res<GameFiles>,
    scoped_q: Query<(Entity, Option<&LocalPlayer>), With<MapScoped>>,
    map_entities: Query<&GameMap>,
    renderer: Option<Res<RendererState>>,
    camera: Option<ResMut<Camera>>,
    minimap_renderer_state: Option<Res<MinimapRendererState>>,
    settings: Res<crate::settings::Settings>,
    mut darkness_state: Option<ResMut<DarknessState>>,
    mut weather_state: Option<ResMut<WeatherState>>,
    metafile_store: Res<MetafileStore>,
    mut door_queue: ResMut<MapDoorQueue>,
    mut tile_counters: ResMut<crate::resources::ItemTileCounters>,
) {
    let mut local_map_renderer: Option<MapRenderer> = None;
    // Track if we cleared the map this frame - if so, don't skip SetInfo even if
    // the old GameMap entity still appears in queries (despawn is deferred)
    let mut cleared_this_frame = false;
    // The map entity spawned by SetInfo isn't visible to `map_entities` until
    // this system's commands are applied, so track it locally to avoid spawning
    // a second GameMap when multiple SetInfo events arrive in the same batch.
    let mut spawned_this_frame = false;

    for event in map_events.read() {
        match event {
            MapEvent::Clear => {
                // Hide the overlay until the new map loads.
                if let Some(darkness) = darkness_state.as_deref_mut() {
                    darkness.renderer.set_ambient(0.0, [0, 0, 0]);
                }
                handle_map_clear(
                    &mut commands,
                    &scoped_q,
                    &mut local_map_renderer,
                    &mut tile_counters,
                );
                door_queue.pending.clear();
                cleared_this_frame = true;
            }
            MapEvent::SetInfo(map_info, map_bytes) => {
                if spawned_this_frame {
                    info!(
                        map_id = map_info.map_id,
                        "Skipping SetInfo for map_id {} - map entity already spawned this frame",
                        map_info.map_id
                    );
                    continue;
                }
                // Check if we're already on this map (happens during refresh)
                // Skip this check if we just cleared the map this frame, since the
                // old GameMap entity is still visible due to deferred despawning
                if !cleared_this_frame {
                    if let Some(current_map) = map_entities.iter().next() {
                        if current_map.map_id == map_info.map_id {
                            // Same-map refresh: apply flag-driven state without
                            // rebuilding the map renderer.
                            handle_map_weather(weather_state.as_deref_mut(), map_info.flags);
                            handle_map_darkness(
                                darkness_state.as_deref_mut(),
                                renderer.as_deref(),
                                &archive,
                                &metafile_store,
                                map_info,
                            );
                            continue;
                        }
                    }
                }

                local_map_renderer = handle_map_set_info(
                    &mut commands,
                    &archive,
                    renderer.as_deref(),
                    camera.as_deref(),
                    minimap_renderer_state.as_deref(),
                    &settings,
                    map_info,
                    map_bytes,
                );
                handle_map_darkness(
                    darkness_state.as_deref_mut(),
                    renderer.as_deref(),
                    &archive,
                    &metafile_store,
                    map_info,
                );
                handle_map_weather(weather_state.as_deref_mut(), map_info.flags);
                spawned_this_frame = true;
            }
            MapEvent::SetLightLevel(kind) => {
                handle_light_level(darkness_state.as_deref_mut(), kind);
            }
            MapEvent::ReloadLightMetadata => {
                reload_light_metadata(darkness_state.as_deref_mut(), &metafile_store);
            }
            MapEvent::SetDoors(door_data) => {
                door_queue.pending.extend(door_data.doors.clone());
            }
        }
    }

    if let Some(map_renderer) = local_map_renderer {
        commands.insert_resource(MapRendererState { map_renderer });
    }
}

/// Sets the weather mode from the map flags' low nibble (1 = snow, 2 = rain).
fn handle_map_weather(weather_state: Option<&mut WeatherState>, flags: u8) {
    use rendering::scene::weather::WeatherMode;

    let Some(weather) = weather_state else {
        return;
    };

    let mode = match flags & 0x0F {
        0x01 => WeatherMode::Snow,
        0x02 => WeatherMode::Rain,
        _ => WeatherMode::None,
    };
    weather.mode = mode;
    if let Some(renderer) = &mut weather.renderer {
        renderer.set_mode(mode);
    }
}

fn handle_map_clear(
    commands: &mut Commands,
    scoped_q: &Query<(Entity, Option<&LocalPlayer>), With<MapScoped>>,
    local_map_renderer: &mut Option<MapRenderer>,
    tile_counters: &mut crate::resources::ItemTileCounters,
) {
    info!("Map change pending: clearing current map entities");
    let mut count = 0;
    for (e, local_player) in scoped_q.iter() {
        // The local player survives map changes; the server repositions it via
        // Location / DisplayPlayer, and dedupe_entities_by_id swaps in a fresh
        // entity if a new DisplayPlayer arrives. Everything else is despawned.
        if local_player.is_some() {
            continue;
        }
        commands.entity(e).despawn();
        count += 1;
    }
    info!("Despawned {} MapScoped entities", count);
    commands.remove_resource::<MapRendererState>();
    commands.remove_resource::<MinimapCacheState>();
    commands.remove_resource::<crate::ecs::collision::MapCollisionData>();
    tile_counters.counters.clear();
    *local_map_renderer = None;
}

#[tracing::instrument(
    level = "info",
    skip_all,
    fields(map_id = map_info.map_id, map_bytes = map_bytes.len())
)]
fn handle_map_set_info(
    commands: &mut Commands,
    archive: &Res<GameFiles>,
    renderer: Option<&RendererState>,
    camera: Option<&Camera>,
    minimap_renderer_state: Option<&MinimapRendererState>,
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
    commands.insert_resource(MinimapCacheState::new(
        map_info.map_id,
        map_info.width,
        map_info.height,
    ));

    if let (Some(renderer), Some(camera)) = (renderer, camera) {
        if minimap_renderer_state.is_none() {
            if let Ok(minimap_state) = MinimapRendererState::new(
                renderer,
                &camera.camera.bind_group_layout,
                crate::FULLSCREEN_MINIMAP_ASSETS,
                camera.camera.camera.width as u32,
                camera.camera.camera.height as u32,
            ) {
                commands.insert_resource(minimap_state);
            }
        }
    }

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

/// Applies a light level packet.
fn handle_light_level(
    darkness_state: Option<&mut DarknessState>,
    kind: &packets::server::LightLevelKind,
) {
    tracing::info!("Setting light level to {:?}", kind);

    let Some(darkness) = darkness_state else {
        return;
    };

    let level = *kind as u8;
    darkness.last_light_level = Some(level);
    apply_ambient(darkness, level);
}

fn apply_ambient(darkness: &mut DarknessState, level: u8) {
    let (alpha, color) = darkness
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.resolve(darkness.map_id, level))
        .unwrap_or_else(|| {
            if darkness.is_dark_map {
                (1.0, [0, 0, 0])
            } else {
                (0.0, [0, 0, 0])
            }
        });
    darkness.renderer.set_ambient(alpha, color);
}

/// Re-parses the `Light` metafile and reapplies the current light level.
fn reload_light_metadata(
    darkness_state: Option<&mut DarknessState>,
    metafile_store: &MetafileStore,
) {
    let Some(darkness) = darkness_state else {
        return;
    };

    darkness.metadata = metafile_store
        .get_metafile_data("Light")
        .map(LightMetadata::from_metafile);

    match darkness.last_light_level {
        Some(level) => apply_ambient(darkness, level),
        None => darkness
            .renderer
            .set_ambient(if darkness.is_dark_map { 1.0 } else { 0.0 }, [0, 0, 0]),
    }
}

/// Loads the map's HEA light map and light metadata, then applies the current
/// light level.
fn handle_map_darkness(
    darkness_state: Option<&mut DarknessState>,
    renderer: Option<&RendererState>,
    archive: &GameFiles,
    metafile_store: &MetafileStore,
    map_info: &packets::server::MapInfo,
) {
    let (Some(darkness), Some(renderer)) = (darkness_state, renderer) else {
        return;
    };

    let weather_nibble = map_info.flags & 0x0F;
    let is_dark = weather_nibble == 0x03; // MapFlags.Darkness
    let hea = crate::lighting::load_hea(archive.inner().archive(), map_info.map_id);

    darkness.map_id = map_info.map_id;
    darkness.is_dark_map = is_dark;
    darkness.sources.clear();
    darkness.metadata = metafile_store
        .get_metafile_data("Light")
        .map(LightMetadata::from_metafile);
    darkness
        .renderer
        .set_map(&renderer.device, &renderer.queue, map_info.height, hea);
    darkness.composite_bind_group = None;
    darkness
        .renderer
        .set_ambient(if is_dark { 1.0 } else { 0.0 }, [0, 0, 0]);

    tracing::info!(
        map_id = map_info.map_id,
        flags = map_info.flags,
        is_dark,
        has_hea = darkness.renderer.has_hea(),
        has_light_metadata = darkness
            .metadata
            .as_ref()
            .is_some_and(|metadata| metadata.has_entry(map_info.map_id)),
        "Map darkness state"
    );

    match darkness.last_light_level {
        Some(level) => apply_ambient(darkness, level),
        None => darkness
            .renderer
            .set_ambient(if is_dark { 1.0 } else { 0.0 }, [0, 0, 0]),
    }
}

pub fn handle_doors(
    mut map_renderer_state: Option<ResMut<MapRendererState>>,
    minimap_cache: Option<ResMut<MinimapCacheState>>,
    mut map_collision: Option<ResMut<crate::ecs::collision::MapCollisionData>>,
    mut door_queue: ResMut<MapDoorQueue>,
) {
    let (Some(map_state), Some(map_collision)) = (
        map_renderer_state.as_deref_mut(),
        map_collision.as_deref_mut(),
    ) else {
        return;
    };

    if door_queue.pending.is_empty() {
        return;
    }

    for door in &door_queue.pending {
        map_state
            .map_renderer
            .set_wall_toggle_state(door.x, door.y, door.closed);
        map_collision.set_door(door.x, door.y, door.closed);
    }

    if let Some(mut minimap_cache) = minimap_cache {
        minimap_cache.mark_topology_dirty();
    }

    door_queue.pending.clear();
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
