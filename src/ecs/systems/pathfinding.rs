use bevy::prelude::*;
use pathfinding::prelude::astar;
use std::collections::HashSet;

use crate::ecs::collision::{MapCollisionData, WallCollisionTable, can_walk_to};
use crate::ecs::components::{
    Direction, GameMap, ItemSprite, LocalPlayer, MovementTween, NPC, PathTarget, PathfindingState,
    Player, Position, occupied_tile,
};
use crate::ecs::spell_casting::SpellCastingState;
use crate::events::{
    ClickSource, EntityClickEvent, InputSource, InteractionIntentAction, InteractionIntentEvent,
    InteractionTargetKind, PlayerAction, TileClickEvent,
};
use crate::plugins::input::InputTimer;

const STEP_COST_CLEAR: u32 = 2;
const STEP_COST_NEAR_ENTITY: u32 = 3;

pub fn pathfinding_target_system(
    mut commands: Commands,
    mut tile_clicks: MessageReader<TileClickEvent>,
    player_query: Query<(Entity, &Position), With<LocalPlayer>>,
    map_query: Query<&GameMap>,
    collision_table: Option<Res<WallCollisionTable>>,
    map_collision: Option<Res<MapCollisionData>>,
    entity_positions: Query<
        (&Position, Option<&MovementTween>),
        (Or<(With<NPC>, With<Player>)>, Without<LocalPlayer>),
    >,
) {
    let Ok((player_entity, player_pos)) = player_query.single() else {
        return;
    };

    let Ok(map) = map_query.single() else {
        return;
    };

    let occupied_tiles = collect_occupied_tiles(&entity_positions);

    for event in tile_clicks.read() {
        if event.button != MouseButton::Right {
            continue;
        }

        let target_x = event.tile_x.clamp(0, map.width as i32 - 1) as u8;
        let target_y = event.tile_y.clamp(0, map.height as i32 - 1) as u8;

        let start_x = player_pos.x.round() as u8;
        let start_y = player_pos.y.round() as u8;

        if start_x == target_x && start_y == target_y {
            continue;
        }

        let Some(destination) = resolve_walk_target(
            (start_x, start_y),
            (target_x, target_y),
            map.width,
            map.height,
            collision_table.as_deref(),
            map_collision.as_deref(),
            &occupied_tiles,
        ) else {
            continue;
        };

        insert_pathfinding_target(&mut commands, player_entity, destination, None);
    }
}

pub fn resolve_interaction_intents_system(
    spell_casting: Res<SpellCastingState>,
    mut entity_clicks: MessageReader<EntityClickEvent>,
    mut tile_clicks: MessageReader<TileClickEvent>,
    entity_query: Query<(
        &Position,
        Option<&Player>,
        Option<&NPC>,
        Option<&ItemSprite>,
        Option<&LocalPlayer>,
    )>,
    mut interaction_intents: MessageWriter<InteractionIntentEvent>,
) {
    let is_waiting_for_target = spell_casting
        .active_cast
        .as_ref()
        .map(|cast| cast.waiting_for_target)
        .unwrap_or(false);

    if is_waiting_for_target {
        return;
    }

    for event in entity_clicks.read() {
        let is_mobile_short_press = event.source == ClickSource::AndroidShortPress;
        let is_desktop_right_click =
            event.source == ClickSource::DesktopMouse && event.button == MouseButton::Right;

        if (!is_mobile_short_press && !is_desktop_right_click) || event.is_double_click {
            continue;
        }

        let Ok((position, player, npc, item, local_player)) = entity_query.get(event.entity) else {
            continue;
        };

        if local_player.is_some() {
            let player_tile_x = position.x.round() as i32;
            let player_tile_y = position.y.round() as i32;

            if (event.ground_tile_x, event.ground_tile_y) != (player_tile_x, player_tile_y) {
                interaction_intents.write(InteractionIntentEvent {
                    source: event.source,
                    target_kind: InteractionTargetKind::Ground,
                    target_entity: None,
                    tile_x: event.ground_tile_x,
                    tile_y: event.ground_tile_y,
                    action: InteractionIntentAction::WalkToTile,
                });
            }
            continue;
        }

        let tile_x = position.x.round() as i32;
        let tile_y = position.y.round() as i32;

        if item.is_some() {
            interaction_intents.write(InteractionIntentEvent {
                source: event.source,
                target_kind: InteractionTargetKind::Item,
                target_entity: Some(event.entity),
                tile_x,
                tile_y,
                action: InteractionIntentAction::WalkToTile,
            });
            continue;
        }

        if player.is_some() || npc.is_some() {
            interaction_intents.write(InteractionIntentEvent {
                source: event.source,
                target_kind: InteractionTargetKind::Actor,
                target_entity: Some(event.entity),
                tile_x,
                tile_y,
                action: InteractionIntentAction::ApproachAndFace,
            });
        }
    }

    for event in tile_clicks.read() {
        if event.source != ClickSource::AndroidShortPress || event.button != MouseButton::Left {
            continue;
        }

        interaction_intents.write(InteractionIntentEvent {
            source: event.source,
            target_kind: InteractionTargetKind::Ground,
            target_entity: None,
            tile_x: event.tile_x,
            tile_y: event.tile_y,
            action: InteractionIntentAction::WalkToTile,
        });
    }
}

pub fn consume_interaction_intents_system(
    mut commands: Commands,
    mut interaction_intents: MessageReader<InteractionIntentEvent>,
    player_query: Query<(Entity, &Position), With<LocalPlayer>>,
    map_query: Query<&GameMap>,
    collision_table: Option<Res<WallCollisionTable>>,
    map_collision: Option<Res<MapCollisionData>>,
    entity_positions: Query<
        (&Position, Option<&MovementTween>),
        (Or<(With<NPC>, With<Player>)>, Without<LocalPlayer>),
    >,
) {
    let Ok((player_entity, player_pos)) = player_query.single() else {
        return;
    };

    let Ok(map) = map_query.single() else {
        return;
    };

    let start = (player_pos.x.round() as u8, player_pos.y.round() as u8);
    let occupied_tiles = collect_occupied_tiles(&entity_positions);

    for event in interaction_intents.read() {
        let target_x = event.tile_x.clamp(0, map.width as i32 - 1) as u8;
        let target_y = event.tile_y.clamp(0, map.height as i32 - 1) as u8;

        match event.action {
            InteractionIntentAction::WalkToTile => {
                let Some(destination) = resolve_walk_target(
                    start,
                    (target_x, target_y),
                    map.width,
                    map.height,
                    collision_table.as_deref(),
                    map_collision.as_deref(),
                    &occupied_tiles,
                ) else {
                    continue;
                };

                if start == destination {
                    continue;
                }

                insert_pathfinding_target(&mut commands, player_entity, destination, None);
            }
            InteractionIntentAction::ApproachAndFace => {
                let Some(destination) = choose_best_approach_tile(
                    start,
                    (target_x, target_y),
                    map.width,
                    map.height,
                    collision_table.as_deref(),
                    map_collision.as_deref(),
                    &occupied_tiles,
                ) else {
                    continue;
                };

                insert_pathfinding_target(
                    &mut commands,
                    player_entity,
                    destination,
                    Some((target_x, target_y)),
                );
            }
        }
    }
}

pub fn pathfinding_execution_system(
    time: Res<Time>,
    mut commands: Commands,
    input_timer: Res<InputTimer>,
    mut player_query: Query<
        (
            Entity,
            &Position,
            &mut Direction,
            Option<&MovementTween>,
            &mut PathfindingState,
        ),
        With<LocalPlayer>,
    >,
    map_query: Query<&GameMap>,
    collision_table: Option<Res<WallCollisionTable>>,
    map_collision: Option<Res<MapCollisionData>>,
    entity_positions: Query<
        (&Position, Option<&MovementTween>),
        (Or<(With<NPC>, With<Player>)>, Without<LocalPlayer>),
    >,
    mut player_actions: MessageWriter<PlayerAction>,
    spell_casting: Res<SpellCastingState>,
) {
    let Ok((player_entity, player_pos, mut player_direction, tween, mut pathfinding)) =
        player_query.single_mut()
    else {
        return;
    };

    if let Some(ref cast) = spell_casting.active_cast {
        if !cast.waiting_for_target {
            // Wait for spell chant to finish before taking the next pathfinding step
            return;
        }
    }

    let Ok(map) = map_query.single() else {
        return;
    };

    if let Some(ref mut timer) = pathfinding.retry_timer {
        timer.tick(time.delta());
        if !timer.just_finished() {
            return;
        }
        pathfinding.retry_timer = None;
    }

    if tween.is_some() {
        return;
    }

    if !input_timer.walk_cd_finished() {
        return;
    }

    let PathTarget::Tile {
        x: target_x,
        y: target_y,
    } = pathfinding.target;
    let face_after = pathfinding.face_after;
    let start_x = player_pos.x.round() as u8;
    let start_y = player_pos.y.round() as u8;

    if start_x == target_x && start_y == target_y {
        if let Some(face_tile) = face_after {
            if let Some(direction) = direction_toward((start_x, start_y), face_tile) {
                if *player_direction != direction {
                    *player_direction = direction;
                    player_actions.write(PlayerAction::Turn {
                        direction,
                        source: InputSource::Pathfinding,
                    });
                }
            }
        }

        commands.entity(player_entity).remove::<PathfindingState>();
        return;
    }

    let occupied_tiles = collect_occupied_tiles(&entity_positions);

    let path_result = find_path(
        (start_x, start_y),
        (target_x, target_y),
        map.width,
        map.height,
        collision_table.as_deref(),
        map_collision.as_deref(),
        &occupied_tiles,
    );

    match path_result {
        Some(path) if path.len() >= 2 => {
            let next_step = path[1];
            let dx = next_step.0 as i32 - start_x as i32;
            let dy = next_step.1 as i32 - start_y as i32;

            let direction = match (dx, dy) {
                (0, -1) => Direction::Up,
                (1, 0) => Direction::Right,
                (0, 1) => Direction::Down,
                (-1, 0) => Direction::Left,
                _ => {
                    commands.entity(player_entity).remove::<PathfindingState>();
                    return;
                }
            };

            player_actions.write(PlayerAction::Walk {
                direction,
                source: InputSource::Pathfinding,
            });
        }
        _ => {
            pathfinding.retry_timer = Some(Timer::from_seconds(1.0, TimerMode::Once));
        }
    }
}

fn find_path(
    start: (u8, u8),
    goal: (u8, u8),
    map_width: u8,
    map_height: u8,
    collision_table: Option<&WallCollisionTable>,
    map_collision: Option<&MapCollisionData>,
    occupied_tiles: &HashSet<(u8, u8)>,
) -> Option<Vec<(u8, u8)>> {
    find_path_with_cost(
        start,
        goal,
        map_width,
        map_height,
        collision_table,
        map_collision,
        occupied_tiles,
    )
    .map(|(path, _)| path)
}

fn find_path_with_cost(
    start: (u8, u8),
    goal: (u8, u8),
    map_width: u8,
    map_height: u8,
    collision_table: Option<&WallCollisionTable>,
    map_collision: Option<&MapCollisionData>,
    occupied_tiles: &HashSet<(u8, u8)>,
) -> Option<(Vec<(u8, u8)>, u32)> {
    let result = astar(
        &start,
        |&(x, y)| {
            let mut neighbors = Vec::new();
            let directions = [(0, -1), (1, 0), (0, 1), (-1, 0)];

            for (dx, dy) in directions {
                let nx = x as i32 + dx;
                let ny = y as i32 + dy;

                if nx >= 0 && nx < map_width as i32 && ny >= 0 && ny < map_height as i32 {
                    let nx_u8 = nx as u8;
                    let ny_u8 = ny as u8;

                    let is_occupied = occupied_tiles.contains(&(nx_u8, ny_u8));

                    if can_walk_to(nx_u8, ny_u8, collision_table, map_collision) && !is_occupied {
                        let step_cost =
                            if is_tile_adjacent_to_occupied((nx_u8, ny_u8), occupied_tiles) {
                                STEP_COST_NEAR_ENTITY
                            } else {
                                STEP_COST_CLEAR
                            };

                        neighbors.push(((nx_u8, ny_u8), step_cost));
                    }
                }
            }

            neighbors
        },
        |&(x, y)| {
            let dx = (x as i32 - goal.0 as i32).abs();
            let dy = (y as i32 - goal.1 as i32).abs();
            (dx + dy) as u32 * STEP_COST_CLEAR
        },
        |&pos| pos == goal,
    );

    result
}

fn choose_best_approach_tile(
    start: (u8, u8),
    target: (u8, u8),
    map_width: u8,
    map_height: u8,
    collision_table: Option<&WallCollisionTable>,
    map_collision: Option<&MapCollisionData>,
    occupied_tiles: &HashSet<(u8, u8)>,
) -> Option<(u8, u8)> {
    let neighbors = [
        (0_i32, -1_i32),
        (1_i32, 0_i32),
        (0_i32, 1_i32),
        (-1_i32, 0_i32),
    ];
    let mut best: Option<(u32, usize, (u8, u8))> = None;

    for (dx, dy) in neighbors {
        let nx = target.0 as i32 + dx;
        let ny = target.1 as i32 + dy;

        if nx < 0 || nx >= map_width as i32 || ny < 0 || ny >= map_height as i32 {
            continue;
        }

        let candidate = (nx as u8, ny as u8);
        if candidate != start && occupied_tiles.contains(&candidate) {
            continue;
        }

        if !can_walk_to(candidate.0, candidate.1, collision_table, map_collision) {
            continue;
        }

        if candidate == start {
            return Some(candidate);
        }

        let Some((path, path_cost)) = find_path_with_cost(
            start,
            candidate,
            map_width,
            map_height,
            collision_table,
            map_collision,
            occupied_tiles,
        ) else {
            continue;
        };

        let candidate_steps = path.len();
        let replace_best = best
            .map(|(best_cost, best_steps, best_tile)| {
                path_cost < best_cost
                    || (path_cost == best_cost
                        && (candidate_steps < best_steps
                            || (candidate_steps == best_steps && candidate < best_tile)))
            })
            .unwrap_or(true);

        if replace_best {
            best = Some((path_cost, candidate_steps, candidate));
        }
    }

    best.map(|(_, _, tile)| tile)
}

fn resolve_walk_target(
    start: (u8, u8),
    target: (u8, u8),
    map_width: u8,
    map_height: u8,
    collision_table: Option<&WallCollisionTable>,
    map_collision: Option<&MapCollisionData>,
    occupied_tiles: &HashSet<(u8, u8)>,
) -> Option<(u8, u8)> {
    if start == target {
        return Some(target);
    }

    if occupied_tiles.contains(&target) {
        return choose_best_approach_tile(
            start,
            target,
            map_width,
            map_height,
            collision_table,
            map_collision,
            occupied_tiles,
        );
    }

    can_walk_to(target.0, target.1, collision_table, map_collision).then_some(target)
}

fn insert_pathfinding_target(
    commands: &mut Commands,
    player_entity: Entity,
    destination: (u8, u8),
    face_after: Option<(u8, u8)>,
) {
    commands.entity(player_entity).insert(PathfindingState {
        target: PathTarget::Tile {
            x: destination.0,
            y: destination.1,
        },
        face_after,
        retry_timer: None,
    });
}

fn collect_occupied_tiles(
    entity_positions: &Query<
        (&Position, Option<&MovementTween>),
        (Or<(With<NPC>, With<Player>)>, Without<LocalPlayer>),
    >,
) -> HashSet<(u8, u8)> {
    entity_positions
        .iter()
        .map(|(position, tween)| occupied_tile(position, tween))
        .collect()
}

fn is_tile_adjacent_to_occupied(tile: (u8, u8), occupied_tiles: &HashSet<(u8, u8)>) -> bool {
    const CARDINAL_NEIGHBORS: [(i32, i32); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];

    CARDINAL_NEIGHBORS.iter().any(|(dx, dy)| {
        let neighbor_x = tile.0 as i32 + dx;
        let neighbor_y = tile.1 as i32 + dy;

        if neighbor_x < 0 || neighbor_y < 0 {
            return false;
        }

        occupied_tiles.contains(&(neighbor_x as u8, neighbor_y as u8))
    })
}

fn direction_toward(from: (u8, u8), to: (u8, u8)) -> Option<Direction> {
    let dx = to.0 as i32 - from.0 as i32;
    let dy = to.1 as i32 - from.1 as i32;

    match (dx, dy) {
        (0, -1) => Some(Direction::Up),
        (1, 0) => Some(Direction::Right),
        (0, 1) => Some(Direction::Down),
        (-1, 0) => Some(Direction::Left),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn approach_prefers_current_tile_when_already_adjacent() {
        let occupied_tiles = HashSet::from([(4_u8, 4_u8)]);

        let result = choose_best_approach_tile((4, 3), (4, 4), 10, 10, None, None, &occupied_tiles);

        assert_eq!(result, Some((4, 3)));
    }

    #[test]
    fn approach_selects_reachable_adjacent_tile() {
        let occupied_tiles = HashSet::from([(4_u8, 4_u8), (4_u8, 3_u8)]);

        let result = choose_best_approach_tile((2, 4), (4, 4), 10, 10, None, None, &occupied_tiles);

        assert_eq!(result, Some((3, 4)));
    }

    #[test]
    fn walk_target_uses_adjacent_tile_when_destination_is_occupied() {
        let occupied_tiles = HashSet::from([(4_u8, 4_u8)]);

        let result = resolve_walk_target((2, 4), (4, 4), 10, 10, None, None, &occupied_tiles);

        assert_eq!(result, Some((3, 4)));
    }

    #[test]
    fn moving_entities_reserve_their_tween_destination_tile() {
        let position = Position::new(2.0, 2.0);
        let tween = MovementTween {
            start: Vec2::new(2.0, 2.0),
            end: Vec2::new(3.0, 2.0),
            elapsed: 0.2,
            duration: 0.5,
        };

        assert_eq!(occupied_tile(&position, Some(&tween)), (3, 2));
    }

    #[test]
    fn crowd_penalty_prefers_cleaner_route() {
        let occupied_tiles = HashSet::from([
            (1_u8, 0_u8),
            (2_u8, 0_u8),
            (3_u8, 0_u8),
            (4_u8, 0_u8),
            (5_u8, 0_u8),
        ]);

        let path = find_path((0, 1), (6, 1), 7, 3, None, None, &occupied_tiles)
            .expect("path should be found");

        assert_eq!(
            path,
            vec![
                (0, 1),
                (0, 2),
                (1, 2),
                (2, 2),
                (3, 2),
                (4, 2),
                (5, 2),
                (6, 2),
                (6, 1)
            ]
        );
    }

    #[test]
    fn direction_toward_returns_cardinal_direction() {
        assert_eq!(direction_toward((5, 5), (5, 4)), Some(Direction::Up));
        assert_eq!(direction_toward((5, 5), (6, 5)), Some(Direction::Right));
        assert_eq!(direction_toward((5, 5), (5, 6)), Some(Direction::Down));
        assert_eq!(direction_toward((5, 5), (4, 5)), Some(Direction::Left));
        assert_eq!(direction_toward((5, 5), (6, 6)), None);
    }
}
