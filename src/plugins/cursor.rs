use bevy::prelude::*;
use glam::Vec2;
use rendering::scene::utils::screen_to_iso_tile;

use crate::resources::ZoomState;
use crate::webui::plugin::CursorPosition;
use crate::{Camera, WindowSurface};

pub struct CursorPlugin;

impl Plugin for CursorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LastLoggedTile>()
            .add_systems(Update, log_cursor_tile_system);
    }
}

#[derive(Resource, Default, Debug, Clone, Copy)]
struct LastLoggedTile(Option<(i32, i32)>);

fn log_cursor_tile_system(
    cursor: Res<CursorPosition>,
    camera: Res<Camera>,
    window_surface: Option<NonSend<WindowSurface>>,
    zoom_state: Option<Res<ZoomState>>,
    mut last: ResMut<LastLoggedTile>,
) {
    if !cursor.is_changed() {
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

    if win_size.x <= 0.0 || win_size.y <= 0.0 {
        return;
    }

    let cursor_scale = zoom_state.cursor_to_render_scale();
    let screen = Vec2::new(cursor.x * cursor_scale, cursor.y * cursor_scale);
    let tile = screen_to_iso_tile(screen, cam_pos, win_size, zoom);
    // Use floor so each isometric diamond maps to a single integer tile index without quadrant drift.
    let tile_i = (tile.x.floor() as i32, tile.y.floor() as i32);

    if last.0 != Some(tile_i) {
        tracing::debug!(
            screen_x = screen.x,
            screen_y = screen.y,
            cam_x = cam_pos.x,
            cam_y = cam_pos.y,
            zoom,
            win_w = win_size.x,
            win_h = win_size.y,
            tile_fx = tile.x,
            tile_fy = tile.y,
            tile_ix = tile_i.0,
            tile_iy = tile_i.1,
            "cursor debug"
        );
        last.0 = Some(tile_i);
        tracing::info!(tx = tile_i.0, ty = tile_i.1, "cursor tile");
    }
}
