//! ECS Systems organized by domain
//!
//! Systems are grouped into logical modules and execute in a well-defined order
//! managed by [`GameSet`].

mod camera;
mod chat;
mod effects;
mod entities;
mod interruption;
mod map;
mod movement;
mod pathfinding;
mod rendering;

pub use camera::*;
pub use chat::*;
pub use effects::*;
pub use entities::*;
pub use interruption::*;
pub use map::*;
pub use movement::*;
pub use pathfinding::*;
pub use rendering::*;

use bevy::prelude::*;

/// Core game loop system sets with explicit ordering.
///
/// The execution order is:
/// 1. **EventProcessing** - Read network/input events into ECS-friendly form
/// 2. **Spawning** - Spawn new entities from events (including removal)
/// 3. **Movement** - Process movement requests and start tweens
/// 4. **Physics** - Advance tweens, apply positions
/// 5. **Animation** - Update animation frames
/// 6. **Camera** - Follow targets and sync camera position
/// 7. **RenderSync** - Push ECS state to GPU renderers
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum GameSet {
    /// Process incoming events (network packets, input actions)
    EventProcessing,
    /// Spawn new entities from DisplayPlayer, DisplayEntities events
    Spawning,
    /// Process movement inputs, insert movement tweens
    Movement,
    /// Advance tweens, apply interpolated positions
    Physics,
    /// Tick animation timers, advance frames
    Animation,
    /// Camera follow logic, position sync
    Camera,
    /// Sync ECS components to GPU renderer state
    RenderSync,
}

/// Configure the system set ordering for the game loop
pub fn configure_game_sets(app: &mut App) {
    app.configure_sets(
        Update,
        (
            GameSet::EventProcessing,
            GameSet::Spawning.after(GameSet::EventProcessing),
            GameSet::Movement.after(GameSet::Spawning),
            GameSet::Physics.after(GameSet::Movement),
            GameSet::Animation.after(GameSet::Physics),
            GameSet::Camera.after(GameSet::Animation),
            GameSet::RenderSync.after(GameSet::Camera),
        ),
    );
}
