//! Callback wiring for Slint UI.
//!
//! Each module handles a category of UI callbacks, wiring them to channels
//! that communicate with the Bevy ECS.

mod game_callbacks;
mod input_callbacks;
mod login_callbacks;
mod network_callbacks;
mod settings_callbacks;

pub use game_callbacks::wire_game_callbacks;
pub use input_callbacks::wire_input_callbacks;
pub use login_callbacks::wire_login_callbacks;
pub use network_callbacks::wire_network_callbacks;
pub use settings_callbacks::wire_settings_callbacks;
