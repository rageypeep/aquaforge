//! Spawning, streaming, and wiring of voxel chunks into the Bevy world.
//!
//! Chunks are loaded on demand: each frame we look at the player camera's
//! chunk coordinate and ensure every chunk within a square horizontal
//! radius (and a fixed vertical range) is spawned. Chunks that drift
//! outside that radius — plus a one-chunk hysteresis band to avoid
//! thrashing at the boundary — are despawned and their entities removed
//! from [`ChunkMap`]. Generation and despawn work is capped per frame so
//! a distant teleport (or the initial fill) doesn't stall the frame.

use bevy::prelude::*;

use super::chunk::{CHUNK_SIZE, Chunk};
use super::chunk_map::ChunkMap;
use crate::rendering::atlas::BlockAtlas;

/// Vertical extent of the world, in chunks. The seabed and water column
/// fit inside `0..VERTICAL_CHUNKS`, so streaming only varies the chunk
/// coordinate on the horizontal axes.
pub const VERTICAL_CHUNKS: i32 = 2;

/// Seed used by the world generator.
pub const WORLD_SEED: u32 = 0xAFE1_0E6A_u32;

/// World-space Y coordinate of the water surface. Kept as an absolute
/// constant so everything (sea surface, camera start, fog density) can
/// pin off it without coupling to the vertical chunk count.
pub const WATER_LEVEL: f32 = (VERTICAL_CHUNKS as f32) * (CHUNK_SIZE as f32) - 2.0;

/// Marker component for an entity that represents one generated chunk.
#[derive(Component, Debug)]
pub struct ChunkTag(#[allow(dead_code)] pub IVec3);

/// Shared PBR material every chunk mesh renders with. Built once at
/// startup so we never hit `Assets<StandardMaterial>` during streaming.
#[derive(Resource)]
pub struct ChunkMaterial(pub Handle<StandardMaterial>);

/// Tunables for the on-demand chunk streamer.
#[derive(Resource, Debug, Clone, Copy)]
pub struct StreamingConfig {
    /// Load every chunk whose horizontal chunk-distance to the camera is
    /// `<= horizontal_radius` (Chebyshev distance, i.e. the chunks form a
    /// square ring around the camera).
    pub horizontal_radius: i32,
    /// Extra Chebyshev-distance band inside which an already-loaded
    /// chunk is kept alive. Prevents thrashing when the camera sits on a
    /// chunk boundary.
    pub despawn_hysteresis: i32,
    /// Cap on chunks generated-and-spawned per frame. Closest chunks win
    /// ties so the camera's immediate surroundings pop in first.
    pub max_spawns_per_frame: usize,
    /// Cap on chunks despawned per frame. Despawns are cheap, so this is
    /// usually higher than the spawn cap.
    pub max_despawns_per_frame: usize,
}

impl Default for StreamingConfig {
    fn default() -> Self {
        Self {
            horizontal_radius: 6,
            despawn_hysteresis: 1,
            max_spawns_per_frame: 4,
            max_despawns_per_frame: 8,
        }
    }
}

pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ChunkMap>()
            .init_resource::<StreamingConfig>()
            .add_systems(Startup, spawn_chunk_material)
            .add_systems(Update, stream_chunks);
    }
}

fn spawn_chunk_material(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    atlas: Res<BlockAtlas>,
) {
    let handle = materials.add(StandardMaterial {
        // White base_color so the atlas texture and per-vertex AO are
        // the only things driving the surface colour.
        base_color: Color::WHITE,
        base_color_texture: Some(atlas.0.clone()),
        perceptual_roughness: 0.95,
        metallic: 0.0,
        reflectance: 0.05,
        ..default()
    });
    commands.insert_resource(ChunkMaterial(handle));
}

fn stream_chunks(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut chunk_map: ResMut<ChunkMap>,
    config: Res<StreamingConfig>,
    material: Res<ChunkMaterial>,
    cameras: Query<&GlobalTransform, With<Camera3d>>,
) {
    let Ok(cam) = cameras.single() else {
        return;
    };
    let center = camera_chunk_center(cam.translation());

    // Spawn missing chunks within the load radius, closest first.
    let mut to_spawn = missing_chunks_in_radius(center, config.horizontal_radius, &chunk_map);
    sort_by_distance_to(&mut to_spawn, center);
    for chunk_pos in to_spawn.into_iter().take(config.max_spawns_per_frame) {
        let entity = spawn_chunk(&mut commands, &mut meshes, &material, chunk_pos);
        chunk_map.insert(chunk_pos, entity);
    }

    // Despawn chunks that fall outside the load radius + hysteresis band.
    let drop_radius = config.horizontal_radius + config.despawn_hysteresis;
    let mut to_drop = loaded_chunks_outside_radius(&chunk_map, center, drop_radius);
    // Despawn furthest first so the ring contracts evenly.
    to_drop.sort_by_key(|c| -chebyshev_xz(*c, center));
    for chunk_pos in to_drop.into_iter().take(config.max_despawns_per_frame) {
        if let Some(entity) = chunk_map.remove(chunk_pos) {
            commands.entity(entity).despawn();
        }
    }
}

fn spawn_chunk(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    material: &ChunkMaterial,
    chunk_pos: IVec3,
) -> Entity {
    let chunk = Chunk::generate(chunk_pos, WORLD_SEED);
    let mesh = chunk.build_mesh();
    let origin = Vec3::new(
        (chunk_pos.x * CHUNK_SIZE as i32) as f32,
        (chunk_pos.y * CHUNK_SIZE as i32) as f32,
        (chunk_pos.z * CHUNK_SIZE as i32) as f32,
    );

    commands
        .spawn((
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(material.0.clone()),
            Transform::from_translation(origin),
            ChunkTag(chunk_pos),
            chunk,
            Name::new(format!(
                "Chunk ({},{},{})",
                chunk_pos.x, chunk_pos.y, chunk_pos.z
            )),
        ))
        .id()
}

/// Chunk coordinate a camera at `pos` sits inside.
///
/// Y is snapped into the valid vertical range so a camera drifting above
/// the water or below the world still streams the top/bottom slab.
pub fn camera_chunk_center(pos: Vec3) -> IVec3 {
    let size = CHUNK_SIZE as f32;
    IVec3::new(
        (pos.x / size).floor() as i32,
        ((pos.y / size).floor() as i32).clamp(0, VERTICAL_CHUNKS - 1),
        (pos.z / size).floor() as i32,
    )
}

/// Chebyshev distance between two chunk coords projected onto the XZ
/// plane. Y is ignored because the world only has a few fixed vertical
/// slices.
pub fn chebyshev_xz(a: IVec3, b: IVec3) -> i32 {
    (a.x - b.x).abs().max((a.z - b.z).abs())
}

/// All chunk coords within `radius` (inclusive, Chebyshev) of `center`
/// and within the valid vertical range.
pub fn chunks_in_radius(center: IVec3, radius: i32) -> Vec<IVec3> {
    let mut out = Vec::with_capacity(((2 * radius + 1).pow(2) * VERTICAL_CHUNKS) as usize);
    for dx in -radius..=radius {
        for dz in -radius..=radius {
            for cy in 0..VERTICAL_CHUNKS {
                out.push(IVec3::new(center.x + dx, cy, center.z + dz));
            }
        }
    }
    out
}

fn missing_chunks_in_radius(center: IVec3, radius: i32, chunk_map: &ChunkMap) -> Vec<IVec3> {
    chunks_in_radius(center, radius)
        .into_iter()
        .filter(|c| !chunk_map.contains(*c))
        .collect()
}

fn loaded_chunks_outside_radius(chunk_map: &ChunkMap, center: IVec3, radius: i32) -> Vec<IVec3> {
    chunk_map
        .iter()
        .filter_map(|(pos, _)| {
            if chebyshev_xz(*pos, center) > radius {
                Some(*pos)
            } else {
                None
            }
        })
        .collect()
}

/// Sort in place by ascending Chebyshev distance to `center`, so closer
/// chunks get preferentially spawned each frame.
fn sort_by_distance_to(chunks: &mut [IVec3], center: IVec3) {
    chunks.sort_by_key(|c| chebyshev_xz(*c, center));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_chunk_center_clamps_y() {
        assert_eq!(
            camera_chunk_center(Vec3::new(0.0, -5.0, 0.0)),
            IVec3::new(0, 0, 0)
        );
        assert_eq!(
            camera_chunk_center(Vec3::new(0.0, 10_000.0, 0.0)),
            IVec3::new(0, VERTICAL_CHUNKS - 1, 0)
        );
    }

    #[test]
    fn camera_chunk_center_uses_floor_division() {
        // -0.1 lives in chunk -1, not chunk 0.
        assert_eq!(camera_chunk_center(Vec3::new(-0.1, 10.0, 0.0)).x, -1);
        // 16.5 is the first block of chunk 1.
        assert_eq!(camera_chunk_center(Vec3::new(16.5, 10.0, 0.0)).x, 1);
    }

    #[test]
    fn chunks_in_radius_covers_expected_ring() {
        let chunks = chunks_in_radius(IVec3::new(0, 0, 0), 2);
        let expected = (2 * 2 + 1) * (2 * 2 + 1) * VERTICAL_CHUNKS;
        assert_eq!(chunks.len() as i32, expected);
        for c in &chunks {
            assert!((0..VERTICAL_CHUNKS).contains(&c.y));
            assert!(c.x.abs() <= 2 && c.z.abs() <= 2);
        }
    }

    #[test]
    fn chebyshev_xz_ignores_y() {
        let a = IVec3::new(3, 0, -1);
        let b = IVec3::new(0, 7, 2);
        assert_eq!(chebyshev_xz(a, b), 3);
    }

    #[test]
    fn missing_filter_excludes_loaded_chunks() {
        let center = IVec3::new(0, 0, 0);
        let mut map = ChunkMap::default();
        // Pretend the immediate 3×3 horizontal ring at y=0 is already loaded.
        for dx in -1..=1 {
            for dz in -1..=1 {
                map.insert(IVec3::new(dx, 0, dz), Entity::PLACEHOLDER);
            }
        }
        let missing = missing_chunks_in_radius(center, 1, &map);
        // With `VERTICAL_CHUNKS = 2` the y=1 layer is still missing.
        assert_eq!(missing.len() as i32, (2 * 1 + 1) * (2 * 1 + 1));
        for c in missing {
            assert_eq!(c.y, 1);
        }
    }

    #[test]
    fn outside_radius_filter_picks_up_drifted_chunks() {
        let center = IVec3::new(0, 0, 0);
        let mut map = ChunkMap::default();
        map.insert(IVec3::new(10, 0, 0), Entity::PLACEHOLDER);
        map.insert(IVec3::new(2, 0, 0), Entity::PLACEHOLDER);
        map.insert(IVec3::new(-2, 1, 0), Entity::PLACEHOLDER);

        // With radius 3, only (10,0,0) is outside — the (-2,1,0) entry
        // also verifies that vertical position doesn't count against the
        // horizontal radius.
        let outside = loaded_chunks_outside_radius(&map, center, 3);
        assert_eq!(outside, vec![IVec3::new(10, 0, 0)]);
    }

    #[test]
    fn sort_by_distance_puts_closest_first() {
        let center = IVec3::new(0, 0, 0);
        let mut chunks = vec![
            IVec3::new(3, 0, 0),
            IVec3::new(1, 0, 0),
            IVec3::new(5, 0, 0),
            IVec3::new(0, 0, 2),
        ];
        sort_by_distance_to(&mut chunks, center);
        assert_eq!(
            chunks,
            vec![
                IVec3::new(1, 0, 0),
                IVec3::new(0, 0, 2),
                IVec3::new(3, 0, 0),
                IVec3::new(5, 0, 0),
            ]
        );
    }
}
