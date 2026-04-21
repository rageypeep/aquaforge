//! Player-driven block editing: raycast, break, and place.
//!
//! Runs a voxel DDA raycast from the camera each frame to find the targeted
//! block, draws a wireframe highlight on it, and responds to left/right
//! mouse clicks to break or place blocks. Number keys 1-5 cycle the
//! currently held block type.

use bevy::color::palettes::css::YELLOW;
use bevy::input::ButtonInput;
use bevy::input::keyboard::KeyCode;
use bevy::input::mouse::MouseButton;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

use super::blocks::BlockType;
use super::chunk::Chunk;
use super::chunk_map::{ChunkMap, world_block_to_chunk};

/// Registers block-editing resources and systems.
pub struct ChunkEditPlugin;

impl Plugin for ChunkEditPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SelectedBlock>()
            .init_resource::<TargetedBlock>()
            .add_systems(Startup, spawn_hotbar)
            .add_systems(
                Update,
                (
                    raycast_target,
                    draw_target_highlight.after(raycast_target),
                    cycle_selected_block,
                    update_hotbar_label,
                    edit_blocks.after(raycast_target),
                ),
            );
    }
}

/// The block type the player will place on right-click.
#[derive(Resource)]
pub struct SelectedBlock(pub BlockType);

impl Default for SelectedBlock {
    fn default() -> Self {
        Self(BlockType::Stone)
    }
}

/// The block currently under the crosshair, if any.
#[derive(Resource, Default)]
pub struct TargetedBlock(pub Option<Target>);

/// The result of a successful voxel raycast.
#[derive(Clone, Copy)]
pub struct Target {
    /// World-space coordinate of the hit block.
    pub world_block: IVec3,
    /// Outward-pointing face normal of the face the ray entered through.
    ///
    /// Adding this to `world_block` gives the cell a placed block would
    /// occupy.
    pub face_normal: IVec3,
}

/// Maximum reach of the block-picking ray, in world units.
const MAX_REACH: f32 = 6.0;

fn raycast_target(
    cameras: Query<&GlobalTransform, With<Camera3d>>,
    chunk_map: Res<ChunkMap>,
    chunks: Query<&Chunk>,
    mut targeted: ResMut<TargetedBlock>,
) {
    let Ok(cam) = cameras.single() else {
        targeted.0 = None;
        return;
    };

    let origin = cam.translation();
    let dir = cam.forward().as_vec3();

    targeted.0 = voxel_raycast(origin, dir, MAX_REACH, &chunk_map, &chunks);
}

/// 3D DDA through the voxel grid. Returns the first non-air block hit within
/// `max_distance` and the normal of the face the ray entered through.
fn voxel_raycast(
    origin: Vec3,
    dir: Vec3,
    max_distance: f32,
    chunk_map: &ChunkMap,
    chunks: &Query<&Chunk>,
) -> Option<Target> {
    if dir.length_squared() < 1e-6 {
        return None;
    }
    let dir = dir.normalize();

    let mut voxel = IVec3::new(
        origin.x.floor() as i32,
        origin.y.floor() as i32,
        origin.z.floor() as i32,
    );

    // If the camera is inside a solid block already, break immediately.
    if let Some(block) = lookup_block(voxel, chunk_map, chunks)
        && !block.is_air()
    {
        return Some(Target {
            world_block: voxel,
            face_normal: IVec3::ZERO,
        });
    }

    let step = IVec3::new(
        dir.x.signum() as i32,
        dir.y.signum() as i32,
        dir.z.signum() as i32,
    );

    let axis_boundary = |origin: f32, dir: f32, voxel: i32, step: i32| -> f32 {
        if dir.abs() < 1e-6 {
            return f32::INFINITY;
        }
        let next = if step > 0 { voxel + 1 } else { voxel } as f32;
        (next - origin) / dir
    };

    let mut t_max = Vec3::new(
        axis_boundary(origin.x, dir.x, voxel.x, step.x),
        axis_boundary(origin.y, dir.y, voxel.y, step.y),
        axis_boundary(origin.z, dir.z, voxel.z, step.z),
    );

    let inv = |d: f32| {
        if d.abs() < 1e-6 {
            f32::INFINITY
        } else {
            1.0 / d.abs()
        }
    };
    let t_delta = Vec3::new(inv(dir.x), inv(dir.y), inv(dir.z));

    #[allow(unused_assignments)]
    let mut last_normal = IVec3::ZERO;
    let mut t = 0.0_f32;

    while t <= max_distance {
        if t_max.x < t_max.y && t_max.x < t_max.z {
            voxel.x += step.x;
            t = t_max.x;
            t_max.x += t_delta.x;
            last_normal = IVec3::new(-step.x, 0, 0);
        } else if t_max.y < t_max.z {
            voxel.y += step.y;
            t = t_max.y;
            t_max.y += t_delta.y;
            last_normal = IVec3::new(0, -step.y, 0);
        } else {
            voxel.z += step.z;
            t = t_max.z;
            t_max.z += t_delta.z;
            last_normal = IVec3::new(0, 0, -step.z);
        }

        if t > max_distance {
            break;
        }

        if let Some(block) = lookup_block(voxel, chunk_map, chunks)
            && !block.is_air()
        {
            return Some(Target {
                world_block: voxel,
                face_normal: last_normal,
            });
        }
    }

    None
}

fn lookup_block(
    world_block: IVec3,
    chunk_map: &ChunkMap,
    chunks: &Query<&Chunk>,
) -> Option<BlockType> {
    let (chunk_pos, local) = world_block_to_chunk(world_block);
    let entity = chunk_map.get(chunk_pos)?;
    let chunk = chunks.get(entity).ok()?;
    Some(chunk.get(local.x as usize, local.y as usize, local.z as usize))
}

fn draw_target_highlight(targeted: Res<TargetedBlock>, mut gizmos: Gizmos) {
    let Some(target) = targeted.0 else {
        return;
    };
    let min = target.world_block.as_vec3();
    let center = min + Vec3::splat(0.5);
    gizmos.cube(
        Transform::from_translation(center).with_scale(Vec3::splat(1.02)),
        YELLOW,
    );
}

const HOTBAR: [(KeyCode, BlockType); 5] = [
    (KeyCode::Digit1, BlockType::Stone),
    (KeyCode::Digit2, BlockType::Sand),
    (KeyCode::Digit3, BlockType::Dirt),
    (KeyCode::Digit4, BlockType::Coral),
    (KeyCode::Digit5, BlockType::Kelp),
];

fn cycle_selected_block(keys: Res<ButtonInput<KeyCode>>, mut selected: ResMut<SelectedBlock>) {
    for (key, block) in HOTBAR {
        if keys.just_pressed(key) {
            selected.0 = block;
        }
    }
}

fn edit_blocks(
    mouse: Res<ButtonInput<MouseButton>>,
    cursors: Query<&CursorOptions, With<PrimaryWindow>>,
    mut was_locked: Local<bool>,
    targeted: Res<TargetedBlock>,
    selected: Res<SelectedBlock>,
    chunk_map: Res<ChunkMap>,
    mut chunks: Query<(&mut Chunk, &mut Mesh3d)>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let locked = cursors
        .single()
        .map(|c| c.grab_mode == CursorGrabMode::Locked)
        .unwrap_or(false);

    // Only edit when the cursor was already locked on the previous frame.
    // This prevents the "grab" click from also breaking a block.
    let should_act = locked && *was_locked;
    *was_locked = locked;
    if !should_act {
        return;
    }

    let Some(target) = targeted.0 else {
        return;
    };

    if mouse.just_pressed(MouseButton::Left) {
        set_block(
            target.world_block,
            BlockType::Air,
            &chunk_map,
            &mut chunks,
            &mut meshes,
        );
    } else if mouse.just_pressed(MouseButton::Right) {
        let place_pos = target.world_block + target.face_normal;
        // Don't place inside the existing target cell.
        if target.face_normal != IVec3::ZERO {
            set_block(place_pos, selected.0, &chunk_map, &mut chunks, &mut meshes);
        }
    }
}

fn set_block(
    world_block: IVec3,
    block: BlockType,
    chunk_map: &ChunkMap,
    chunks: &mut Query<(&mut Chunk, &mut Mesh3d)>,
    meshes: &mut ResMut<Assets<Mesh>>,
) {
    let (chunk_pos, local) = world_block_to_chunk(world_block);
    let Some(entity) = chunk_map.get(chunk_pos) else {
        return;
    };
    let Ok((mut chunk, mut mesh)) = chunks.get_mut(entity) else {
        return;
    };
    chunk.set(local.x as usize, local.y as usize, local.z as usize, block);
    let new_mesh = chunk.build_mesh();
    mesh.0 = meshes.add(new_mesh);
}

/// Marker for the hotbar text node so `update_hotbar_label` can find it.
#[derive(Component)]
struct HotbarLabel;

fn spawn_hotbar(mut commands: Commands) {
    commands.spawn((
        Text::new("Hold: Stone   (1 Stone  2 Sand  3 Dirt  4 Coral  5 Kelp)"),
        TextFont {
            font_size: 16.0,
            ..default()
        },
        TextColor(Color::srgba(0.9, 0.97, 1.0, 0.9)),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(12.0),
            left: Val::Px(12.0),
            ..default()
        },
        HotbarLabel,
        Name::new("Hotbar Label"),
    ));

    commands.spawn((
        Text::new("+"),
        TextFont {
            font_size: 28.0,
            ..default()
        },
        TextColor(Color::srgba(1.0, 1.0, 1.0, 0.7)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Percent(50.0),
            left: Val::Percent(50.0),
            margin: UiRect {
                top: Val::Px(-14.0),
                left: Val::Px(-7.0),
                ..default()
            },
            ..default()
        },
        Name::new("Crosshair"),
    ));
}

fn update_hotbar_label(
    selected: Res<SelectedBlock>,
    mut labels: Query<&mut Text, With<HotbarLabel>>,
) {
    if !selected.is_changed() {
        return;
    }
    for mut text in &mut labels {
        text.0 = format!(
            "Hold: {:?}   (1 Stone  2 Sand  3 Dirt  4 Coral  5 Kelp)",
            selected.0
        );
    }
}
