use bevy::prelude::*;

use self::game::GameWorldRenderPlugin;

pub mod game;
// pub mod minimap;

pub struct GameRenderPlugin;

impl Plugin for GameRenderPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(GameWorldRenderPlugin);
    }
}
