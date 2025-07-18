pub mod chunk;
use chunk::{Chunk, BlockType, CHUNK_SIZE};
use bevy::prelude::*;
use bevy::render::mesh::{Mesh, PrimitiveTopology, Indices};
use bevy::render::render_asset::RenderAssetUsages;

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_chunk_mesh);
    }
}

fn spawn_chunk_mesh(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let chunk = Chunk::new();

    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut colors = Vec::new();
    let mut indices = Vec::new();

    // Offsets and face normals for all 6 faces
    let faces = [
        // Normal, offsets for quad corners
        // +X
        ([1.0, 0.0, 0.0], [[1,0,0],[1,0,1],[1,1,1],[1,1,0]]),
        // -X
        ([-1.0, 0.0, 0.0], [[0,0,1],[0,0,0],[0,1,0],[0,1,1]]),
        // +Y
        ([0.0, 1.0, 0.0], [[0,1,1],[1,1,1],[1,1,0],[0,1,0]]),
        // -Y
        ([0.0, -1.0, 0.0], [[0,0,0],[1,0,0],[1,0,1],[0,0,1]]),
        // +Z
        ([0.0, 0.0, 1.0], [[0,0,1],[1,0,1],[1,1,1],[0,1,1]]),
        // -Z
        ([0.0, 0.0, -1.0], [[1,0,0],[0,0,0],[0,1,0],[1,1,0]]),
    ];

    for x in 0..CHUNK_SIZE {
        for y in 0..CHUNK_SIZE {
            for z in 0..CHUNK_SIZE {
                let block = chunk.blocks[x][y][z];
                if block == BlockType::Air { continue; }

                // For each face...
                for (normal, corners) in faces.iter() {
                    let nx = x as isize + normal[0] as isize;
                    let ny = y as isize + normal[1] as isize;
                    let nz = z as isize + normal[2] as isize;

                    let exposed = nx < 0
                        || nx >= CHUNK_SIZE as isize
                        || ny < 0
                        || ny >= CHUNK_SIZE as isize
                        || nz < 0
                        || nz >= CHUNK_SIZE as isize
                        || chunk.blocks[nx as usize][ny as usize][nz as usize] == BlockType::Air;

                    if exposed {
                        // Add quad (two triangles)
                        let color = match block {
                            BlockType::Stone => [0.2, 0.2, 0.25, 1.0],
                            BlockType::Sand => [0.8, 0.8, 0.2, 1.0],
                            BlockType::Dirt => [0.5, 0.3, 0.1, 1.0],
                            _ => [1.0, 1.0, 1.0, 1.0],
                        };
                        let base = positions.len() as u32;
                        for &offset in corners {
                            positions.push([
                                x as f32 + offset[0] as f32,
                                y as f32 + offset[1] as f32,
                                z as f32 + offset[2] as f32,
                            ]);
                            normals.push(*normal);
                            colors.push(color);
                        }
                        // Two triangles per face
                        indices.extend_from_slice(&[
                            base, base + 1, base + 2,
                            base, base + 2, base + 3,
                        ]);
                    }
                }
            }
        }
    }

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));

    let material = materials.add(StandardMaterial {
        perceptual_roughness: 1.0,
        metallic: 0.0,
        unlit: true,
        ..default()
    });

    commands.spawn(PbrBundle {
        mesh: meshes.add(mesh),
        material,
        ..default()
    });
}
