//! Gameplay: voxel world, blocks, chunks, and world generation.

use bevy::prelude::*;

pub mod blocks;
pub mod chunk;
pub mod chunk_map;
pub mod edit;
pub mod inventory;
pub mod world;

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            world::WorldPlugin,
            inventory::InventoryPlugin,
            edit::ChunkEditPlugin,
        ));
    }
}
