use bevy::prelude::*;
use pathfinding::prelude::astar;
use std::collections::HashSet;

use crate::ecs::collision::{MapCollisionData, WallCollisionTable, can_walk_to};
use crate::ecs::components::{
    Direction, GameMap, LocalPlayer, MovementTween, NPC, PathTarget, PathfindingState, Player,
    Position,
};
use crate::events::{InputSource, PlayerAction, TileClickEvent};
use crate::plugins::input::InputTimer;

pub fn pathfinding_target_system(
    mut commands: Commands,
    mut tile_clicks: MessageReader<TileClickEvent>,
    player_query: Query<(Entity, &Position), With<LocalPlayer>>,
    map_query: Query<&GameMap>,
) {
    let Ok((player_entity, player_pos)) = player_query.single() else {
        return;
    };

    let Ok(map) = map_query.single() else {
        return;
    };

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

        commands.entity(player_entity).remove::<PathfindingState>();

        commands.entity(player_entity).insert(PathfindingState {
            target: PathTarget::Tile {
                x: target_x,
                y: target_y,
            },
            retry_timer: None,
        });
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
            Option<&MovementTween>,
            &mut PathfindingState,
        ),
        With<LocalPlayer>,
    >,
    map_query: Query<&GameMap>,
    collision_table: Option<Res<WallCollisionTable>>,
    map_collision: Option<Res<MapCollisionData>>,
    entity_positions: Query<&Position, (Or<(With<NPC>, With<Player>)>, Without<LocalPlayer>)>,
    mut player_actions: MessageWriter<PlayerAction>,
) {
    let Ok((player_entity, player_pos, tween, mut pathfinding)) = player_query.single_mut() else {
        return;
    };

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
    let start_x = player_pos.x.round() as u8;
    let start_y = player_pos.y.round() as u8;

    if start_x == target_x && start_y == target_y {
        commands.entity(player_entity).remove::<PathfindingState>();
        return;
    }

    let occupied_tiles: HashSet<(u8, u8)> = entity_positions
        .iter()
        .map(|pos| (pos.x.round() as u8, pos.y.round() as u8))
        .collect();

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
                direction: direction as u8,
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

                    let is_occupied =
                        occupied_tiles.contains(&(nx_u8, ny_u8)) && (nx_u8, ny_u8) != goal;

                    if can_walk_to(nx_u8, ny_u8, collision_table, map_collision) && !is_occupied {
                        neighbors.push(((nx_u8, ny_u8), 1));
                    }
                }
            }

            neighbors
        },
        |&(x, y)| {
            let dx = (x as i32 - goal.0 as i32).abs();
            let dy = (y as i32 - goal.1 as i32).abs();
            (dx + dy) as u32
        },
        |&pos| pos == goal,
    );

    result.map(|(path, _cost)| path)
}
