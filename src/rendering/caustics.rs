//! Caustics: animated sunlight streaks cast onto the seabed.
//!
//! The chunk mesher emits a single mesh per chunk rendered with
//! [`ChunkMaterial`] — an [`ExtendedMaterial`] wrapping the stock
//! [`StandardMaterial`] so the atlas + AO pipeline from PR #13 keeps
//! working. The extension layer only plugs in a fragment shader that
//! adds a procedural caustics term to `emissive` before PBR lighting,
//! so fully-shadowed crevices still pick up the focussed light (which is
//! what caustics physically are — they're projected sunlight that
//! bypasses direct-light occlusion).
//!
//! The shader itself lives in `assets/shaders/caustics.wgsl`.

use bevy::pbr::{ExtendedMaterial, MaterialExtension, MaterialPlugin};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;

/// Path (under `assets/`) of the caustics fragment shader.
const CAUSTICS_SHADER_PATH: &str = "shaders/caustics.wgsl";

/// Complete chunk material: stock PBR + the caustics fragment extension.
pub type ChunkMaterial = ExtendedMaterial<StandardMaterial, CausticsMaterialExt>;

/// Installs the chunk `MaterialPlugin` so [`ChunkMaterial`] assets can be
/// attached to chunk meshes.
pub struct CausticsPlugin;

impl Plugin for CausticsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<ChunkMaterial>::default());
    }
}

/// Caustics tuning uploaded to the fragment shader as a uniform block.
///
/// Kept 16-byte aligned (four `f32`s plus three `f32`s of padding) so the
/// GPU layout matches the WGSL `CausticsParams` struct on every backend,
/// including WebGL2 when we eventually enable it.
#[derive(ShaderType, Reflect, Clone, Copy, Debug)]
#[repr(C)]
pub struct CausticsParams {
    /// World-space Y coordinate of the water surface. Points deeper than
    /// this are where caustics are brightest; the shader fades them to
    /// zero at `fade_depth` metres below.
    pub water_level: f32,
    /// Spatial frequency of the streak pattern. Higher values give
    /// tighter, higher-frequency streaks.
    pub scale: f32,
    /// Temporal multiplier on the shimmer. 1.0 ≈ a lazy roll, 2.0 ≈ choppy.
    pub speed: f32,
    /// Peak brightness added to `emissive`. Keep modest — bloom makes
    /// small values feel surprisingly bright.
    pub intensity: f32,
    /// Depth over which the pattern fades out below the water plane.
    /// Set to `0.0` to disable the falloff and keep caustics at full
    /// strength everywhere.
    pub fade_depth: f32,
    pub _pad0: f32,
    pub _pad1: f32,
    pub _pad2: f32,
}

impl Default for CausticsParams {
    fn default() -> Self {
        Self {
            // Overwritten at spawn with the real world water level.
            water_level: 0.0,
            scale: 0.35,
            speed: 0.6,
            intensity: 1.2,
            fade_depth: 30.0,
            _pad0: 0.0,
            _pad1: 0.0,
            _pad2: 0.0,
        }
    }
}

/// Extension layer bolted onto `StandardMaterial`. Contributes only a
/// fragment shader and a small uniform block; the vertex stage is the
/// stock Bevy mesh vertex stage.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct CausticsMaterialExt {
    // Extension uniforms start at binding 100 so they don't collide with
    // `StandardMaterial`'s own 0..=99 slots.
    #[uniform(100)]
    pub params: CausticsParams,
}

impl Default for CausticsMaterialExt {
    fn default() -> Self {
        Self {
            params: CausticsParams::default(),
        }
    }
}

impl MaterialExtension for CausticsMaterialExt {
    fn fragment_shader() -> ShaderRef {
        CAUSTICS_SHADER_PATH.into()
    }
    fn deferred_fragment_shader() -> ShaderRef {
        CAUSTICS_SHADER_PATH.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn params_default_is_finite() {
        let p = CausticsParams::default();
        assert!(p.scale.is_finite() && p.scale > 0.0);
        assert!(p.speed.is_finite() && p.speed > 0.0);
        assert!(p.intensity.is_finite() && p.intensity >= 0.0);
        assert!(p.fade_depth.is_finite() && p.fade_depth >= 0.0);
    }

    #[test]
    fn params_size_is_sixteen_byte_aligned() {
        // CausticsParams must be a multiple of 16 bytes so the WGSL
        // uniform layout agrees on every backend (including WebGL2).
        assert_eq!(std::mem::size_of::<CausticsParams>() % 16, 0);
    }
}
