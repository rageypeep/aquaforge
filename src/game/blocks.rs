//! Voxel block definitions for the underwater world.

use bevy::prelude::*;

/// A single cube in the voxel grid.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(dead_code)]
pub enum BlockType {
    /// Empty space. Below the water surface this is "flooded" and is
    /// not rendered; the water effect comes from fog + the surface plane.
    Air,
    /// Compacted seafloor rock.
    Stone,
    /// Sandy seabed, the topmost layer in most places.
    Sand,
    /// Darker sediment found between sand and stone.
    Dirt,
    /// Lively coral outcrop: renders like a bright, bumpy block.
    Coral,
    /// Kelp: a green, slightly translucent column that marks a plant.
    Kelp,
}

impl BlockType {
    /// Whether the block occupies the full cube and hides neighbour faces.
    #[inline]
    pub fn is_opaque(self) -> bool {
        !matches!(self, BlockType::Air | BlockType::Kelp)
    }

    /// Whether the block is empty (renders no geometry of its own).
    #[inline]
    pub fn is_air(self) -> bool {
        matches!(self, BlockType::Air)
    }

    /// Vertex colour tint applied per-face.
    pub fn color(self) -> LinearRgba {
        match self {
            BlockType::Air => LinearRgba::new(0.0, 0.0, 0.0, 0.0),
            BlockType::Stone => LinearRgba::new(0.32, 0.34, 0.38, 1.0),
            BlockType::Sand => LinearRgba::new(0.78, 0.72, 0.48, 1.0),
            BlockType::Dirt => LinearRgba::new(0.38, 0.28, 0.18, 1.0),
            BlockType::Coral => LinearRgba::new(0.95, 0.45, 0.55, 1.0),
            BlockType::Kelp => LinearRgba::new(0.15, 0.55, 0.25, 1.0),
        }
    }
}
