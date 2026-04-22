// Underwater volumetric god-rays: a screen-space radial blur applied to
// the post-tonemap HDR target.
//
// Technique is a classic light-shafts post-process (Mitchell 2007 /
// Kenny Mitchell's "Volumetric Light Scattering as a Post-Process"):
// step from every pixel towards the sun's screen-space projection,
// accumulate samples with an exponential decay, and add the result back
// on top of the scene. The bright above-water pixels near the sun end
// up smeared into radial shafts that read as sunlight piercing the
// water column.
//
// We lean on two details to make this cheap and tasteful:
//
// 1. Decay curve narrows the shafts toward the sun: each step multiplies
//    the previous sample's contribution by `decay`, so contributions
//    from pixels far from the sun fade away before they reach the
//    pixel being shaded.
// 2. `weight` and `exposure` are exposed so the effect can be tuned
//    from the main world without editing the shader.
//
// When the sun projects behind the camera (clip-space `w < 0`, packed
// into `visibility`) the CPU side zeros `visibility` and the shader
// short-circuits, producing no rays.

#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput;

struct GodRaysParams {
    // Sun position in screen UV space (`[0, 1]` per axis, origin top-left
    // to match `FullscreenVertexOutput::uv`). Valid only when
    // `visibility > 0`.
    sun_uv: vec2<f32>,
    // `1.0` when the sun is in front of the camera, `0.0` otherwise.
    visibility: f32,
    // How far along each sample ray to march in UV space (fraction of
    // the screen). `1.0` means samples walk the full distance from the
    // pixel to `sun_uv`; smaller values tighten the shafts around the
    // sun.
    density: f32,
    // Multiplicative per-step attenuation. `0.95` gives soft, long
    // shafts; `0.85` gives tight nibs right at the sun.
    decay: f32,
    // Weight applied to each sample before decay. Scales overall
    // brightness without changing the shaft length.
    weight: f32,
    // Final multiplier on the accumulated shaft colour. Separate from
    // `weight` so you can boost shafts without having to compensate
    // against the sample count.
    exposure: f32,
    // Number of samples along each ray. Stored as an `f32` so the
    // uniform block matches the WGSL layout on all backends.
    samples: f32,
}

@group(0) @binding(0) var scene_texture: texture_2d<f32>;
@group(0) @binding(1) var scene_sampler: sampler;
@group(0) @binding(2) var<uniform> params: GodRaysParams;

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let original = textureSampleLevel(scene_texture, scene_sampler, in.uv, 0.0);

    // Sun off-screen or behind us → nothing to add.
    if params.visibility <= 0.0 || params.samples < 1.0 {
        return original;
    }

    let samples_i = i32(params.samples);
    let inv_samples = 1.0 / params.samples;

    // Step vector from the current pixel toward the sun in UV space.
    // Scaled by `density / samples` so total walk length = `density`.
    let delta = (params.sun_uv - in.uv) * (params.density * inv_samples);

    var uv = in.uv;
    var illum_decay: f32 = 1.0;
    var shafts = vec3<f32>(0.0);

    for (var i: i32 = 0; i < samples_i; i = i + 1) {
        uv = uv + delta;
        // Guard against sampling outside the texture: samples that
        // step off-screen contribute nothing rather than smearing
        // the edge clamp back into the rays.
        if uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0 {
            break;
        }
        // Brightness-gated sample: we only want actually-bright pixels
        // (the sky / water column near the surface) to contribute to
        // shafts. The `max(0, x - threshold)` keeps the dim seabed
        // from smearing into the water.
        //
        // `textureSampleLevel` with an explicit LOD rather than
        // `textureSample` so WGSL doesn't need this call to sit in
        // uniform control flow — the `break` above makes the surrounding
        // flow non-uniform, and the scene target is a single-mip surface
        // anyway so implicit derivatives would be wasted.
        let sample_rgb = textureSampleLevel(scene_texture, scene_sampler, uv, 0.0).rgb;
        let brightness = max(max(sample_rgb.r, sample_rgb.g), sample_rgb.b);
        let gated = sample_rgb * max(0.0, brightness - 0.25);

        shafts = shafts + gated * (illum_decay * params.weight);
        illum_decay = illum_decay * params.decay;
    }

    // Tint the accumulated shafts so they read as warm shallow-water
    // sunlight rather than generic grey bloom.
    let tint = vec3<f32>(1.00, 0.92, 0.75);
    let rays = shafts * params.exposure * params.visibility * tint;

    return vec4<f32>(original.rgb + rays, original.a);
}
