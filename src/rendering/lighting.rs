//! PBR lighting: sun + underwater scatter fill, cascaded shadows, ambient.
//!
//! Bevy's [`StandardMaterial`] is a physically based renderer, so the scene
//! only looks correct when the light rig respects physical units (lux /
//! cd·m⁻²). The defaults (~220 cd·m⁻² ambient, 9000 lux direct) we used
//! before overwhelmed the direct light and flattened the image. This module
//! replaces them with a tuned rig:
//!
//! * A primary "sun" directional light, shining down from the surface with
//!   cascaded shadows enabled so chunks self-shadow correctly.
//! * A cool, dim fill light coming roughly from below — a cheap stand-in for
//!   the blue-green scatter you get from light bouncing around a water
//!   column — with shadows disabled to keep the base scene cheap.
//! * A low, slightly-blue [`GlobalAmbientLight`] so shadowed crevices stay
//!   readable without washing out the direct light.
//!
//! The values here are tuned together; tweak them as a set.
//!
//! See [`AtmospherePlugin`](crate::rendering::AtmospherePlugin) for how this
//! plugin composes with the camera, fog, tone-mapping, and sea surface.

use bevy::light::{CascadeShadowConfigBuilder, DirectionalLightShadowMap};
use bevy::prelude::*;

/// Cascaded shadow-map resolution per cascade, in texels.
///
/// Must be a power of two. 4096 gives crisp contact shadows on the nearest
/// cascade while still fitting comfortably in VRAM on mid-range GPUs.
const SHADOW_MAP_SIZE: usize = 4096;

/// Furthest distance, in world units, at which the sun casts shadows.
///
/// Our world is currently a ~96×96 block footprint with a 32-block water
/// column, so 200 units comfortably covers everything on screen.
const SHADOW_MAX_DISTANCE: f32 = 200.0;

/// Far plane of the first (sharpest) cascade.
///
/// Anything closer than this to the camera uses the highest-resolution slice
/// of the cascaded shadow map.
const FIRST_CASCADE_FAR_BOUND: f32 = 18.0;

/// Plugin that installs the PBR light rig for the underwater scene.
pub struct LightingPlugin;

impl Plugin for LightingPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(DirectionalLightShadowMap {
            size: SHADOW_MAP_SIZE,
        })
        .insert_resource(GlobalAmbientLight {
            // A dim, cool ambient fills shadowed crevices with the same
            // blue-green cast as the water volume without competing with the
            // direct sun.
            color: Color::srgb(0.55, 0.78, 0.95),
            brightness: 60.0,
            ..default()
        })
        .add_systems(Startup, (spawn_sun, spawn_scatter_fill));
    }
}

/// Primary directional light — the sun as it filters through the surface.
fn spawn_sun(mut commands: Commands) {
    let shadow_config = CascadeShadowConfigBuilder {
        num_cascades: 4,
        minimum_distance: 0.1,
        first_cascade_far_bound: FIRST_CASCADE_FAR_BOUND,
        maximum_distance: SHADOW_MAX_DISTANCE,
        overlap_proportion: 0.2,
    }
    .build();

    commands.spawn((
        DirectionalLight {
            // Slight warm-to-cool tint: the sun is still warm, but most
            // green/red wavelengths get absorbed on the way down.
            color: Color::srgb(0.92, 0.97, 1.0),
            // Overcast-daylight through several metres of water.
            illuminance: 12_000.0,
            shadows_enabled: true,
            ..default()
        },
        shadow_config,
        // Aim the sun slightly off-axis so chunk faces pick up a clear
        // light/shadow split rather than a flat top-down wash.
        Transform::from_xyz(40.0, 80.0, 25.0).looking_at(Vec3::ZERO, Vec3::Y),
        Name::new("Sun"),
    ));
}

/// Secondary directional light — cool blue-green scatter from below.
///
/// Not a real physical light, but a cheap approximation of the indirect
/// fill you get underwater: light bouncing off suspended particles and the
/// seabed back up into the shadowed undersides of geometry.
fn spawn_scatter_fill(mut commands: Commands) {
    commands.spawn((
        DirectionalLight {
            color: Color::srgb(0.35, 0.65, 0.8),
            illuminance: 1_500.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_xyz(-10.0, -40.0, -10.0).looking_at(Vec3::ZERO, Vec3::Y),
        Name::new("Underwater Scatter Fill"),
    ));
}
