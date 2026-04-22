//! Chunks: fixed-size 3D arrays of blocks plus face-culling mesh generation.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, Mesh, PrimitiveTopology};
use bevy::prelude::*;

use super::blocks::BlockType;
use crate::utils::noise;

/// Edge length of a chunk, in blocks.
pub const CHUNK_SIZE: usize = 16;

/// A fixed-size 3D grid of blocks.
///
/// Chunks are attached as components to chunk entities so gameplay systems
/// can mutate them and trigger mesh rebuilds.
#[derive(Component, Clone)]
pub struct Chunk {
    /// Block data indexed as `blocks[x][y][z]`.
    pub blocks: Vec<BlockType>,
}

impl Chunk {
    /// An empty (all-air) chunk.
    pub fn empty() -> Self {
        Self {
            blocks: vec![BlockType::Air; CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE],
        }
    }

    /// Generate a chunk filled with a piece of the underwater world.
    ///
    /// `chunk_pos` is the chunk's coordinate in the chunk grid (each step
    /// of 1 == `CHUNK_SIZE` blocks in world space).
    pub fn generate(chunk_pos: IVec3, seed: u32) -> Self {
        let mut chunk = Self::empty();

        for x in 0..CHUNK_SIZE {
            for z in 0..CHUNK_SIZE {
                let wx = chunk_pos.x * CHUNK_SIZE as i32 + x as i32;
                let wz = chunk_pos.z * CHUNK_SIZE as i32 + z as i32;

                // Base seabed height (in world blocks).
                let h = seabed_height(wx, wz, seed);

                for y in 0..CHUNK_SIZE {
                    let wy = chunk_pos.y * CHUNK_SIZE as i32 + y as i32;

                    let block = pick_block(wx, wy, wz, h, seed);
                    chunk.set(x, y, z, block);
                }
            }
        }

        chunk
    }

    #[inline]
    fn idx(x: usize, y: usize, z: usize) -> usize {
        x + CHUNK_SIZE * (y + CHUNK_SIZE * z)
    }

    #[inline]
    pub fn get(&self, x: usize, y: usize, z: usize) -> BlockType {
        self.blocks[Self::idx(x, y, z)]
    }

    #[inline]
    pub fn set(&mut self, x: usize, y: usize, z: usize, block: BlockType) {
        self.blocks[Self::idx(x, y, z)] = block;
    }

    /// Whether the cell at `(x, y, z)` is inside this chunk and opaque.
    /// Out-of-chunk cells are treated as non-opaque so the outer shell of
    /// chunks still renders and isn't AO-darkened by missing neighbour data.
    #[inline]
    fn is_opaque_at(&self, x: i32, y: i32, z: i32) -> bool {
        let size = CHUNK_SIZE as i32;
        if (0..size).contains(&x) && (0..size).contains(&y) && (0..size).contains(&z) {
            self.get(x as usize, y as usize, z as usize).is_opaque()
        } else {
            false
        }
    }

    /// Build a mesh from this chunk's non-air blocks, emitting one quad per
    /// face that touches an air (or kelp) neighbour.
    ///
    /// Each vertex is shaded with classic three-neighbour ambient occlusion
    /// baked into the vertex colour, and each quad's triangulation is flipped
    /// so the interpolated AO is continuous across the diagonal.
    pub fn build_mesh(&self) -> Mesh {
        let mut positions: Vec<[f32; 3]> = Vec::new();
        let mut normals: Vec<[f32; 3]> = Vec::new();
        let mut colors: Vec<[f32; 4]> = Vec::new();
        let mut uvs: Vec<[f32; 2]> = Vec::new();
        let mut indices: Vec<u32> = Vec::new();

        // Normal and the four corners (as unit-cube offsets) per face,
        // wound counter-clockwise from outside.
        let faces: [([f32; 3], [[i32; 3]; 4]); 6] = [
            // +X
            ([1.0, 0.0, 0.0], [[1, 0, 1], [1, 0, 0], [1, 1, 0], [1, 1, 1]]),
            // -X
            ([-1.0, 0.0, 0.0], [[0, 0, 0], [0, 0, 1], [0, 1, 1], [0, 1, 0]]),
            // +Y
            ([0.0, 1.0, 0.0], [[0, 1, 1], [1, 1, 1], [1, 1, 0], [0, 1, 0]]),
            // -Y
            ([0.0, -1.0, 0.0], [[0, 0, 0], [1, 0, 0], [1, 0, 1], [0, 0, 1]]),
            // +Z
            ([0.0, 0.0, 1.0], [[0, 0, 1], [1, 0, 1], [1, 1, 1], [0, 1, 1]]),
            // -Z
            ([0.0, 0.0, -1.0], [[1, 0, 0], [0, 0, 0], [0, 1, 0], [1, 1, 0]]),
        ];

        for x in 0..CHUNK_SIZE {
            for y in 0..CHUNK_SIZE {
                for z in 0..CHUNK_SIZE {
                    let block = self.get(x, y, z);
                    if block.is_air() {
                        continue;
                    }

                    for (normal, corners) in faces.iter() {
                        let nx = x as i32 + normal[0] as i32;
                        let ny = y as i32 + normal[1] as i32;
                        let nz = z as i32 + normal[2] as i32;

                        let neighbour_opaque = self.is_opaque_at(nx, ny, nz);

                        if neighbour_opaque {
                            continue;
                        }

                        // Identify the face's normal axis (0=X, 1=Y, 2=Z) and
                        // the two tangent axes we'll sample AO along.
                        let normal_axis = normal
                            .iter()
                            .position(|c| *c != 0.0)
                            .expect("face normal must have one non-zero component");
                        let tangent_axes = [(normal_axis + 1) % 3, (normal_axis + 2) % 3];
                        let normal_step = normal[normal_axis] as i32;

                        let color = block.color().to_f32_array();
                        let base = positions.len() as u32;

                        let mut ao_level = [0u8; 4];
                        for (i, offset) in corners.iter().enumerate() {
                            // Sign along each tangent axis: 0 -> -1, 1 -> +1.
                            let u_sign = 2 * offset[tangent_axes[0]] - 1;
                            let v_sign = 2 * offset[tangent_axes[1]] - 1;

                            let mut side1 = [0i32; 3];
                            let mut side2 = [0i32; 3];
                            let mut corner = [0i32; 3];
                            side1[normal_axis] = normal_step;
                            side2[normal_axis] = normal_step;
                            corner[normal_axis] = normal_step;
                            side1[tangent_axes[0]] = u_sign;
                            side2[tangent_axes[1]] = v_sign;
                            corner[tangent_axes[0]] = u_sign;
                            corner[tangent_axes[1]] = v_sign;

                            let s1 = self.is_opaque_at(
                                x as i32 + side1[0],
                                y as i32 + side1[1],
                                z as i32 + side1[2],
                            );
                            let s2 = self.is_opaque_at(
                                x as i32 + side2[0],
                                y as i32 + side2[1],
                                z as i32 + side2[2],
                            );
                            let c = self.is_opaque_at(
                                x as i32 + corner[0],
                                y as i32 + corner[1],
                                z as i32 + corner[2],
                            );

                            ao_level[i] = if s1 && s2 {
                                0
                            } else {
                                3 - (s1 as u8 + s2 as u8 + c as u8)
                            };
                        }

                        // AO level (0..=3) -> per-vertex brightness multiplier.
                        const AO_BRIGHTNESS: [f32; 4] = [0.55, 0.72, 0.87, 1.0];

                        for (i, offset) in corners.iter().enumerate() {
                            positions.push([
                                x as f32 + offset[0] as f32,
                                y as f32 + offset[1] as f32,
                                z as f32 + offset[2] as f32,
                            ]);
                            normals.push(*normal);
                            let b = AO_BRIGHTNESS[ao_level[i] as usize];
                            colors.push([color[0] * b, color[1] * b, color[2] * b, color[3]]);
                            uvs.push(match i {
                                0 => [0.0, 1.0],
                                1 => [1.0, 1.0],
                                2 => [1.0, 0.0],
                                _ => [0.0, 0.0],
                            });
                        }

                        // Flip the triangulation when the 1-3 diagonal has the
                        // stronger contrast; this keeps interpolated AO from
                        // tearing along the default 0-2 split.
                        let flip = (ao_level[0] as i32 + ao_level[2] as i32)
                            < (ao_level[1] as i32 + ao_level[3] as i32);
                        if flip {
                            indices.extend_from_slice(&[
                                base,
                                base + 1,
                                base + 3,
                                base + 1,
                                base + 2,
                                base + 3,
                            ]);
                        } else {
                            indices.extend_from_slice(&[
                                base,
                                base + 1,
                                base + 2,
                                base,
                                base + 2,
                                base + 3,
                            ]);
                        }
                    }
                }
            }
        }

        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
        mesh.insert_indices(Indices::U32(indices));
        mesh
    }
}

/// Height of the seabed surface, in world-blocks, at the given column.
fn seabed_height(wx: i32, wz: i32, seed: u32) -> i32 {
    let base = 6.0;
    let big = noise::fbm_2d(wx as f32 * 0.035, wz as f32 * 0.035, seed, 4) * 10.0;
    let detail = noise::fbm_2d(wx as f32 * 0.11, wz as f32 * 0.11, seed ^ 0x9E37, 2) * 2.0;
    (base + big + detail).round() as i32
}

/// Pick which block belongs at the given world cell.
fn pick_block(wx: i32, wy: i32, wz: i32, seabed: i32, seed: u32) -> BlockType {
    if wy > seabed {
        return BlockType::Air;
    }

    if wy == seabed {
        // Pockets of coral and kelp dotted across the sandy top layer.
        let decoration = noise::value_2d(wx as f32 * 0.31, wz as f32 * 0.31, seed ^ 0xC0FFEE);
        if decoration > 0.86 {
            return BlockType::Coral;
        }
        return BlockType::Sand;
    }

    if wy >= seabed - 2 {
        return BlockType::Sand;
    }

    if wy >= seabed - 4 {
        return BlockType::Dirt;
    }

    BlockType::Stone
}
