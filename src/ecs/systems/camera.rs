//! Camera systems

use super::super::components::*;
use crate::{Camera, RendererState};
use bevy::prelude::*;
use tracing::debug;

/// Initializes the game world with a camera entity.
/// Runs once at startup.
pub fn initialize_game_world(mut game_initialized: Local<bool>, mut commands: Commands) {
    if !*game_initialized {
        commands.spawn(CameraBundle {
            camera: GameCamera,
            position: Position { x: 1., y: 1. },
        });
        *game_initialized = true;
    }
}

/// Makes the ECS camera follow the local player (CameraTarget).
pub fn camera_follow_system(
    target_query: Query<&Position, (With<LocalPlayer>, With<CameraTarget>)>,
    mut camera_query: Query<&mut Position, (With<GameCamera>, Without<CameraTarget>)>,
) {
    let mut targets = target_query.iter();
    let first = targets.next();
    let second = targets.next();

    // Only follow if there's exactly one target
    if let (Some(target_pos), None) = (first, second) {
        if let Ok(mut camera_pos) = camera_query.single_mut() {
            let before = (camera_pos.x, camera_pos.y);
            camera_pos.x = target_pos.x;
            camera_pos.y = target_pos.y;

            if (before.0 - camera_pos.x).abs() > f32::EPSILON
                || (before.1 - camera_pos.y).abs() > f32::EPSILON
            {
                debug!(
                    cam_x = camera_pos.x,
                    cam_y = camera_pos.y,
                    "Camera follow updated"
                );
            }
        }
    }
}

/// Syncs the ECS camera position to the GPU camera uniform.
pub fn camera_position_sync(
    mut camera: ResMut<Camera>,
    renderer: Res<RendererState>,
    camera_query: Query<&Position, (Changed<Position>, With<GameCamera>)>,
) {
    for position in camera_query.iter() {
        camera
            .camera
            .set_position(&renderer.queue, position.x, position.y);
    }
}

/// Syncs the X-ray size setting to the camera shader.
pub fn camera_xray_sync(
    mut camera: ResMut<Camera>,
    renderer: Res<RendererState>,
    settings: Res<crate::settings_types::Settings>,
) {
    if settings.is_changed() {
        camera.camera.set_xray_size(
            &renderer.queue,
            settings.graphics.xray_size.to_shader_multiplier(),
        );
    }
}
