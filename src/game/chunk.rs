//! Chunks: fixed-size 3D arrays of blocks plus greedy mesh generation.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, Mesh, PrimitiveTopology};
use bevy::prelude::*;

use super::blocks::BlockType;
use crate::rendering::atlas;
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

    /// Build a mesh from this chunk's non-air blocks using a greedy mesher
    /// with per-vertex ambient occlusion.
    ///
    /// Each of the 6 axis-aligned face directions is swept one 2D slice at
    /// a time. For every cell whose face is exposed we precompute the
    /// classic three-neighbour AO value at each of the 4 face corners and
    /// use the `(BlockType, [ao;4])` tuple as the merge key — neighbouring
    /// cells only fuse into the same quad when both their material and
    /// their corner AO patterns agree. That keeps AO pixel-identical to
    /// the per-face mesher on broken ground while still collapsing large
    /// flat shelves (where every corner sits at AO=3) into single quads.
    ///
    /// Out-of-chunk neighbours are treated as transparent so the chunk's
    /// outer shell is always drawn — adjacent chunks' opposite-facing
    /// shells occupy the same plane but are back-face culled from their
    /// respective sides.
    pub fn build_mesh(&self) -> Mesh {
        let mut positions: Vec<[f32; 3]> = Vec::new();
        let mut normals: Vec<[f32; 3]> = Vec::new();
        let mut colors: Vec<[f32; 4]> = Vec::new();
        let mut uvs: Vec<[f32; 2]> = Vec::new();
        let mut indices: Vec<u32> = Vec::new();

        for face in FACES.iter() {
            self.greedy_sweep_face(
                face,
                &mut positions,
                &mut normals,
                &mut colors,
                &mut uvs,
                &mut indices,
            );
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

    /// Greedy-mesh every slice perpendicular to `face.d0` for one face
    /// direction, pushing merged quads into the buffers.
    fn greedy_sweep_face(
        &self,
        face: &FaceDef,
        positions: &mut Vec<[f32; 3]>,
        normals: &mut Vec<[f32; 3]>,
        colors: &mut Vec<[f32; 4]>,
        uvs: &mut Vec<[f32; 2]>,
        indices: &mut Vec<u32>,
    ) {
        let mut mask = vec![[None::<MaskCell>; CHUNK_SIZE]; CHUNK_SIZE];

        for s in 0..CHUNK_SIZE {
            // Populate the mask for this slice: each cell that has an
            // exposed face in this direction stores its BlockType plus the
            // 4-corner AO pattern for that face.
            for (u, row) in mask.iter_mut().enumerate() {
                for (v, cell_mask) in row.iter_mut().enumerate() {
                    *cell_mask = self.face_mask_cell(face, s, u, v);
                }
            }

            // Sweep the mask, emitting merged quads. A rect grows along d1
            // first, then d2, only while every covered cell has the same
            // (block, ao) tuple.
            for v in 0..CHUNK_SIZE {
                let mut u = 0;
                while u < CHUNK_SIZE {
                    let Some(cell) = mask[u][v] else {
                        u += 1;
                        continue;
                    };

                    // Maximal width along d1.
                    let mut du = 1;
                    while u + du < CHUNK_SIZE && mask[u + du][v] == Some(cell) {
                        du += 1;
                    }

                    // Maximal height along d2.
                    let mut dv = 1;
                    'grow: while v + dv < CHUNK_SIZE {
                        for uu in 0..du {
                            if mask[u + uu][v + dv] != Some(cell) {
                                break 'grow;
                            }
                        }
                        dv += 1;
                    }

                    emit_quad(
                        face,
                        s,
                        u,
                        u + du,
                        v,
                        v + dv,
                        cell,
                        positions,
                        normals,
                        colors,
                        uvs,
                        indices,
                    );

                    for vv in 0..dv {
                        for uu in 0..du {
                            mask[u + uu][v + vv] = None;
                        }
                    }

                    u += du;
                }
            }
        }
    }

    /// Mask contribution for one cell on a slice. Returns `None` if the
    /// cell is air or its face in this direction is culled by an opaque
    /// neighbour; otherwise returns the `MaskCell` that drives greedy
    /// merging.
    fn face_mask_cell(&self, face: &FaceDef, s: usize, u: usize, v: usize) -> Option<MaskCell> {
        let mut cell = [0usize; 3];
        cell[face.d0] = s;
        cell[face.d1] = u;
        cell[face.d2] = v;

        let block = self.get(cell[0], cell[1], cell[2]);
        if block.is_air() {
            return None;
        }

        // Face is culled if the neighbour one step along d0 in the face
        // direction is opaque.
        let mut nb = [cell[0] as i32, cell[1] as i32, cell[2] as i32];
        nb[face.d0] += face.sign;
        if self.is_opaque_at(nb[0], nb[1], nb[2]) {
            return None;
        }

        // Compute AO at each of the 4 face corners, stored in the same
        // order as `face.corner_ends` so `emit_quad` can index them by
        // vertex index directly.
        let mut ao = [0u8; 4];
        for (i, corner_end) in face.corner_ends.iter().enumerate() {
            ao[i] = self.corner_ao(face, cell, corner_end);
        }

        Some(MaskCell { block, ao })
    }

    /// Classic 3-neighbour AO for one face corner, returning 0 (fully
    /// occluded) to 3 (no occlusion). `cell` is the block the face
    /// belongs to, `face` describes which of the 6 face directions we're
    /// on, and `corner_end[0..2]` picks which side of the face rect the
    /// corner sits on (0 = `u0/v0` edge, 1 = `u1/v1` edge).
    fn corner_ao(&self, face: &FaceDef, cell: [usize; 3], corner_end: &[u32; 2]) -> u8 {
        // -1 when on the low edge of the face, +1 when on the high edge.
        let u_sign = if corner_end[0] == 0 { -1 } else { 1 };
        let v_sign = if corner_end[1] == 0 { -1 } else { 1 };

        let mut sample = |d1_off: i32, d2_off: i32| -> bool {
            let mut p = [cell[0] as i32, cell[1] as i32, cell[2] as i32];
            p[face.d0] += face.sign;
            p[face.d1] += d1_off;
            p[face.d2] += d2_off;
            self.is_opaque_at(p[0], p[1], p[2])
        };

        let s1 = sample(u_sign, 0);
        let s2 = sample(0, v_sign);
        let c = sample(u_sign, v_sign);

        if s1 && s2 {
            0
        } else {
            3 - (s1 as u8 + s2 as u8 + c as u8)
        }
    }
}

/// One cell in the face-sweep mask. Two mask cells merge into the same
/// greedy rectangle only when both fields are equal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct MaskCell {
    block: BlockType,
    /// AO value (0..=3) at each of the face's 4 corners, in the same
    /// order as the face's `corner_ends`.
    ao: [u8; 4],
}

/// Describes one of the six axis-aligned face directions of a unit cube.
struct FaceDef {
    /// Axis perpendicular to the face (0 = X, 1 = Y, 2 = Z).
    d0: usize,
    /// First in-plane axis.
    d1: usize,
    /// Second in-plane axis.
    d2: usize,
    /// +1 for positive-pointing faces, -1 for negative-pointing ones.
    sign: i32,
    /// Outward-pointing face normal as f32.
    normal: [f32; 3],
    /// Which corner of the face rectangle each emitted vertex sits at,
    /// expressed as a `[u_end, v_end]` pair where `0` = (u0, v0) end of the
    /// rect and `1` = (u1, v1) end. Wound counter-clockwise when viewed
    /// from outside so back-face culling behaves.
    corner_ends: [[u32; 2]; 4],
}

const FACES: [FaceDef; 6] = [
    // +X
    FaceDef {
        d0: 0,
        d1: 1,
        d2: 2,
        sign: 1,
        normal: [1.0, 0.0, 0.0],
        corner_ends: [[0, 1], [0, 0], [1, 0], [1, 1]],
    },
    // -X
    FaceDef {
        d0: 0,
        d1: 1,
        d2: 2,
        sign: -1,
        normal: [-1.0, 0.0, 0.0],
        corner_ends: [[0, 0], [0, 1], [1, 1], [1, 0]],
    },
    // +Y
    FaceDef {
        d0: 1,
        d1: 0,
        d2: 2,
        sign: 1,
        normal: [0.0, 1.0, 0.0],
        corner_ends: [[0, 1], [1, 1], [1, 0], [0, 0]],
    },
    // -Y
    FaceDef {
        d0: 1,
        d1: 0,
        d2: 2,
        sign: -1,
        normal: [0.0, -1.0, 0.0],
        corner_ends: [[0, 0], [1, 0], [1, 1], [0, 1]],
    },
    // +Z
    FaceDef {
        d0: 2,
        d1: 0,
        d2: 1,
        sign: 1,
        normal: [0.0, 0.0, 1.0],
        corner_ends: [[0, 0], [1, 0], [1, 1], [0, 1]],
    },
    // -Z
    FaceDef {
        d0: 2,
        d1: 0,
        d2: 1,
        sign: -1,
        normal: [0.0, 0.0, -1.0],
        corner_ends: [[1, 0], [0, 0], [0, 1], [1, 1]],
    },
];

/// AO level (0..=3) -> per-vertex brightness multiplier. Shared with the
/// AO implementation this greedy mesher replaced so pixel output is
/// identical on non-merged faces.
const AO_BRIGHTNESS: [f32; 4] = [0.55, 0.72, 0.87, 1.0];

#[allow(clippy::too_many_arguments)]
fn emit_quad(
    face: &FaceDef,
    s: usize,
    u0: usize,
    u1: usize,
    v0: usize,
    v1: usize,
    cell: MaskCell,
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    colors: &mut Vec<[f32; 4]>,
    uvs: &mut Vec<[f32; 2]>,
    indices: &mut Vec<u32>,
) {
    // Position along the perpendicular axis: +1 faces live at s+1, -1 at s.
    let d0_plane = if face.sign > 0 { s + 1 } else { s } as f32;

    let (uv_min, uv_max) = atlas::tile_uv_rect(cell.block);
    let base = positions.len() as u32;

    for (i, corner) in face.corner_ends.iter().enumerate() {
        let u = if corner[0] == 0 { u0 } else { u1 };
        let v = if corner[1] == 0 { v0 } else { v1 };

        let mut pos = [0.0_f32; 3];
        pos[face.d0] = d0_plane;
        pos[face.d1] = u as f32;
        pos[face.d2] = v as f32;
        positions.push(pos);

        normals.push(face.normal);

        // The atlas texture carries the block's hue, so the vertex
        // colour is now a pure AO modulator.
        let b = AO_BRIGHTNESS[cell.ao[i] as usize];
        colors.push([b, b, b, 1.0]);

        // UV is the block's tile rect stretched to cover the whole
        // merged quad. Per-block tiling on merged quads is a follow-up
        // that needs a custom shader (`fract(uv) * tile + offset`); for
        // now a single tile stretched across e.g. a sand shelf reads as
        // "sandy" at play distance without any new render plumbing.
        let u_norm = if corner[0] == 0 { uv_min.x } else { uv_max.x };
        let v_norm = if corner[1] == 0 { uv_min.y } else { uv_max.y };
        uvs.push([u_norm, v_norm]);
    }

    // Flip the triangulation when the 1-3 diagonal has the stronger AO
    // contrast; this keeps interpolated AO from tearing along the default
    // 0-2 split.
    let flip = (cell.ao[0] as i32 + cell.ao[2] as i32) < (cell.ao[1] as i32 + cell.ao[3] as i32);
    if flip {
        indices.extend_from_slice(&[base, base + 1, base + 3, base + 1, base + 2, base + 3]);
    } else {
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
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

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::mesh::VertexAttributeValues;

    fn fill(block: BlockType) -> Chunk {
        let mut c = Chunk::empty();
        for x in 0..CHUNK_SIZE {
            for y in 0..CHUNK_SIZE {
                for z in 0..CHUNK_SIZE {
                    c.set(x, y, z, block);
                }
            }
        }
        c
    }

    fn vertex_count(mesh: &Mesh) -> usize {
        match mesh
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .expect("position attribute missing")
        {
            VertexAttributeValues::Float32x3(v) => v.len(),
            _ => panic!("unexpected position attribute type"),
        }
    }

    fn index_count(mesh: &Mesh) -> usize {
        match mesh.indices().expect("mesh has no indices") {
            Indices::U32(v) => v.len(),
            Indices::U16(v) => v.len(),
        }
    }

    /// An empty chunk has no geometry.
    #[test]
    fn empty_chunk_has_no_geometry() {
        let mesh = Chunk::empty().build_mesh();
        assert_eq!(vertex_count(&mesh), 0);
        assert_eq!(index_count(&mesh), 0);
    }

    /// A fully-solid chunk only draws its 6 outer shell faces, one giant
    /// greedy quad each — AO at every corner is 3 (out-of-chunk cells
    /// count as non-opaque) so the whole shell merges. 6 quads → 24
    /// vertices / 36 indices.
    #[test]
    fn solid_chunk_emits_single_quad_per_face() {
        let mesh = fill(BlockType::Stone).build_mesh();
        assert_eq!(vertex_count(&mesh), 24);
        assert_eq!(index_count(&mesh), 36);
    }

    /// A solid chunk split half Stone / half Sand along x emits one quad
    /// per shell half. Stone and Sand are both opaque so the internal
    /// seam is mutually culled, and all shell-corner AO values are 3 so
    /// the halves each merge to a single rectangle.
    #[test]
    fn split_chunk_emits_two_block_types() {
        let mut c = Chunk::empty();
        for x in 0..CHUNK_SIZE {
            for y in 0..CHUNK_SIZE {
                for z in 0..CHUNK_SIZE {
                    let b = if x < CHUNK_SIZE / 2 {
                        BlockType::Stone
                    } else {
                        BlockType::Sand
                    };
                    c.set(x, y, z, b);
                }
            }
        }
        let mesh = c.build_mesh();
        // Quads:
        //   +X: 1 (sand shell)        -X: 1 (stone shell)
        //   +Y: 2   -Y: 2   +Z: 2   -Z: 2   (half stone + half sand per face)
        // Total 10 quads → 40 vertices / 60 indices.
        assert_eq!(vertex_count(&mesh), 40);
        assert_eq!(index_count(&mesh), 60);
    }

    /// A single lone block still meshes to exactly 6 quads (one per face)
    /// so break/place interactions remain intact.
    #[test]
    fn single_block_emits_six_faces() {
        let mut c = Chunk::empty();
        c.set(5, 5, 5, BlockType::Stone);
        let mesh = c.build_mesh();
        assert_eq!(vertex_count(&mesh), 24);
        assert_eq!(index_count(&mesh), 36);
    }

    /// An L-shape of two blocks sharing one edge produces an AO-darkened
    /// inner corner — greedy merging must not collapse cells whose AO
    /// corners differ.
    #[test]
    fn ao_variation_prevents_incorrect_merge() {
        let mut c = Chunk::empty();
        // A 2x1 row along X with a third block stacked on top of the left
        // cell. The top faces of both row cells differ in AO: the left
        // cell has the stacked block covering its +Y face entirely (face
        // culled), while the right cell's top face has an opaque corner
        // neighbour on its (-X, +Y) side, darkening one corner.
        c.set(5, 5, 5, BlockType::Stone);
        c.set(6, 5, 5, BlockType::Stone);
        c.set(5, 6, 5, BlockType::Stone);
        let mesh = c.build_mesh();
        let quads = vertex_count(&mesh) / 4;
        // Three isolated stone cubes could emit up to 18 quads without
        // any merging. With greedy merging but per-cell AO variation the
        // count should still be strictly less than that but well above
        // the 6 quads a naive single-cube would produce. This test
        // mostly guards against the two degenerate failure modes: the
        // mesher silently merging cells with different AO patterns (too
        // few quads, < 8) or losing faces entirely (< 6).
        assert!(
            (8..=18).contains(&quads),
            "expected 8..=18 quads for a 3-block L-shape, got {}",
            quads
        );
    }

    /// The generated seabed should mesh to drastically fewer faces with
    /// the greedy mesher than the per-face count would be. Guard against
    /// accidentally regressing to the naive mesher. AO variation along
    /// ridges blocks some merges so the cap is more generous than the
    /// flat case, but it's still well under the naive face count.
    #[test]
    fn generated_chunk_is_well_below_naive_face_count() {
        let chunk = Chunk::generate(IVec3::new(0, 0, 0), 0xAFE1_0E6A);
        let mesh = chunk.build_mesh();
        let quads = vertex_count(&mesh) / 4;
        assert!(
            quads < 1024,
            "greedy mesher emitted {} quads, which is not greedy enough",
            quads
        );
    }
}
