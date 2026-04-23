//! AquaForge: an underwater, Minecraft-style voxel game built on Bevy 0.18.

use bevy::diagnostic::FrameTimeDiagnosticsPlugin;
use bevy::prelude::*;

mod game;
mod rendering;
mod systems;
mod utils;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "AquaForge".to_string(),
                resolution: (1280u32, 720u32).into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(FrameTimeDiagnosticsPlugin::default())
        .add_plugins((
            rendering::AtmospherePlugin,
            game::GamePlugin,
            systems::ControlsPlugin,
        ))
        .run();
}
