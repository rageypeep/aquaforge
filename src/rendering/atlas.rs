//! Procedurally generated block texture atlas.
//!
//! Every [`BlockType`](crate::game::blocks::BlockType) owns one tile in a
//! small square atlas. Tiles are generated deterministically at startup
//! from the block's base colour plus a cheap per-pixel value-noise hash,
//! so there's nothing to load from disk and the colours per block stay
//! reproducible between runs. The atlas is then wrapped in a standard
//! Bevy [`Image`] and stored as a [`BlockAtlas`] resource that the chunk
//! material samples.

use bevy::asset::RenderAssetUsages;
use bevy::image::{Image, ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use crate::game::blocks::BlockType;

/// Edge length, in tiles, of the atlas. Large enough to hold every
/// current [`BlockType`] with room to grow without reshuffling tile
/// coordinates.
pub const ATLAS_GRID: u32 = 4;
/// Edge length, in texels, of each tile.
pub const TILE_SIZE: u32 = 32;
/// Edge length, in texels, of the full atlas.
pub const ATLAS_SIZE: u32 = ATLAS_GRID * TILE_SIZE;

/// Handle to the generated block atlas.
///
/// Held as a resource so systems that need to (re)bind the material can
/// look it up without regenerating the texture.
#[derive(Resource, Debug, Clone)]
pub struct BlockAtlas(pub Handle<Image>);

/// Register startup generation of the atlas.
pub struct BlockAtlasPlugin;

impl Plugin for BlockAtlasPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreStartup, build_atlas);
    }
}

fn build_atlas(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let handle = images.add(generate_atlas());
    commands.insert_resource(BlockAtlas(handle));
}

/// Which atlas tile a block type uses, as `(tx, ty)` in tile units.
///
/// This is the single source of truth: block meshers consult it to emit
/// UVs and [`generate_atlas`] consults it to decide where to paint each
/// block's pattern.
pub fn tile_of(block: BlockType) -> UVec2 {
    match block {
        // Air never gets meshed, but assigning (0,0) keeps this total.
        BlockType::Air => UVec2::new(0, 0),
        BlockType::Stone => UVec2::new(0, 0),
        BlockType::Sand => UVec2::new(1, 0),
        BlockType::Dirt => UVec2::new(2, 0),
        BlockType::Coral => UVec2::new(3, 0),
        BlockType::Kelp => UVec2::new(0, 1),
    }
}

/// UV rect for a single tile in atlas-normalized coordinates.
///
/// Returned as `(min_uv, max_uv)` so meshers can pick either corner.
pub fn tile_uv_rect(block: BlockType) -> (Vec2, Vec2) {
    let tile = tile_of(block);
    let size = 1.0 / ATLAS_GRID as f32;
    let min = Vec2::new(tile.x as f32 * size, tile.y as f32 * size);
    let max = min + Vec2::splat(size);
    (min, max)
}

/// Build the RGBA8 atlas image.
///
/// Split out from [`build_atlas`] so tests can verify deterministic
/// output without touching Bevy's asset server.
pub fn generate_atlas() -> Image {
    let pixels = generate_atlas_pixels();
    let mut image = Image::new(
        Extent3d {
            width: ATLAS_SIZE,
            height: ATLAS_SIZE,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    // Nearest-neighbour filtering + repeating wrap gives crisp pixel-art
    // blocks and lets future greedy-meshed quads tile tiles cleanly.
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        address_mode_w: ImageAddressMode::Repeat,
        mag_filter: bevy::image::ImageFilterMode::Nearest,
        min_filter: bevy::image::ImageFilterMode::Nearest,
        mipmap_filter: bevy::image::ImageFilterMode::Nearest,
        ..default()
    });
    image
}

/// Pack every tile into a single RGBA8 byte buffer.
fn generate_atlas_pixels() -> Vec<u8> {
    let mut pixels = vec![0u8; (ATLAS_SIZE * ATLAS_SIZE * 4) as usize];
    for block in [
        BlockType::Stone,
        BlockType::Sand,
        BlockType::Dirt,
        BlockType::Coral,
        BlockType::Kelp,
    ] {
        paint_tile(&mut pixels, block);
    }
    pixels
}

fn paint_tile(pixels: &mut [u8], block: BlockType) {
    let tile = tile_of(block);
    let [br, bg, bb] = base_rgb_u8(block);
    let noise_scale = noise_scale(block);
    let detail = detail_scale(block);

    for py in 0..TILE_SIZE {
        for px in 0..TILE_SIZE {
            let n = value_noise(block, px, py);
            let (r, g, b) = tint_pixel(block, br, bg, bb, n, noise_scale, detail, px, py);

            let x = tile.x * TILE_SIZE + px;
            let y = tile.y * TILE_SIZE + py;
            let i = ((y * ATLAS_SIZE + x) * 4) as usize;
            pixels[i] = r;
            pixels[i + 1] = g;
            pixels[i + 2] = b;
            pixels[i + 3] = 255;
        }
    }
}

fn base_rgb_u8(block: BlockType) -> [u8; 3] {
    // Convert the block's linear-space colour to sRGB u8 for the texture.
    let c = block.color();
    let srgba: Srgba = Color::LinearRgba(c).into();
    [
        (srgba.red.clamp(0.0, 1.0) * 255.0) as u8,
        (srgba.green.clamp(0.0, 1.0) * 255.0) as u8,
        (srgba.blue.clamp(0.0, 1.0) * 255.0) as u8,
    ]
}

/// Amplitude of the base per-pixel noise, in 0..1.
fn noise_scale(block: BlockType) -> f32 {
    match block {
        BlockType::Stone => 0.18,
        BlockType::Sand => 0.10,
        BlockType::Dirt => 0.20,
        BlockType::Coral => 0.28,
        BlockType::Kelp => 0.22,
        BlockType::Air => 0.0,
    }
}

/// Amplitude of a coarser secondary feature (stripes, dapples, etc).
fn detail_scale(block: BlockType) -> f32 {
    match block {
        BlockType::Stone => 0.08,
        BlockType::Sand => 0.05,
        BlockType::Dirt => 0.10,
        BlockType::Coral => 0.20,
        BlockType::Kelp => 0.35,
        BlockType::Air => 0.0,
    }
}

#[allow(clippy::too_many_arguments)]
fn tint_pixel(
    block: BlockType,
    br: u8,
    bg: u8,
    bb: u8,
    noise: f32,
    noise_amp: f32,
    detail_amp: f32,
    px: u32,
    py: u32,
) -> (u8, u8, u8) {
    // Primary darkening/lightening multiplier from value noise.
    let primary = 1.0 + (noise - 0.5) * 2.0 * noise_amp;

    // Block-specific secondary feature.
    let detail = match block {
        // Subtle vertical-ish streaks for kelp to evoke fronds.
        BlockType::Kelp => {
            let stripe = ((px as f32 * 0.7).sin() * 0.5 + 0.5) * 0.6
                + ((py as f32 * 0.17).sin() * 0.5 + 0.5) * 0.4;
            1.0 + (stripe - 0.5) * 2.0 * detail_amp
        }
        // Brighter dapples for coral.
        BlockType::Coral => {
            let hash = (value_noise(block, px / 3, py / 3) - 0.5) * 2.0 * detail_amp;
            1.0 + hash
        }
        _ => {
            let coarse = value_noise(block, px / 4, py / 4);
            1.0 + (coarse - 0.5) * 2.0 * detail_amp
        }
    };

    let mul = (primary * detail).clamp(0.35, 1.6);
    let r = (br as f32 * mul).clamp(0.0, 255.0) as u8;
    let g = (bg as f32 * mul).clamp(0.0, 255.0) as u8;
    let b = (bb as f32 * mul).clamp(0.0, 255.0) as u8;
    (r, g, b)
}

/// Deterministic 2D value noise in `[0, 1]`, seeded off the block type so
/// every tile has its own grain.
fn value_noise(block: BlockType, x: u32, y: u32) -> f32 {
    let seed = block_seed(block);
    let mut h = seed
        .wrapping_add(x.wrapping_mul(0x9E37_79B9))
        .wrapping_add(y.wrapping_mul(0x85EB_CA6B));
    h ^= h >> 13;
    h = h.wrapping_mul(0xC2B2_AE35);
    h ^= h >> 16;
    (h & 0x00FF_FFFF) as f32 / 0x00FF_FFFF as f32
}

fn block_seed(block: BlockType) -> u32 {
    match block {
        BlockType::Air => 0,
        BlockType::Stone => 0x13AD_BEEF,
        BlockType::Sand => 0x5A9D_D11E,
        BlockType::Dirt => 0xD147_FACE,
        BlockType::Coral => 0xC0EA_7555,
        BlockType::Kelp => 0x8E19_F0E0,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn atlas_has_expected_pixel_count() {
        let pixels = generate_atlas_pixels();
        assert_eq!(pixels.len(), (ATLAS_SIZE * ATLAS_SIZE * 4) as usize);
    }

    #[test]
    fn every_block_type_has_a_unique_tile() {
        let tiles: HashSet<UVec2> = [
            BlockType::Stone,
            BlockType::Sand,
            BlockType::Dirt,
            BlockType::Coral,
            BlockType::Kelp,
        ]
        .into_iter()
        .map(tile_of)
        .collect();
        assert_eq!(tiles.len(), 5, "each block must own its own tile");
    }

    #[test]
    fn tile_uv_rect_matches_tile_of() {
        let (min, max) = tile_uv_rect(BlockType::Sand);
        let tile = tile_of(BlockType::Sand);
        let size = 1.0 / ATLAS_GRID as f32;
        assert!((min.x - tile.x as f32 * size).abs() < 1e-6);
        assert!((min.y - tile.y as f32 * size).abs() < 1e-6);
        assert!((max.x - min.x - size).abs() < 1e-6);
        assert!((max.y - min.y - size).abs() < 1e-6);
    }

    #[test]
    fn generation_is_deterministic() {
        let a = generate_atlas_pixels();
        let b = generate_atlas_pixels();
        assert_eq!(a, b);
    }

    #[test]
    fn distinct_block_tiles_have_different_contents() {
        let pixels = generate_atlas_pixels();
        // Sample the centre of two tiles; they must disagree somewhere.
        let centre = |b: BlockType| {
            let t = tile_of(b);
            let x = t.x * TILE_SIZE + TILE_SIZE / 2;
            let y = t.y * TILE_SIZE + TILE_SIZE / 2;
            let i = ((y * ATLAS_SIZE + x) * 4) as usize;
            [pixels[i], pixels[i + 1], pixels[i + 2]]
        };
        assert_ne!(centre(BlockType::Stone), centre(BlockType::Coral));
        assert_ne!(centre(BlockType::Sand), centre(BlockType::Kelp));
    }
}
