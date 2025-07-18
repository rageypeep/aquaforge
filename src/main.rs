// src/main.rs

use bevy::prelude::*;

// Modules go here
mod game;
mod utils;
mod systems;
mod rendering;

fn main() {
    App::new()
        .insert_resource(Msaa::Sample4)
        .add_plugins(DefaultPlugins)
        .add_plugins((
            game::GamePlugin,
            systems::ControlsPlugin, // this will add all control systems (including mouse look setup)
        ))
        .add_systems(Startup, setup_camera)
        .run();
}

// Basic 3D camera setup
fn setup_camera(mut commands: Commands) {
    commands.spawn(Camera3dBundle {
        transform: Transform::from_xyz(0.0, 25.0, 40.0)
            .looking_at(Vec3::new(0.0, 0.0, 0.0), Vec3::Y),
        ..default()
    });
}