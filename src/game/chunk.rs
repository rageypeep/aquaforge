// src/game/chunk.rs

pub const CHUNK_SIZE: usize = 16; // Size of a chunk in blocks

#[derive(Clone, Copy, PartialEq)]
pub enum BlockType {
    Air,
    Dirt,
    Stone,
    Sand
}

pub struct Chunk {
    pub blocks: [[[BlockType; CHUNK_SIZE]; CHUNK_SIZE]; CHUNK_SIZE],
}

impl Chunk {
    pub fn new() -> Self {
        let mut blocks = [[[BlockType::Air; CHUNK_SIZE]; CHUNK_SIZE]; CHUNK_SIZE];

        // Example: Fill the bottom layer with dirt
        for x in 0..CHUNK_SIZE {
            for z in 0..CHUNK_SIZE {
                blocks[x][0][z] = BlockType::Dirt;
            }
        }

        // Add some stone and sand blocks
        for x in 0..CHUNK_SIZE {
            for y in 1..CHUNK_SIZE - 1 {
                for z in 0..CHUNK_SIZE {
                    if (x + y + z) % 5 == 0 {
                        blocks[x][y][z] = BlockType::Stone;
                    } else if (x + y + z) % 7 == 0 {
                        blocks[x][y][z] = BlockType::Sand;
                    }
                }
            }
        }

        Chunk { blocks }
    }
}
    