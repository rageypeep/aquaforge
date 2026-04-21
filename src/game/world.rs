//! Spawning and wiring of voxel chunks into the Bevy world.

use bevy::prelude::*;

use super::chunk::{CHUNK_SIZE, Chunk};
use super::chunk_map::ChunkMap;

/// Number of chunks laid out along each horizontal axis, centred on the origin.
pub const WORLD_CHUNKS_XZ: i32 = 6;
/// Number of chunks stacked vertically.
pub const WORLD_CHUNKS_Y: i32 = 2;

/// Seed used by the world generator.
pub const WORLD_SEED: u32 = 0xAFE1_0E6A_u32;

/// World-space Y coordinate of the water surface.
pub const WATER_LEVEL: f32 = (WORLD_CHUNKS_Y as f32) * (CHUNK_SIZE as f32) - 2.0;

/// Marker component for an entity that represents one generated chunk.
#[derive(Component, Debug)]
pub struct ChunkTag(#[allow(dead_code)] pub IVec3);

pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ChunkMap>()
            .add_systems(Startup, spawn_world);
    }
}

fn spawn_world(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut chunk_map: ResMut<ChunkMap>,
) {
    let block_material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.95,
        metallic: 0.0,
        reflectance: 0.05,
        ..default()
    });

    let half = WORLD_CHUNKS_XZ / 2;
    for cx in -half..half {
        for cz in -half..half {
            for cy in 0..WORLD_CHUNKS_Y {
                let chunk_pos = IVec3::new(cx, cy, cz);
                let chunk = Chunk::generate(chunk_pos, WORLD_SEED);
                let mesh = chunk.build_mesh();

                let origin = Vec3::new(
                    (cx * CHUNK_SIZE as i32) as f32,
                    (cy * CHUNK_SIZE as i32) as f32,
                    (cz * CHUNK_SIZE as i32) as f32,
                );

                let entity = commands
                    .spawn((
                        Mesh3d(meshes.add(mesh)),
                        MeshMaterial3d(block_material.clone()),
                        Transform::from_translation(origin),
                        ChunkTag(chunk_pos),
                        chunk,
                        Name::new(format!("Chunk ({cx},{cy},{cz})")),
                    ))
                    .id();
                chunk_map.insert(chunk_pos, entity);
            }
        }
    }
}
