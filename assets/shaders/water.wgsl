// Animated sea surface: a custom vertex stage that extends the PBR standard
// material. We displace the plane vertically with a small sum of sinusoids
// and emit an analytically-derived world-space normal so the PBR fragment
// pass picks up live Fresnel / specular highlights as the waves move.
//
// The fragment stage is left to Bevy's default StandardMaterial fragment.

#import bevy_pbr::{
    mesh_functions,
    forward_io::{Vertex, VertexOutput},
    view_transformations::position_world_to_clip,
    mesh_view_bindings::globals,
}

struct WaterParams {
    // Scales the overall wave amplitude in metres.
    amplitude: f32,
    // Multiplies the temporal term of every wave.
    speed: f32,
    // Padding to keep the uniform block 16-byte aligned.
    _pad0: f32,
    _pad1: f32,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> water_params: WaterParams;

// Four wavelets, each described by a direction (xy) and a temporal phase (z).
// Wavelengths are chosen so that they aren't harmonics of each other — the
// surface never visibly "clicks" back to a repeating state in a short window.
const WAVE_COUNT: u32 = 4u;
const WAVES: array<vec3<f32>, 4> = array<vec3<f32>, 4>(
    vec3<f32>( 0.35,  0.10, 1.2),
    vec3<f32>(-0.12,  0.28, 0.9),
    vec3<f32>( 0.22, -0.19, 1.5),
    vec3<f32>(-0.31, -0.07, 0.7),
);
// Per-wave amplitude weights. Sum ~= 1.0 so `amplitude` parameter stays in
// metres.
const WAVE_WEIGHTS: array<f32, 4> = array<f32, 4>(0.40, 0.28, 0.20, 0.12);

fn wave_sample(p: vec2<f32>, t: f32) -> vec3<f32> {
    // Returns (height, dH/dx, dH/dz) for the analytic sum.
    var h: f32 = 0.0;
    var dhx: f32 = 0.0;
    var dhz: f32 = 0.0;
    for (var i: u32 = 0u; i < WAVE_COUNT; i = i + 1u) {
        let w = WAVES[i];
        let weight = WAVE_WEIGHTS[i];
        let phase = dot(p, w.xy) + t * w.z;
        let s = sin(phase);
        let c = cos(phase);
        h = h + weight * s;
        dhx = dhx + weight * c * w.x;
        dhz = dhz + weight * c * w.y;
    }
    return vec3<f32>(h, dhx, dhz);
}

@vertex
fn vertex(vertex_in: Vertex) -> VertexOutput {
    var out: VertexOutput;

    let world_from_local = mesh_functions::get_world_from_local(vertex_in.instance_index);
    let t = globals.time * water_params.speed;
    let amp = water_params.amplitude;

    // Use local XZ as the wave domain. The water plane is only translated on
    // Y, so local and world XZ agree on horizontal placement.
    let sample = wave_sample(vertex_in.position.xz, t);
    let h = sample.x * amp;
    let dhx = sample.y * amp;
    let dhz = sample.z * amp;

    var local_pos = vertex_in.position;
    local_pos.y = local_pos.y + h;

    out.world_position = mesh_functions::mesh_position_local_to_world(
        world_from_local,
        vec4<f32>(local_pos, 1.0),
    );
    out.position = position_world_to_clip(out.world_position.xyz);

#ifdef VERTEX_NORMALS
    // Normal of a heightfield H(x,z) is `normalize(-dH/dx, 1, -dH/dz)`.
    let n_local = normalize(vec3<f32>(-dhx, 1.0, -dhz));
    out.world_normal = mesh_functions::mesh_normal_local_to_world(
        n_local,
        vertex_in.instance_index,
    );
#endif

#ifdef VERTEX_UVS_A
    out.uv = vertex_in.uv;
#endif
#ifdef VERTEX_UVS_B
    out.uv_b = vertex_in.uv_b;
#endif

#ifdef VERTEX_TANGENTS
    out.world_tangent = mesh_functions::mesh_tangent_local_to_world(
        world_from_local,
        vertex_in.tangent,
        vertex_in.instance_index,
    );
#endif

#ifdef VERTEX_COLORS
    out.color = vertex_in.color;
#endif

#ifdef VERTEX_OUTPUT_INSTANCE_INDEX
    out.instance_index = vertex_in.instance_index;
#endif

    return out;
}
