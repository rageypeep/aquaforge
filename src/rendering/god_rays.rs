//! Volumetric god-rays: a fullscreen post-process that turns the bright
//! area around the sun into radial light shafts.
//!
//! The effect is a classic screen-space "light scattering as a
//! post-process" (Mitchell 2007): for every pixel we march a short
//! distance towards the sun's screen-space projection, sample the scene
//! colour at each step with an exponential decay, and add the result
//! on top of the frame. The bright sky / water column near the sun
//! smears into radial shafts; dark seabed pixels contribute nothing
//! because of the brightness gate baked into the shader.
//!
//! Breakdown:
//!
//! * The main-world side owns a [`GodRaysSettings`] component attached
//!   to the player camera plus a [`SunLight`] marker on whichever
//!   `DirectionalLight` is the sun. [`update_god_rays_sun`] projects
//!   the sun's world position into screen UV space each frame and
//!   writes it into the component.
//! * The render-world side installs a [`ViewNode`] between tone-mapping
//!   and the end of the main post-processing chain, reads the settings
//!   via [`ComponentUniforms`], and runs a fullscreen pass sampling
//!   the current view target with the shader in
//!   `assets/shaders/god_rays.wgsl`.

use bevy::core_pipeline::FullscreenShader;
use bevy::core_pipeline::core_3d::graph::{Core3d, Node3d};
use bevy::ecs::query::QueryItem;
use bevy::image::BevyDefault;
use bevy::prelude::*;
use bevy::render::RenderApp;
use bevy::render::RenderStartup;
use bevy::render::extract_component::{
    ComponentUniforms, DynamicUniformIndex, ExtractComponent, ExtractComponentPlugin,
    UniformComponentPlugin,
};
use bevy::render::render_graph::{
    NodeRunError, RenderGraphContext, RenderGraphExt, RenderLabel, ViewNode, ViewNodeRunner,
};
use bevy::render::render_resource::binding_types::{sampler, texture_2d, uniform_buffer};
use bevy::render::render_resource::{
    BindGroupEntries, BindGroupLayoutDescriptor, BindGroupLayoutEntries, CachedRenderPipelineId,
    ColorTargetState, ColorWrites, FragmentState, Operations, PipelineCache,
    RenderPassColorAttachment, RenderPassDescriptor, RenderPipelineDescriptor, Sampler,
    SamplerBindingType, SamplerDescriptor, ShaderStages, ShaderType, TextureFormat,
    TextureSampleType,
};
use bevy::render::renderer::{RenderContext, RenderDevice};
use bevy::render::view::ViewTarget;

/// Path (under `assets/`) of the god-rays fragment shader.
const GOD_RAYS_SHADER_PATH: &str = "shaders/god_rays.wgsl";

/// Plugin installing the god-rays post-process pipeline and its
/// main-world → render-world bridge.
pub struct GodRaysPlugin;

impl Plugin for GodRaysPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            ExtractComponentPlugin::<GodRaysSettings>::default(),
            UniformComponentPlugin::<GodRaysSettings>::default(),
        ))
        .add_systems(Update, update_god_rays_sun);

        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app
            .add_systems(RenderStartup, init_god_rays_pipeline)
            .add_render_graph_node::<ViewNodeRunner<GodRaysNode>>(Core3d, GodRaysLabel)
            .add_render_graph_edges(
                Core3d,
                (
                    Node3d::Tonemapping,
                    GodRaysLabel,
                    Node3d::EndMainPassPostProcessing,
                ),
            );
    }
}

/// Marker component placed on whichever `DirectionalLight` represents
/// the sun. The god-rays system uses this to find the light whose
/// world-space direction should drive the shaft origin.
#[derive(Component, Debug, Default)]
pub struct SunLight;

/// Per-camera tuning for the god-rays post-process.
///
/// Attach this component to a [`Camera3d`] entity to opt that camera
/// into the effect. The [`ExtractComponentPlugin`] mirrors it into the
/// render world each frame, and [`UniformComponentPlugin`] uploads it
/// as a uniform block.
///
/// Fields prefixed `_pad` are WebGL2 alignment padding and have no
/// runtime semantics.
#[derive(Component, Clone, Copy, Debug, ShaderType, ExtractComponent)]
pub struct GodRaysSettings {
    /// Sun projected into screen-UV space (`0..1`). Written every frame
    /// by [`update_god_rays_sun`] from the sun's `GlobalTransform`.
    pub sun_uv: Vec2,
    /// `1.0` when the sun is in front of the camera, `0.0` otherwise.
    /// The shader short-circuits when this is zero so off-screen suns
    /// don't leak rays from clamped UV samples.
    pub visibility: f32,
    /// Fraction of the screen each sample ray covers. `0.5` gives short
    /// shafts; `1.0` lets rays reach from the far corner all the way
    /// to the sun.
    pub density: f32,
    /// Multiplicative per-step attenuation. `0.96` is soft and long;
    /// `0.88` is a tight nib at the sun.
    pub decay: f32,
    /// Weight applied to each sample before decay. Scales the overall
    /// brightness without changing shaft length.
    pub weight: f32,
    /// Final multiplier on the accumulated shaft colour.
    pub exposure: f32,
    /// Number of samples per ray. Stored as `f32` so the WGSL uniform
    /// layout matches on every backend.
    pub samples: f32,
}

impl Default for GodRaysSettings {
    fn default() -> Self {
        Self {
            sun_uv: Vec2::new(0.5, 0.0),
            visibility: 0.0,
            density: 0.85,
            decay: 0.955,
            weight: 0.35,
            exposure: 0.60,
            samples: 48.0,
        }
    }
}

/// Pre-clamps god-rays tuning to safe ranges. Useful for authored
/// presets that might drift out of bounds as fields are added.
pub fn clamp_settings(settings: GodRaysSettings) -> GodRaysSettings {
    GodRaysSettings {
        sun_uv: settings.sun_uv,
        visibility: settings.visibility.clamp(0.0, 1.0),
        density: settings.density.clamp(0.0, 2.0),
        decay: settings.decay.clamp(0.0, 0.9999),
        weight: settings.weight.max(0.0),
        exposure: settings.exposure.max(0.0),
        // Shader uses an `i32` loop count internally; cap at 128 to
        // keep the pass cheap on integrated GPUs.
        samples: settings.samples.clamp(1.0, 128.0),
    }
}

/// Per-frame system: project the sun's world position into the
/// camera's screen-UV space and stash it in [`GodRaysSettings::sun_uv`].
pub fn update_god_rays_sun(
    cameras: Query<(&GlobalTransform, &Camera, &mut GodRaysSettings)>,
    sun: Query<&GlobalTransform, With<SunLight>>,
) {
    let Ok(sun_tf) = sun.single() else {
        return;
    };
    // DirectionalLight shines along local `-Z`; `back()` points from the
    // lit scene up toward the sun's notional position. Placing a proxy
    // sun at `camera + sun_dir * far` matches where the sun appears on
    // screen, because a directional light is treated as infinitely far
    // away.
    let sun_dir_world = sun_tf.back();
    for (cam_tf, camera, mut settings) in cameras {
        let sun_world = cam_tf.translation() + Vec3::from(sun_dir_world) * 1_000.0;
        match camera.world_to_ndc(cam_tf, sun_world) {
            Some(ndc) if ndc.z > 0.0 && ndc.z < 1.0 => {
                // NDC.x ∈ [-1, 1] → UV.x ∈ [0, 1]; NDC.y is bottom-up,
                // so flip to match top-down UV.
                let uv = Vec2::new(ndc.x * 0.5 + 0.5, ndc.y * -0.5 + 0.5);
                settings.sun_uv = uv;
                // Fade out as the sun approaches the edge of the
                // viewport so shafts don't cut off abruptly.
                let edge_fade = edge_fade(uv);
                settings.visibility = edge_fade;
            }
            _ => {
                settings.visibility = 0.0;
            }
        }
    }
}

/// Returns a `[0, 1]` factor that falls off toward zero as a UV
/// coordinate leaves the viewport. Pure function so it can be unit-tested.
pub fn edge_fade(uv: Vec2) -> f32 {
    // In-frame: 1.0. Up to 0.5 UV units past the edge: fades to 0.
    let over_x = (uv.x - 1.0).max(-uv.x).max(0.0);
    let over_y = (uv.y - 1.0).max(-uv.y).max(0.0);
    let over = over_x.max(over_y);
    (1.0 - over * 2.0).clamp(0.0, 1.0)
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
struct GodRaysLabel;

#[derive(Default)]
struct GodRaysNode;

impl ViewNode for GodRaysNode {
    type ViewQuery = (
        &'static ViewTarget,
        &'static GodRaysSettings,
        &'static DynamicUniformIndex<GodRaysSettings>,
    );

    fn run(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        (view_target, _settings, settings_index): QueryItem<Self::ViewQuery>,
        world: &World,
    ) -> Result<(), NodeRunError> {
        let pipeline_res = world.resource::<GodRaysPipeline>();
        let pipeline_cache = world.resource::<PipelineCache>();
        let Some(pipeline) = pipeline_cache.get_render_pipeline(pipeline_res.pipeline_id) else {
            return Ok(());
        };

        let settings_uniforms = world.resource::<ComponentUniforms<GodRaysSettings>>();
        let Some(settings_binding) = settings_uniforms.uniforms().binding() else {
            return Ok(());
        };

        let post_process = view_target.post_process_write();

        let bind_group = render_context.render_device().create_bind_group(
            "god_rays_bind_group",
            &pipeline_cache.get_bind_group_layout(&pipeline_res.layout),
            &BindGroupEntries::sequential((
                post_process.source,
                &pipeline_res.sampler,
                settings_binding.clone(),
            )),
        );

        let mut render_pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("god_rays_pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: post_process.destination,
                depth_slice: None,
                resolve_target: None,
                ops: Operations::default(),
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        render_pass.set_render_pipeline(pipeline);
        render_pass.set_bind_group(0, &bind_group, &[settings_index.index()]);
        render_pass.draw(0..3, 0..1);

        Ok(())
    }
}

/// GPU-side pipeline state for [`GodRaysNode`]. Created once on render
/// startup and never mutated afterwards.
#[derive(Resource)]
struct GodRaysPipeline {
    layout: BindGroupLayoutDescriptor,
    sampler: Sampler,
    pipeline_id: CachedRenderPipelineId,
}

fn init_god_rays_pipeline(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    asset_server: Res<AssetServer>,
    fullscreen_shader: Res<FullscreenShader>,
    pipeline_cache: Res<PipelineCache>,
) {
    let layout = BindGroupLayoutDescriptor::new(
        "god_rays_bind_group_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (
                texture_2d(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
                uniform_buffer::<GodRaysSettings>(true),
            ),
        ),
    );
    let sampler = render_device.create_sampler(&SamplerDescriptor::default());
    let shader = asset_server.load(GOD_RAYS_SHADER_PATH);
    let vertex_state = fullscreen_shader.to_vertex_state();
    let pipeline_id = pipeline_cache.queue_render_pipeline(RenderPipelineDescriptor {
        label: Some("god_rays_pipeline".into()),
        layout: vec![layout.clone()],
        vertex: vertex_state,
        fragment: Some(FragmentState {
            shader,
            targets: vec![Some(ColorTargetState {
                format: TextureFormat::bevy_default(),
                blend: None,
                write_mask: ColorWrites::ALL,
            })],
            ..default()
        }),
        ..default()
    });
    commands.insert_resource(GodRaysPipeline {
        layout,
        sampler,
        pipeline_id,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edge_fade_in_frame_is_one() {
        assert_eq!(edge_fade(Vec2::new(0.5, 0.5)), 1.0);
        assert_eq!(edge_fade(Vec2::new(0.0, 1.0)), 1.0);
    }

    #[test]
    fn edge_fade_off_frame_fades() {
        let fade = edge_fade(Vec2::new(1.25, 0.5));
        assert!(fade > 0.0 && fade < 1.0);
    }

    #[test]
    fn edge_fade_far_off_frame_is_zero() {
        assert_eq!(edge_fade(Vec2::new(1.6, 0.5)), 0.0);
        assert_eq!(edge_fade(Vec2::new(0.5, -0.6)), 0.0);
    }

    #[test]
    fn defaults_clamp_in_range() {
        let s = clamp_settings(GodRaysSettings::default());
        assert!(s.decay < 1.0);
        assert!(s.density > 0.0 && s.density <= 2.0);
        assert!(s.samples >= 1.0 && s.samples <= 128.0);
    }

    #[test]
    fn clamp_catches_bad_inputs() {
        let bad = GodRaysSettings {
            sun_uv: Vec2::ZERO,
            visibility: 10.0,
            density: -1.0,
            decay: 1.5,
            weight: -0.2,
            exposure: -1.0,
            samples: 10_000.0,
        };
        let c = clamp_settings(bad);
        assert_eq!(c.visibility, 1.0);
        assert_eq!(c.density, 0.0);
        assert!(c.decay <= 0.9999);
        assert_eq!(c.weight, 0.0);
        assert_eq!(c.exposure, 0.0);
        assert_eq!(c.samples, 128.0);
    }
}
