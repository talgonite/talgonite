use bevy::prelude::*;
use glam::Vec2;
use rendering::scene::utils::screen_to_iso_tile;
use std::cmp::Ordering;

use crate::app_state::AppState;
use crate::ecs::components::{EntityId, Hitbox, ItemSprite, LocalPlayer, NPC, Player, Position};
use crate::ecs::interaction::HoveredEntity;
use crate::ecs::spell_casting::SpellCastingState;
use crate::events::{
    ClickSource, EntityClickEvent, EntityHoverEvent, ResolvedPointerClickEvent, TileClickEvent,
    WallClickEvent, WorldContextAction, WorldContextMenuEntry,
};
use crate::network::PacketOutbox;
use crate::resources::ZoomState;
use crate::slint_plugin::{ShowSelfProfileEvent, SlintDoubleClickEvent};
use crate::webui::plugin::{ActiveWorldContextMenu, CursorPosition, UiOutbound};
use crate::{Camera, WindowSurface};
use game_ui::{CoreToUi, WorldContextMenuEntryUi};
use packets::client::{Click, Pickup, SelfProfileRequest};

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MouseInteractionSet;

pub struct MouseInteractionPlugin;

impl Plugin for MouseInteractionPlugin {
    fn build(&self, app: &mut App) {
        // Note: EntityHoverEvent and EntityClickEvent are registered in CoreEventsPlugin
        app.insert_resource(InteractionState::default())
            .init_resource::<HoveredEntity>()
            .add_systems(
                Update,
                (
                    mouse_interaction_system,
                    handle_resolved_pointer_clicks,
                    handle_double_clicks,
                    handle_entity_clicks,
                    handle_wall_clicks,
                )
                    .chain()
                    .in_set(MouseInteractionSet)
                    .after(crate::plugins::input::InputPumpSet)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}

#[derive(Resource, Default)]
struct InteractionState {
    last_entity: Option<Entity>,
}

struct SceneHitResult {
    top_entity: Option<Entity>,
    entity_hits: Vec<SceneEntityHit>,
    matching_walls: Vec<(i32, i32, bool)>,
    ground_x: i32,
    ground_y: i32,
    ground_is_walkable: bool,
}

#[derive(Clone)]
struct SceneEntityHit {
    entity: Entity,
    tile_x: i32,
    tile_y: i32,
    depth: f32,
    kind: SceneEntityHitKind,
}

#[derive(Clone)]
enum SceneEntityHitKind {
    Player { name: String, is_self: bool },
    Npc { name: String },
    Item,
}

fn mouse_interaction_system(
    cursor: Res<CursorPosition>,
    buttons: Res<ButtonInput<MouseButton>>,
    camera: Res<Camera>,
    window_surface: Option<NonSend<WindowSurface>>,
    zoom_state: Option<Res<ZoomState>>,
    collision_table: Option<Res<crate::ecs::collision::WallCollisionTable>>,
    mut interaction_state: ResMut<InteractionState>,
    mut hovered_entity: ResMut<HoveredEntity>,
    entity_query: Query<(
        Entity,
        &Position,
        Option<&Hitbox>,
        Option<&Player>,
        Option<&NPC>,
        Option<&ItemSprite>,
        Option<&LocalPlayer>,
    )>,
    mut hover_events: MessageWriter<EntityHoverEvent>,
    mut click_events: MessageWriter<EntityClickEvent>,
    mut tile_click_events: MessageWriter<TileClickEvent>,
    mut wall_click_events: MessageWriter<WallClickEvent>,
    map_collision: Option<Res<crate::ecs::collision::MapCollisionData>>,
) {
    let Some(hit_result) = hit_test_scene(
        &camera,
        window_surface.as_deref(),
        zoom_state.as_deref(),
        collision_table.as_deref(),
        &entity_query,
        map_collision.as_deref(),
        (cursor.x, cursor.y),
    ) else {
        return;
    };

    let current_entity = hit_result.top_entity;
    hovered_entity.0 = current_entity;

    if current_entity != interaction_state.last_entity {
        if let Some(entity) = current_entity {
            hover_events.write(EntityHoverEvent { entity });
        }
        interaction_state.last_entity = current_entity;
    }

    if buttons.just_pressed(MouseButton::Left) {
        emit_scene_click(
            &hit_result,
            MouseButton::Left,
            ClickSource::DesktopMouse,
            false,
            &mut click_events,
            &mut tile_click_events,
            &mut wall_click_events,
        );
    }

    if buttons.just_pressed(MouseButton::Right) {
        emit_scene_click(
            &hit_result,
            MouseButton::Right,
            ClickSource::DesktopMouse,
            false,
            &mut click_events,
            &mut tile_click_events,
            &mut wall_click_events,
        );
    }
}

fn handle_resolved_pointer_clicks(
    mut resolved_clicks: MessageReader<ResolvedPointerClickEvent>,
    camera: Res<Camera>,
    window_surface: Option<NonSend<WindowSurface>>,
    zoom_state: Option<Res<ZoomState>>,
    collision_table: Option<Res<crate::ecs::collision::WallCollisionTable>>,
    entity_query: Query<(
        Entity,
        &Position,
        Option<&Hitbox>,
        Option<&Player>,
        Option<&NPC>,
        Option<&ItemSprite>,
        Option<&LocalPlayer>,
    )>,
    mut click_events: MessageWriter<EntityClickEvent>,
    mut tile_click_events: MessageWriter<TileClickEvent>,
    mut wall_click_events: MessageWriter<WallClickEvent>,
    map_collision: Option<Res<crate::ecs::collision::MapCollisionData>>,
    spell_casting: Res<SpellCastingState>,
    mut ui_outbound: MessageWriter<UiOutbound>,
    mut world_context_menu: ResMut<ActiveWorldContextMenu>,
) {
    let is_waiting_for_target = spell_casting
        .active_cast
        .as_ref()
        .map_or(false, |cast| cast.waiting_for_target);

    for event in resolved_clicks.read() {
        let Some(hit_result) = hit_test_scene(
            &camera,
            window_surface.as_deref(),
            zoom_state.as_deref(),
            collision_table.as_deref(),
            &entity_query,
            map_collision.as_deref(),
            event.position,
        ) else {
            continue;
        };

        if event.source == ClickSource::AndroidLongPress {
            if is_waiting_for_target {
                continue;
            }

            if show_world_context_menu(
                &hit_result,
                event.position,
                &mut world_context_menu,
                &mut ui_outbound,
            ) {
                continue;
            }
        }

        emit_scene_click(
            &hit_result,
            event.button,
            event.source,
            false,
            &mut click_events,
            &mut tile_click_events,
            &mut wall_click_events,
        );
    }
}

fn handle_double_clicks(
    mut double_click_events: MessageReader<SlintDoubleClickEvent>,
    spell_casting: Res<SpellCastingState>,
    camera: Res<Camera>,
    window_surface: Option<NonSend<WindowSurface>>,
    zoom_state: Option<Res<ZoomState>>,
    entity_query: Query<(Entity, &Position, Option<&Hitbox>)>,
    mut click_events: MessageWriter<EntityClickEvent>,
) {
    if spell_casting
        .active_cast
        .as_ref()
        .map_or(false, |c| c.waiting_for_target)
    {
        return;
    }

    let Some(window_surface) = window_surface else {
        return;
    };
    let Some(zoom_state) = zoom_state else {
        return;
    };

    let cam_pos = camera.camera.position();
    let zoom = camera.camera.zoom();
    let win_size = Vec2::new(window_surface.width as f32, window_surface.height as f32);
    let cursor_scale = zoom_state.cursor_to_render_scale();

    for event in double_click_events.read() {
        let screen = Vec2::new(event.0 * cursor_scale, event.1 * cursor_scale);
        let tile = screen_to_iso_tile(screen, cam_pos, win_size, zoom);

        let mut hits = Vec::new();
        for (entity, pos, hitbox) in entity_query.iter() {
            let Some(hb) = hitbox else { continue };

            let hit = hb.check_hit(
                Vec2::new(pos.x, pos.y),
                tile,
                screen,
                cam_pos,
                win_size,
                zoom,
            );

            if hit {
                hits.push((entity, pos.x + pos.y));
            }
        }

        hits.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        if let Some((entity, _)) = hits.first() {
            click_events.write(EntityClickEvent {
                entity: *entity,
                ground_tile_x: tile.x.floor() as i32,
                ground_tile_y: tile.y.floor() as i32,
                button: MouseButton::Left,
                source: ClickSource::DesktopMouse,
                is_double_click: true,
            });
        }
    }
}

fn handle_entity_clicks(
    mut events: MessageReader<EntityClickEvent>,
    mut profile_events: MessageWriter<ShowSelfProfileEvent>,
    spell_casting: Res<SpellCastingState>,
    query: Query<(
        &EntityId,
        &Position,
        Option<&Player>,
        Option<&NPC>,
        Option<&LocalPlayer>,
        Option<&ItemSprite>,
    )>,
    outbox: Res<PacketOutbox>,
) {
    let is_waiting_for_target = spell_casting
        .active_cast
        .as_ref()
        .map_or(false, |c| c.waiting_for_target);

    for event in events.read() {
        if is_waiting_for_target {
            continue;
        }

        if let Ok((entity_id, position, player, npc, local_player, item)) = query.get(event.entity)
        {
            if event.button == MouseButton::Left {
                if event.is_double_click {
                    if local_player.is_some() {
                        // Show profile panel locally
                        profile_events.write(ShowSelfProfileEvent::SelfRequested);
                        // Also send to server (for future use)
                        outbox.send(&SelfProfileRequest {});
                    } else if item.is_some() {
                        outbox.send(&Pickup {
                            destination_slot: 0,
                            source_point: (position.x as u16, position.y as u16),
                        });
                    } else if player.is_some() {
                        // Optimistically clear previous other player data
                        profile_events.write(ShowSelfProfileEvent::OtherRequested);
                        outbox.send(&Click::TargetEntity(entity_id.id));
                    } else if npc.is_some() {
                        outbox.send(&Click::TargetEntity(entity_id.id));
                    }
                } else {
                    // Single click for players/NPCs is now ignored here to prevent
                    // opening profiles on single clicks. Targeting is handled
                    // separately by spell systems or client-side selection.
                }
            }
        }
    }
}

fn handle_wall_clicks(mut wall_events: MessageReader<WallClickEvent>, outbox: Res<PacketOutbox>) {
    for event in wall_events.read() {
        if event.button == MouseButton::Left {
            outbox.send(&Click::TargetWall {
                x: event.tile_x.max(0) as u16,
                y: event.tile_y.max(0) as u16,
                is_right: event.is_right,
            });
        }
    }
}

fn hit_test_scene(
    camera: &Camera,
    window_surface: Option<&WindowSurface>,
    zoom_state: Option<&ZoomState>,
    collision_table: Option<&crate::ecs::collision::WallCollisionTable>,
    entity_query: &Query<(
        Entity,
        &Position,
        Option<&Hitbox>,
        Option<&Player>,
        Option<&NPC>,
        Option<&ItemSprite>,
        Option<&LocalPlayer>,
    )>,
    map_collision: Option<&crate::ecs::collision::MapCollisionData>,
    pointer_position: (f32, f32),
) -> Option<SceneHitResult> {
    let window_surface = window_surface?;
    let zoom_state = zoom_state?;

    let cam_pos = camera.camera.position();
    let zoom = camera.camera.zoom();
    let win_size = Vec2::new(window_surface.width as f32, window_surface.height as f32);

    if win_size.x <= 0.0 || win_size.y <= 0.0 {
        return None;
    }

    let cursor_scale = zoom_state.cursor_to_render_scale();
    let screen = Vec2::new(
        pointer_position.0 * cursor_scale,
        pointer_position.1 * cursor_scale,
    );
    let tile = screen_to_iso_tile(screen, cam_pos, win_size, zoom);

    let mut matching_walls = Vec::new();
    if let Some(map_collision) = map_collision {
        let d_floor = (tile.x - tile.y).floor() as i32;
        let s_idx = d_floor + (map_collision.height as i32) - 1;

        if let Some(strip) = map_collision.strips.get(s_idx as usize) {
            let world_y = (tile.x + tile.y - 1.0) * 14.0;

            for wall in strip {
                let base_y = (wall.x as f32 + wall.y as f32 + 1.0) * 14.0;
                let top_y = base_y - wall.height as f32;

                if world_y >= top_y && world_y <= base_y {
                    matching_walls.push((wall.x as i32, wall.y as i32, wall.is_right));
                }
            }
        }
    }

    let mut hits = Vec::new();
    for (entity, pos, hitbox, player, npc, item, local_player) in entity_query.iter() {
        let Some(hb) = hitbox else {
            continue;
        };

        if hb.check_hit(
            Vec2::new(pos.x, pos.y),
            tile,
            screen,
            cam_pos,
            win_size,
            zoom,
        ) {
            let kind = if let Some(player) = player {
                Some(SceneEntityHitKind::Player {
                    name: player.name.clone(),
                    is_self: local_player.is_some(),
                })
            } else if let Some(npc) = npc {
                Some(SceneEntityHitKind::Npc {
                    name: npc.name.clone(),
                })
            } else if item.is_some() {
                Some(SceneEntityHitKind::Item)
            } else {
                None
            };

            if let Some(kind) = kind {
                hits.push(SceneEntityHit {
                    entity,
                    tile_x: pos.x.round() as i32,
                    tile_y: pos.y.round() as i32,
                    depth: pos.x + pos.y,
                    kind,
                });
            }
        }
    }

    hits.sort_by(|a, b| b.depth.partial_cmp(&a.depth).unwrap_or(Ordering::Equal));

    Some(SceneHitResult {
        top_entity: hits.first().map(|hit| hit.entity),
        entity_hits: hits,
        matching_walls,
        ground_x: tile.x.floor() as i32,
        ground_y: tile.y.floor() as i32,
        ground_is_walkable: crate::ecs::collision::can_walk_to(
            tile.x.floor().max(0.0) as u8,
            tile.y.floor().max(0.0) as u8,
            collision_table,
            map_collision,
        ),
    })
}

fn emit_scene_click(
    hit_result: &SceneHitResult,
    button: MouseButton,
    source: ClickSource,
    is_double_click: bool,
    click_events: &mut MessageWriter<EntityClickEvent>,
    tile_click_events: &mut MessageWriter<TileClickEvent>,
    wall_click_events: &mut MessageWriter<WallClickEvent>,
) {
    if button == MouseButton::Left {
        if let Some(entity) = hit_result.top_entity {
            click_events.write(EntityClickEvent {
                entity,
                ground_tile_x: hit_result.ground_x,
                ground_tile_y: hit_result.ground_y,
                button,
                source,
                is_double_click,
            });
            return;
        }

        if !hit_result.matching_walls.is_empty() {
            if source == ClickSource::AndroidShortPress && hit_result.ground_is_walkable {
                tile_click_events.write(TileClickEvent {
                    tile_x: hit_result.ground_x,
                    tile_y: hit_result.ground_y,
                    button,
                    source,
                });
                return;
            }

            for (tile_x, tile_y, is_right) in &hit_result.matching_walls {
                wall_click_events.write(WallClickEvent {
                    tile_x: *tile_x,
                    tile_y: *tile_y,
                    is_right: *is_right,
                    button,
                    source,
                });
            }
            return;
        }
    }

    if button == MouseButton::Right {
        if let Some(entity) = hit_result.top_entity {
            click_events.write(EntityClickEvent {
                entity,
                ground_tile_x: hit_result.ground_x,
                ground_tile_y: hit_result.ground_y,
                button,
                source,
                is_double_click,
            });

            return;
        }

        tile_click_events.write(TileClickEvent {
            tile_x: hit_result.ground_x,
            tile_y: hit_result.ground_y,
            button,
            source,
        });
        return;
    }

    tile_click_events.write(TileClickEvent {
        tile_x: hit_result.ground_x,
        tile_y: hit_result.ground_y,
        button,
        source,
    });
}

fn show_world_context_menu(
    hit_result: &SceneHitResult,
    pointer_position: (f32, f32),
    world_context_menu: &mut ResMut<ActiveWorldContextMenu>,
    ui_outbound: &mut MessageWriter<UiOutbound>,
) -> bool {
    let (title, entries) = build_world_context_entries(hit_result);
    if entries.is_empty() {
        return false;
    }

    world_context_menu.title = title.clone();
    world_context_menu.entries = entries.clone();

    ui_outbound.write(UiOutbound(CoreToUi::ShowWorldContextMenu {
        title,
        x: pointer_position.0,
        y: pointer_position.1,
        anchor_width: 1.0,
        anchor_height: 1.0,
        entries: entries
            .into_iter()
            .map(|entry| WorldContextMenuEntryUi {
                id: entry.id,
                text: entry.text,
            })
            .collect(),
    }));

    true
}

fn build_world_context_entries(
    hit_result: &SceneHitResult,
) -> (String, Vec<WorldContextMenuEntry>) {
    let mut title = String::new();
    let mut entries = Vec::new();

    for hit in &hit_result.entity_hits {
        match &hit.kind {
            SceneEntityHitKind::Player { name, is_self } => {
                if title.is_empty() && hit_result.top_entity == Some(hit.entity) {
                    title = name.clone();
                }

                if !is_self {
                    push_world_context_entry(
                        &mut entries,
                        "Walk to",
                        WorldContextAction::ApproachActor {
                            entity: hit.entity,
                            tile_x: hit.tile_x,
                            tile_y: hit.tile_y,
                        },
                    );
                }

                push_world_context_entry(
                    &mut entries,
                    "View profile",
                    WorldContextAction::ViewProfile {
                        entity: hit.entity,
                        is_self: *is_self,
                    },
                );
            }
            SceneEntityHitKind::Npc { name } => {
                if title.is_empty() && hit_result.top_entity == Some(hit.entity) {
                    title = name.clone();
                }

                push_world_context_entry(
                    &mut entries,
                    format!("Speak to {}", name),
                    WorldContextAction::SpeakToNpc { entity: hit.entity },
                );
            }
            SceneEntityHitKind::Item => {
                push_world_context_entry(
                    &mut entries,
                    "Pick up",
                    WorldContextAction::PickUpItem {
                        tile_x: hit.tile_x,
                        tile_y: hit.tile_y,
                    },
                );
            }
        }
    }

    if !hit_result.matching_walls.is_empty() {
        push_world_context_entry(
            &mut entries,
            "Interact",
            WorldContextAction::InteractWalls {
                walls: hit_result.matching_walls.clone(),
            },
        );
    }

    if hit_result.ground_is_walkable {
        push_world_context_entry(
            &mut entries,
            "Walk here",
            WorldContextAction::WalkToTile {
                tile_x: hit_result.ground_x,
                tile_y: hit_result.ground_y,
            },
        );
    }

    (title, entries)
}

fn push_world_context_entry(
    entries: &mut Vec<WorldContextMenuEntry>,
    text: impl Into<String>,
    action: WorldContextAction,
) {
    if entries.iter().any(|entry| entry.action == action) {
        return;
    }

    entries.push(WorldContextMenuEntry {
        id: entries.len() as i32,
        text: text.into(),
        action,
    });
}
