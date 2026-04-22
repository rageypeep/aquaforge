//! Underwater atmosphere: camera, lighting, fog, and the sea-surface plane.

use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::light::ShadowFilteringMethod;
use bevy::pbr::{DistanceFog, FogFalloff, ScreenSpaceAmbientOcclusion};
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::render::view::{Hdr, Msaa};

use crate::game::chunk::CHUNK_SIZE;
use crate::game::world::{StreamingConfig, WATER_LEVEL};

use self::headlights::HeadlightsPlugin;
use self::water::{WaterMaterial, WaterMaterialExt, WaterMaterialPlugin};

pub mod headlights;
pub mod lighting;
pub mod shaders;
pub mod ui;
pub mod water;

/// Plugin that installs the underwater look-and-feel and the player camera.
pub struct AtmospherePlugin;

/// Colour of the water volume; used for fog and (semi-opaquely) the surface.
pub const WATER_COLOR: Color = Color::srgb(0.04, 0.22, 0.34);

/// Minimum edge length of the sea-surface plane, in world units. The
/// actual size scales up with `StreamingConfig::horizontal_radius` so
/// the plane always covers the load horizon even with large radii.
const MIN_SEA_SURFACE_SIZE: f32 = 512.0;

/// Safety multiplier applied to the streaming diameter so the surface
/// still reads as infinite when the camera is near a load-ring corner.
const SEA_SURFACE_SAFETY_FACTOR: f32 = 1.5;

impl Plugin for AtmospherePlugin {
    fn build(&self, app: &mut App) {
        // `ScreenSpaceAmbientOcclusionPlugin` is already registered by
        // `PbrPlugin` (part of `DefaultPlugins`), so we only need to attach
        // the `ScreenSpaceAmbientOcclusion` component to the camera.
        app.insert_resource(ClearColor(WATER_COLOR))
<<<<<<< HEAD
            .add_plugins((lighting::LightingPlugin, WaterMaterialPlugin, HeadlightsPlugin))
||||||| parent of 54b60e8 (Replace fly-cam with a swimming player: swept-AABB collision + oxygen)
            .add_plugins((lighting::LightingPlugin, WaterMaterialPlugin))
=======
            .add_plugins((lighting::LightingPlugin, WaterMaterialPlugin, ui::HudPlugin))
>>>>>>> 54b60e8 (Replace fly-cam with a swimming player: swept-AABB collision + oxygen)
            .add_systems(Startup, (spawn_camera, spawn_water_surface))
            .add_systems(Update, follow_camera_on_xz);
    }
}

/// Marker for entities whose XZ position should follow the camera so
/// they stay centred as the streaming world scrolls.
#[derive(Component)]
struct FollowCameraXZ;

fn follow_camera_on_xz(
    cameras: Query<&GlobalTransform, (With<Camera3d>, Without<FollowCameraXZ>)>,
    mut followers: Query<&mut Transform, With<FollowCameraXZ>>,
) {
    let Ok(cam) = cameras.single() else {
        return;
    };
    let cam_xz = cam.translation();
    for mut t in followers.iter_mut() {
        t.translation.x = cam_xz.x;
        t.translation.z = cam_xz.z;
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
        // SSAO requires MSAA disabled. The blocky voxel style means we're
        // not losing much by dropping multisampling — the silhouettes are
        // axis-aligned quads.
        Msaa::Off,
        // Screen-space AO grounds block corners and contact points. On GPUs
        // that don't meet the storage-texture limit the plugin logs a
        // warning and skips; it never crashes.
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
    mut materials: ResMut<Assets<WaterMaterial>>,
    config: Res<StreamingConfig>,
) {
    // Derive the plane size from the streaming radius so raising
    // `horizontal_radius` doesn't reveal a hard water edge at the
    // horizon. Clamp to `MIN_SEA_SURFACE_SIZE` so small radii still get
    // a visually infinite surface.
    let load_diameter = ((2 * config.horizontal_radius + 1) * CHUNK_SIZE as i32) as f32;
    let size = (load_diameter * SEA_SURFACE_SAFETY_FACTOR).max(MIN_SEA_SURFACE_SIZE);

    // The vertex shader displaces per-vertex; we need enough subdivisions
    // that the wavelength is well-sampled. Our shortest wave-vector is
    // ~0.35 rad/m (wavelength ≈ 18 m), so 6 m spacing gives ~3 vertices per
    // wavelength — enough to read as smooth motion without choking llvmpipe.
    let spacing: f32 = 6.0;
    let subdivisions = (size / spacing).round().max(2.0) as u32;

    let mesh = meshes.add(
        Plane3d::default()
            .mesh()
            .size(size, size)
            .subdivisions(subdivisions),
    );

    let material = materials.add(WaterMaterial {
        base: StandardMaterial {
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
        },
        extension: WaterMaterialExt::default(),
    });

    commands.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(material),
        Transform::from_translation(Vec3::new(0.0, WATER_LEVEL, 0.0)),
        FollowCameraXZ,
        Name::new("Sea Surface"),
    ));
}
