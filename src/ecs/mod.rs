//! Entity Component System module
//!
//! Contains components, systems, and plugins for the game's ECS architecture.
//! Systems are organized into modules by domain and execute in a well-defined
//! order managed by [`systems::GameSet`].

pub mod animation;
pub mod collision;
pub mod components;
pub mod hotbar;
pub mod interaction;
pub mod plugin;
pub mod spell_casting;
pub mod systems;

// Re-export commonly used items
pub use plugin::GamePlugin;
pub use systems::GameSet;
