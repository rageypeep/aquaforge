//! Underwater atmosphere: camera, lighting, fog, and the sea-surface plane.

use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::light::ShadowFilteringMethod;
use bevy::pbr::{DistanceFog, FogFalloff, ScreenSpaceAmbientOcclusion};
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
            .add_plugins((
                lighting::LightingPlugin,
                bevy::pbr::ScreenSpaceAmbientOcclusionPlugin,
            ))
            .add_systems(Startup, (spawn_camera, spawn_water_surface));
    }
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Hdr,
        // `TonyMcMapface` is Bevy's neutral HDR→LDR transform of choice; it
        // preserves hue and avoids the heavy hue-shift of ACES, which matters
        // here because we're leaning on saturated blues and greens.
        Tonemapping::TonyMcMapface,
        DebandDither::Enabled,
        // Soft-but-cheap PCF; good match for the non-temporal renderer.
        ShadowFilteringMethod::Gaussian,
        // Screen-space AO grounds block corners and contact points. The
        // plugin gracefully no-ops on GPUs without the required storage
        // texture limits, so it's safe to enable unconditionally.
        ScreenSpaceAmbientOcclusion::default(),
        Transform::from_xyz(0.0, WATER_LEVEL - 4.0, 20.0)
            .looking_at(Vec3::new(0.0, WATER_LEVEL - 6.0, 0.0), Vec3::Y),
        DistanceFog {
            color: Color::srgb(0.03, 0.18, 0.3),
            falloff: FogFalloff::Exponential { density: 0.035 },
            // Tint in-scattering along the sun direction so beams of murky
            // light seem to come from above. The colour is deliberately
            // dim — PBR in-scattering adds to the fog, it doesn't replace
            // it.
            directional_light_color: Color::srgba(0.55, 0.85, 1.0, 0.35),
            directional_light_exponent: 25.0,
        },
        Bloom::NATURAL,
        Name::new("Player Camera"),
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
        // Glassy but not mirror-perfect — a little surface chop is implied.
        perceptual_roughness: 0.15,
        metallic: 0.0,
        // Water has an IOR of ~1.33, so its Fresnel reflectance at normal
        // incidence is ~0.02. Bevy's `reflectance` remaps that via a 0..1
        // slider where 0.5 ≈ 4% reflectance, so ~0.35 is closer to physical.
        reflectance: 0.35,
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
