//! Per-frame light source gathering for the darkness overlay.

use crate::ecs::components::{Lantern, Position};
use crate::{Camera, DarknessState};
use bevy::prelude::*;
use rendering::scene::darkness::LightSource;
use rendering::scene::tile_to_screen;

/// Gathers lantern entities into the per-frame light source list.
pub fn lighting_gather_system(
    darkness: Option<ResMut<DarknessState>>,
    camera: Option<Res<Camera>>,
    query: Query<(&Position, &Lantern)>,
) {
    let (Some(mut darkness), Some(camera)) = (darkness, camera) else {
        return;
    };

    darkness.sources.clear();
    if !darkness.is_dark_map {
        return;
    }

    let window = Vec2::new(camera.camera.camera.width, camera.camera.camera.height);
    let zoom = camera.camera.zoom();
    let cam_pos = camera.camera.position();
    for (position, lantern) in &query {
        let mask_layer = match lantern.size {
            0 => continue,
            1 => 0, // small
            _ => 1, // large
        };

        let screen = tile_to_screen(
            Vec2::new(position.x, position.y),
            cam_pos,
            window,
            zoom,
        );

        darkness.sources.push(LightSource {
            screen_x: screen.x,
            screen_y: screen.y,
            mask_layer,
        });
    }
}
