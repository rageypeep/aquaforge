//! Lookup from chunk grid coordinates to the ECS entity holding that chunk.

use std::collections::HashMap;
use std::collections::hash_map;

use bevy::prelude::*;

use super::chunk::CHUNK_SIZE;

/// Maps a chunk-grid coordinate (one unit per `CHUNK_SIZE` blocks) to the
/// entity that owns that chunk's data and mesh.
#[derive(Resource, Default)]
pub struct ChunkMap {
    entities: HashMap<IVec3, Entity>,
}

impl ChunkMap {
    pub fn insert(&mut self, chunk_pos: IVec3, entity: Entity) {
        self.entities.insert(chunk_pos, entity);
    }

    pub fn get(&self, chunk_pos: IVec3) -> Option<Entity> {
        self.entities.get(&chunk_pos).copied()
    }

    pub fn contains(&self, chunk_pos: IVec3) -> bool {
        self.entities.contains_key(&chunk_pos)
    }

    pub fn remove(&mut self, chunk_pos: IVec3) -> Option<Entity> {
        self.entities.remove(&chunk_pos)
    }

    pub fn iter(&self) -> hash_map::Iter<'_, IVec3, Entity> {
        self.entities.iter()
    }
}

/// Split a world-space block coordinate into `(chunk_pos, local_coords)`.
///
/// `local_coords` is always in `0..CHUNK_SIZE` on each axis.
pub fn world_block_to_chunk(world_block: IVec3) -> (IVec3, UVec3) {
    let size = CHUNK_SIZE as i32;
    let chunk_pos = IVec3::new(
        world_block.x.div_euclid(size),
        world_block.y.div_euclid(size),
        world_block.z.div_euclid(size),
    );
    let local = UVec3::new(
        world_block.x.rem_euclid(size) as u32,
        world_block.y.rem_euclid(size) as u32,
        world_block.z.rem_euclid(size) as u32,
    );
    (chunk_pos, local)
}
