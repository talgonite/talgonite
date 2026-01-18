use bevy::prelude::*;
use glam::Vec2;
use rendering::scene::utils::screen_to_iso_tile;

use crate::ecs::components::{EntityId, Hitbox, ItemSprite, LocalPlayer, NPC, Player, Position};
use crate::ecs::interaction::HoveredEntity;
use crate::ecs::spell_casting::SpellCastingState;
use crate::events::{EntityClickEvent, EntityHoverEvent, TileClickEvent};
use crate::network::PacketOutbox;
use crate::resources::ZoomState;
use crate::slint_plugin::{ShowSelfProfileEvent, SlintDoubleClickEvent};
use crate::webui::plugin::CursorPosition;
use crate::{Camera, WindowSurface};
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
                    handle_double_clicks,
                    handle_entity_clicks,
                )
                    .chain()
                    .in_set(MouseInteractionSet)
                    .after(crate::plugins::input::InputPumpSet),
            );
    }
}

#[derive(Resource, Default)]
struct InteractionState {
    last_tile: Option<(i32, i32)>,
    last_entity: Option<Entity>,
}

fn mouse_interaction_system(
    cursor: Res<CursorPosition>,
    buttons: Res<ButtonInput<MouseButton>>,
    camera: Res<Camera>,
    window_surface: Option<NonSend<WindowSurface>>,
    zoom_state: Option<Res<ZoomState>>,
    mut interaction_state: ResMut<InteractionState>,
    mut hovered_entity: ResMut<HoveredEntity>,
    entity_query: Query<(
        Entity,
        &Position,
        Option<&Hitbox>,
        Option<&Player>,
        Option<&NPC>,
        Option<&ItemSprite>,
    )>,
    mut hover_events: MessageWriter<EntityHoverEvent>,
    mut click_events: MessageWriter<EntityClickEvent>,
    mut tile_click_events: MessageWriter<TileClickEvent>,
) {
    let Some(window_surface) = window_surface else {
        return;
    };
    let Some(zoom_state) = zoom_state else {
        return;
    };

    let cam_pos = camera.camera.position();
    let zoom = camera.camera.zoom();
    let win_size = Vec2::new(window_surface.width as f32, window_surface.height as f32);

    if win_size.x <= 0.0 || win_size.y <= 0.0 {
        return;
    }

    let cursor_scale = zoom_state.cursor_to_render_scale();
    let screen = Vec2::new(cursor.x * cursor_scale, cursor.y * cursor_scale);
    let tile = screen_to_iso_tile(screen, cam_pos, win_size, zoom);
    let tile_i = (tile.x.floor() as i32, tile.y.floor() as i32);

    // Find hovered entities
    let mut hits = Vec::new();

    for (entity, pos, hitbox, player, npc, item) in entity_query.iter() {
        let Some(hb) = hitbox else {
            continue;
        };

        let hit = hb.check_hit(
            Vec2::new(pos.x, pos.y),
            tile,
            screen,
            cam_pos,
            win_size,
            zoom,
        );

        if hit {
            hits.push((entity, player, npc, item, pos.x + pos.y));
        }
    }

    hits.sort_by(|a, b| b.4.partial_cmp(&a.4).unwrap_or(std::cmp::Ordering::Equal));

    // Emit Hover Events
    let top_hit = hits.first();
    let current_entity = top_hit.map(|(e, _, _, _, _)| *e);
    hovered_entity.0 = current_entity;

    if current_entity != interaction_state.last_entity {
        if let Some((entity, _, _, _, _)) = top_hit {
            hover_events.write(EntityHoverEvent { entity: *entity });
        }
        interaction_state.last_entity = current_entity;
    }

    // Update last hover for logging/debouncing
    if interaction_state.last_tile != Some(tile_i) {
        interaction_state.last_tile = Some(tile_i);
    }

    // Handle Clicks
    if buttons.just_pressed(MouseButton::Left) {
        if let Some((entity, _, _, _, _)) = hits.first() {
            click_events.write(EntityClickEvent {
                entity: *entity,
                button: MouseButton::Left,
                is_double_click: false,
            });
            tracing::info!("Clicked Entity {:?}", entity);
        }
    }

    if buttons.just_pressed(MouseButton::Right) {
        if let Some((entity, _, _, _, _)) = hits.first() {
            click_events.write(EntityClickEvent {
                entity: *entity,
                button: MouseButton::Right,
                is_double_click: false,
            });
            tracing::info!("Right Clicked Entity {:?}", entity);
        }

        tile_click_events.write(TileClickEvent {
            tile_x: tile_i.0,
            tile_y: tile_i.1,
            button: MouseButton::Right,
        });
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
        let tile_i = (tile.x.floor() as i32, tile.y.floor() as i32);
        tracing::info!("Double click at screen {:?} -> tile {:?}", screen, tile_i);

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
                button: MouseButton::Left,
                is_double_click: true,
            });
            tracing::info!("Double Clicked Entity {:?}", entity);
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
                        tracing::info!("Showing self profile (Double Click)");
                    } else if item.is_some() {
                        outbox.send(&Pickup {
                            destination_slot: 0,
                            source_point: (position.x as u16, position.y as u16),
                        });
                        tracing::info!("Sent Pickup (Double Click)");
                    } else if player.is_some() {
                        // Optimistically clear previous other player data
                        profile_events.write(ShowSelfProfileEvent::OtherRequested);
                        outbox.send(&Click::TargetId(entity_id.id));
                        tracing::info!("Sent Click (Double Click Player): {}", entity_id.id);
                    } else if npc.is_some() {
                        outbox.send(&Click::TargetId(entity_id.id));
                        tracing::info!("Sent Click (Double Click NPC): {}", entity_id.id);
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
