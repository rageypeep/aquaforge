// src/game/chunk.rs

pub const CHUNK_SIZE: usize = 16; // Size of a chunk in blocks

#[derive(Clone, Copy, PartialEq)]
pub enum BlockType {
    Air,
    Dirt,
    Stone,
    Sand,
    Coral,
    Seaweed,
}

impl BlockType {
    pub fn from_name(name: &str) -> Self {
        match name {
            "Dirt" => BlockType::Dirt,
            "Stone" => BlockType::Stone,
            "Sand" => BlockType::Sand,
            "Coral" => BlockType::Coral,
            "Seaweed" => BlockType::Seaweed,
            _ => BlockType::Air,
        }
    }
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

    pub fn from_db(conn: &rusqlite::Connection, chunk_id: i32) -> Self {
        let mut blocks = [[[BlockType::Air; CHUNK_SIZE]; CHUNK_SIZE]; CHUNK_SIZE];

        let mut stmt = conn
            .prepare(
                "SELECT x, y, z, name FROM chunk_blocks \
                 JOIN block_types ON block_types.id = chunk_blocks.block_type_id \
                 WHERE chunk_id = ?1",
            )
            .unwrap();
        let rows = stmt
            .query_map([chunk_id], |row| {
                Ok((
                    row.get::<_, i32>(0)?,
                    row.get::<_, i32>(1)?,
                    row.get::<_, i32>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .unwrap();

        for row in rows {
            let (x, y, z, name) = row.unwrap();
            if x < CHUNK_SIZE as i32 && y < CHUNK_SIZE as i32 && z < CHUNK_SIZE as i32 {
                blocks[x as usize][y as usize][z as usize] = BlockType::from_name(&name);
            }
        }

        Chunk { blocks }
    }
}
    