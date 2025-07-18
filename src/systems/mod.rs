// src/systems/mod.rs

use bevy::prelude::*;

pub mod input;
pub use input::ControlsPlugin;
pub struct GamePlugin;


impl Plugin for GamePlugin {
    fn build(&self, _app: &mut App) {
        // Register systems here later!
    }
}
