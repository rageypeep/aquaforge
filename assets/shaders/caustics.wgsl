// Caustics-aware PBR fragment for underwater voxel surfaces.
//
// Pipeline:
//   1. Run the stock `StandardMaterial` fragment logic (so the chunk still
//      reads as the same PBR surface it does today — texture, AO, fog,
//      shadows, the works).
//   2. On top of that, add a procedural caustics term that's masked to
//      upward-facing faces (so it only brightens the seabed, not ceilings
//      or vertical walls) and fades with depth under the water plane.
//
// Caustics pattern: sum three sinusoids at uncorrelated directions and
// phases, take `abs(sum)`, then raise to a power so the bright crests
// become narrow streaks against a dark background — a cheap approximation
// of the focussed light the wavy surface projects onto the seabed.

#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
    forward_io::{VertexOutput, FragmentOutput},
    mesh_view_bindings::globals,
}

struct CausticsParams {
    // World-space Y coordinate of the water surface. The caustics fade as
    // a point sits further below this plane.
    water_level: f32,
    // Spatial frequency of the pattern. Higher = tighter streaks.
    scale: f32,
    // Time multiplier. 1.0 ≈ a lazy shimmer, 2.0 ≈ choppy.
    speed: f32,
    // Peak brightness added to the emissive term. Keep modest (~1.5)
    // because this feeds into PBR-lit emission and bloom can make it
    // blow out very quickly otherwise.
    intensity: f32,
    // Depth over which the pattern fades to zero beneath the water plane,
    // in metres. 0.0 disables the falloff (caustics stay bright everywhere).
    fade_depth: f32,
    // Padding to keep the uniform block 16-byte aligned on every backend.
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> caustics_params: CausticsParams;

// Sum-of-sinusoids caustics pattern. Returns a value in `[0, 1]` that
// peaks sharply along animated streaks in world-XZ.
fn caustics_intensity(world_xz: vec2<f32>, t: f32, scale: f32) -> f32 {
    let p = world_xz * scale;
    let a = sin(p.x * 1.3 + p.y * 0.7 + t * 1.10);
    let b = sin(p.x * 0.7 - p.y * 1.2 + t * 0.80);
    let c = sin((p.x - p.y) * 0.9 + t * 1.40);
    // Mean of three terms in `[-1, 1]`; abs → `[0, 1]`.
    let n = abs(a + b + c) * (1.0 / 3.0);
    // Pow narrows the bright crests. 3.5 gives pleasantly tight streaks
    // without making the pattern sparse enough to look like polka dots.
    return pow(n, 3.5);
}

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);

    // Caustics only really make sense on upward-facing faces below the
    // water plane. Gate by world-space normal Y and smooth-step the
    // result so near-vertical faces still pick up a faint hint.
    let up_mask = smoothstep(0.2, 0.85, pbr_input.N.y);

    // Fade with depth beneath the water plane so deep crevices don't
    // over-brighten. Surfaces above the water plane get zero caustics —
    // they're emerged, not submerged, so there's no water lens focusing
    // sunlight onto them. `fade_depth = 0` keeps full brightness at every
    // depth below the plane but still clips above-water points to zero.
    var depth_fade: f32 = 0.0;
    let depth_below = caustics_params.water_level - in.world_position.y;
    if depth_below > 0.0 {
        if caustics_params.fade_depth > 0.0 {
            depth_fade = 1.0 - clamp(depth_below / caustics_params.fade_depth, 0.0, 1.0);
        } else {
            depth_fade = 1.0;
        }
    }

    let t = globals.time * caustics_params.speed;
    let streaks = caustics_intensity(in.world_position.xz, t, caustics_params.scale);

    // Pale cyan — the sun's wavelength through a few metres of water.
    let caustic_color = vec3<f32>(0.55, 0.90, 1.00);
    let contribution = caustic_color * streaks
        * up_mask
        * depth_fade
        * caustics_params.intensity;

    // Feed caustics into `emissive` so PBR still respects AO and shadows
    // (emission is added after lighting, so fully-shadowed pixels still
    // glow — which is physically what caustics _do_: they're focussed
    // sunlight that reaches the ground independent of direct-light
    // occlusion from nearby geometry).
    pbr_input.material.emissive = vec4<f32>(
        pbr_input.material.emissive.rgb + contribution,
        pbr_input.material.emissive.a,
    );

    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
