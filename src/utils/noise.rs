//! Dependency-free deterministic noise used by the world generator.
//!
//! We avoid pulling in an external noise crate so `cargo build --offline`
//! stays cheap. This module implements a very small value-noise + fBm good
//! enough to produce believable-looking seabeds.

use super::math::{bilerp, smoothstep};

/// Deterministic per-cell hash in `[0, 1)`.
#[inline]
fn hash2(x: i32, y: i32, seed: u32) -> f32 {
    // Wang-style integer hash mixed with the seed.
    let mut h = seed
        .wrapping_add((x as u32).wrapping_mul(0x85EBCA77))
        .wrapping_add((y as u32).wrapping_mul(0xC2B2AE3D));
    h ^= h >> 13;
    h = h.wrapping_mul(0x27D4EB2F);
    h ^= h >> 16;
    (h & 0x00FF_FFFF) as f32 / 0x0100_0000 as f32
}

/// 2D value noise in `[0, 1]`.
pub fn value_2d(x: f32, y: f32, seed: u32) -> f32 {
    let xi = x.floor() as i32;
    let yi = y.floor() as i32;
    let xf = x - xi as f32;
    let yf = y - yi as f32;

    let a = hash2(xi, yi, seed);
    let b = hash2(xi + 1, yi, seed);
    let c = hash2(xi, yi + 1, seed);
    let d = hash2(xi + 1, yi + 1, seed);

    let tx = smoothstep(xf);
    let ty = smoothstep(yf);
    bilerp(a, b, c, d, tx, ty)
}

/// Fractional Brownian Motion (summed octaves of value noise) in `[-1, 1]`.
pub fn fbm_2d(x: f32, y: f32, seed: u32, octaves: u32) -> f32 {
    let mut amp = 1.0_f32;
    let mut freq = 1.0_f32;
    let mut sum = 0.0_f32;
    let mut norm = 0.0_f32;

    for o in 0..octaves {
        // Remap value noise to [-1, 1].
        let n = value_2d(x * freq, y * freq, seed.wrapping_add(o)) * 2.0 - 1.0;
        sum += n * amp;
        norm += amp;
        amp *= 0.5;
        freq *= 2.0;
    }

    if norm == 0.0 { 0.0 } else { sum / norm }
}
