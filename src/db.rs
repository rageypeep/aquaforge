use bevy::prelude::*;
use rusqlite::{Connection, Result};
use std::sync::{Arc, Mutex};

#[derive(Resource, Clone)]
pub struct Db(pub Arc<Mutex<Connection>>);

pub struct DatabasePlugin;

impl Plugin for DatabasePlugin {
    fn build(&self, app: &mut App) {
        let conn = Connection::open_in_memory().expect("create db");
        initialize_database(&conn).expect("init db");
        app.insert_resource(Db(Arc::new(Mutex::new(conn))));
    }
}

fn initialize_database(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS block_types (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            color_r REAL NOT NULL,
            color_g REAL NOT NULL,
            color_b REAL NOT NULL
        );
        CREATE TABLE IF NOT EXISTS chunks (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS chunk_blocks (
            chunk_id INTEGER NOT NULL,
            x INTEGER NOT NULL,
            y INTEGER NOT NULL,
            z INTEGER NOT NULL,
            block_type_id INTEGER NOT NULL,
            PRIMARY KEY(chunk_id, x, y, z)
        );"
    )?;

    // Insert block types if table empty
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM block_types", [], |r| r.get(0))?;
    if count == 0 {
        let blocks = [
            (1, "Air", 1.0, 1.0, 1.0),
            (2, "Dirt", 0.5, 0.3, 0.1),
            (3, "Stone", 0.2, 0.2, 0.25),
            (4, "Sand", 0.8, 0.8, 0.2),
            (5, "Coral", 0.9, 0.2, 0.5),
            (6, "Seaweed", 0.1, 0.8, 0.1),
        ];
        for b in blocks.iter() {
            conn.execute(
                "INSERT INTO block_types (id, name, color_r, color_g, color_b) VALUES (?1, ?2, ?3, ?4, ?5)",
                (b.0, b.1, b.2, b.3, b.4),
            )?;
        }
    }

    // Insert one chunk if empty
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM chunks", [], |r| r.get(0))?;
    if count == 0 {
        conn.execute("INSERT INTO chunks (id, name) VALUES (1, 'start')", [])?;
        // Fill with air
        for x in 0..16 {
            for y in 0..16 {
                for z in 0..16 {
                    let block_id = if y == 0 { 4 } else { 1 }; // sand bottom else air
                    conn.execute(
                        "INSERT INTO chunk_blocks (chunk_id, x, y, z, block_type_id) VALUES (1, ?1, ?2, ?3, ?4)",
                        (x as i32, y as i32, z as i32, block_id),
                    )?;
                }
            }
        }
        // Add some coral and seaweed columns
        for x in (0..16).step_by(4) {
            for z in (0..16).step_by(4) {
                for y in 1..4 {
                    let id = if y % 2 == 0 { 5 } else { 6 }; // coral/seaweed stack
                    conn.execute(
                        "UPDATE chunk_blocks SET block_type_id=?4 WHERE chunk_id=1 AND x=?1 AND y=?2 AND z=?3",
                        (x as i32, y as i32, z as i32, id),
                    )?;
                }
            }
        }
    }

    Ok(())
}
