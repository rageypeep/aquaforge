//! Underwater atmosphere: camera, lighting, fog, and the sea-surface plane.

use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::render::view::Hdr;

use crate::game::chunk::CHUNK_SIZE;
use crate::game::world::{WATER_LEVEL, WORLD_CHUNKS_XZ};

pub mod lighting;
pub mod shaders;
pub mod ui;

/// Plugin that installs the underwater look-and-feel and the player camera.
pub struct AtmospherePlugin;

/// Colour of the water volume; used for fog and (semi-opaquely) the surface.
pub const WATER_COLOR: Color = Color::srgb(0.04, 0.22, 0.34);

impl Plugin for AtmospherePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ClearColor(WATER_COLOR))
            .insert_resource(GlobalAmbientLight {
                color: Color::srgb(0.45, 0.7, 0.9),
                brightness: 220.0,
                ..default()
            })
            .add_systems(Startup, (spawn_camera, spawn_sun, spawn_water_surface));
    }
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Hdr,
        Transform::from_xyz(0.0, WATER_LEVEL - 4.0, 20.0)
            .looking_at(Vec3::new(0.0, WATER_LEVEL - 6.0, 0.0), Vec3::Y),
        DistanceFog {
            color: Color::srgb(0.03, 0.18, 0.3),
            falloff: FogFalloff::Exponential { density: 0.035 },
            ..default()
        },
        Bloom::NATURAL,
        Name::new("Player Camera"),
    ));
}

fn spawn_sun(mut commands: Commands) {
    // Sun-through-water: a cool directional light. Shadows are disabled to
    // keep the base scene cheap on low-end GPUs.
    commands.spawn((
        DirectionalLight {
            color: Color::srgb(0.85, 0.95, 1.0),
            illuminance: 9000.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_xyz(30.0, 80.0, 30.0).looking_at(Vec3::ZERO, Vec3::Y),
        Name::new("Sun"),
    ));
}

fn spawn_water_surface(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let size = (WORLD_CHUNKS_XZ as f32) * (CHUNK_SIZE as f32) * 4.0;

    let mesh = meshes.add(Plane3d::default().mesh().size(size, size));
    let material = materials.add(StandardMaterial {
        base_color: Color::srgba(0.25, 0.55, 0.8, 0.55),
        alpha_mode: AlphaMode::Blend,
        perceptual_roughness: 0.2,
        metallic: 0.0,
        reflectance: 0.5,
        double_sided: true,
        cull_mode: None,
        ..default()
    });

    commands.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(material),
        Transform::from_translation(Vec3::new(0.0, WATER_LEVEL, 0.0)),
        Name::new("Sea Surface"),
    ));
}
