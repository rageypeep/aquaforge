//! Animated sea surface built as an [`ExtendedMaterial`] over
//! [`StandardMaterial`].
//!
//! The base material keeps all the underwater PBR tweaks set in
//! [`super::spawn_water_surface`] — blend alpha, low roughness, physically
//! reasonable reflectance. The extension layer plugs in a custom vertex
//! shader that displaces the tessellated plane with a small sum of sinusoids
//! and emits an analytically-derived normal, so the PBR fragment stack picks
//! up live specular highlights as the waves roll.
//!
//! There's no custom fragment shader: we lean on Bevy's default
//! `StandardMaterial` fragment for lighting, Fresnel, and fog integration.

use bevy::pbr::{ExtendedMaterial, MaterialExtension, MaterialPlugin};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;

/// Path (under `assets/`) of the water vertex shader.
const WATER_SHADER_PATH: &str = "shaders/water.wgsl";

/// Public type alias for the complete water material, so callers don't have
/// to spell out the `ExtendedMaterial` wrapper.
pub type WaterMaterial = ExtendedMaterial<StandardMaterial, WaterMaterialExt>;

/// Installs the water `MaterialPlugin` so [`WaterMaterial`] assets can be
/// spawned on mesh entities.
pub struct WaterMaterialPlugin;

impl Plugin for WaterMaterialPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<WaterMaterial>::default());
    }
}

/// Wave-animation tuning knobs uploaded to the vertex shader.
///
/// Kept 16-byte aligned so the GPU layout matches the WGSL `WaterParams`
/// struct on every backend — including WebGL2 when we eventually enable it.
#[derive(ShaderType, Reflect, Clone, Copy, Debug)]
#[repr(C)]
pub struct WaterParams {
    /// Peak-to-mean wave displacement in metres. The WGSL shader clamps the
    /// summed waveform to ~±1 before this scales it, so this is roughly the
    /// absolute amplitude of the tallest crest.
    pub amplitude: f32,
    /// Temporal multiplier on every wave's phase. 1.0 ≈ a lazy roll, 2.0 ≈
    /// choppy.
    pub speed: f32,
    pub _pad0: f32,
    pub _pad1: f32,
}

impl Default for WaterParams {
    fn default() -> Self {
        Self {
            amplitude: 0.22,
            speed: 1.0,
            _pad0: 0.0,
            _pad1: 0.0,
        }
    }
}

/// Extension layer bolted onto `StandardMaterial`. Only contributes a vertex
/// shader and a small uniform block — the fragment stage remains the stock
/// PBR one.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct WaterMaterialExt {
    // Extension uniforms start at binding 100 so they don't collide with
    // StandardMaterial's slots 0..=99.
    #[uniform(100)]
    pub params: WaterParams,
}

impl Default for WaterMaterialExt {
    fn default() -> Self {
        Self {
            params: WaterParams::default(),
        }
    }
}

impl MaterialExtension for WaterMaterialExt {
    fn vertex_shader() -> ShaderRef {
        WATER_SHADER_PATH.into()
    }
}
