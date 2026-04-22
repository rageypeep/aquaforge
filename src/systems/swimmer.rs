//! First-person swimmer controller with swept-AABB collision and oxygen.
//!
//! Replaces the old noclip fly-cam with an in-world player: the camera
//! entity gets a compact AABB body, input drives a velocity that is
//! damped like a swimmer in water (no instantaneous stops), and the
//! new position is resolved axis-by-axis against the voxel grid so the
//! player can't tunnel into solid terrain. While the body is under
//! `WATER_LEVEL` an oxygen meter drains at 1.0/s and refills at 5.0/s
//! once the head breaks the surface. No drowning damage yet — the
//! meter is here to make the "come up for air" loop feel real once a
//! health system lands.
//!
//! Mouse-look reads `AccumulatedMouseMotion` only when the cursor is
//! grabbed (matching `edit.rs`), so the crosshair-raycast and block
//! break/place systems keep working unchanged.
//!
//! All pure logic (AABB sweeping, oxygen bookkeeping, wish-vector
//! composition) lives in free functions with unit tests below; the
//! Bevy systems are thin glue.
//!
//! See `AGENTS.md` for the repo's plugin-composition convention.

use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

use crate::game::chunk::Chunk;
use crate::game::chunk_map::{ChunkMap, world_block_to_chunk};
use crate::game::world::WATER_LEVEL;

/// Plugin that installs the swimmer controller and oxygen tick.
pub struct SwimmerPlugin;

impl Plugin for SwimmerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                attach_swimmer,
                apply_swimmer_look,
                apply_swimmer_motion,
                tick_oxygen,
            ),
        );
    }
}

/// Per-camera swimmer state. Carries rotation so mouse deltas integrate
/// cleanly, velocity so damping/physics can see last frame's state, and
/// the AABB half-extents used for collision.
#[derive(Component)]
pub struct Swimmer {
    pub yaw: f32,
    pub pitch: f32,
    /// World-space velocity, in units / second.
    pub velocity: Vec3,
    /// Half-extents of the body AABB around the camera's position.
    pub aabb_half: Vec3,
    /// Top input speed, in units / second. Ctrl multiplies by `sprint`.
    pub speed: f32,
    pub sprint: f32,
    /// Radians of rotation per pixel of mouse motion.
    pub sensitivity: f32,
}

impl Default for Swimmer {
    fn default() -> Self {
        Self {
            yaw: 0.0,
            pitch: 0.0,
            velocity: Vec3::ZERO,
            // A compact ~0.7³ body so the camera stays centred inside
            // its collider. We're modelling a floating swimmer, not a
            // standing character, so there's no dedicated "eye height"
            // offset from the body centre.
            aabb_half: Vec3::splat(0.35),
            speed: 6.0,
            sprint: 2.0,
            sensitivity: 0.0025,
        }
    }
}

/// Oxygen meter. Depletes while the swimmer's body centre is below the
/// water surface and refills once the head breaches it.
#[derive(Component, Debug, Clone, Copy)]
pub struct Oxygen {
    pub current: f32,
    pub max: f32,
    pub depletion_per_sec: f32,
    pub regen_per_sec: f32,
}

impl Default for Oxygen {
    fn default() -> Self {
        Self {
            current: 30.0,
            max: 30.0,
            depletion_per_sec: 1.0,
            regen_per_sec: 5.0,
        }
    }
}

/// Water drag time constant: velocity decays toward the input-driven
/// target with this τ when the body is submerged (1/e per 0.3 s).
const WATER_TAU: f32 = 0.3;
/// Out-of-water (light-air) drag time constant. Weaker so the player
/// can skim along the surface without feeling stuck.
const AIR_TAU: f32 = 1.2;
/// Gravity applied only when the body centre is above the water line.
const AIR_GRAVITY: f32 = 18.0;
/// Epsilon padding used when snapping to a block face to avoid getting
/// re-flagged as colliding on the next frame.
const COLLISION_EPSILON: f32 = 1e-3;

/// Attach a [`Swimmer`] + [`Oxygen`] to every `Camera3d` that doesn't
/// have them yet. Mirrors the old fly-cam's attach pass.
fn attach_swimmer(
    mut commands: Commands,
    cameras: Query<(Entity, &Transform), (With<Camera3d>, Without<Swimmer>)>,
) {
    for (entity, transform) in &cameras {
        let (yaw, pitch, _) = transform.rotation.to_euler(EulerRot::YXZ);
        commands.entity(entity).insert((
            Swimmer {
                yaw,
                pitch,
                ..default()
            },
            Oxygen::default(),
        ));
    }
}

fn apply_swimmer_look(
    motion: Res<AccumulatedMouseMotion>,
    windows: Query<&CursorOptions, With<PrimaryWindow>>,
    mut swimmers: Query<(&mut Transform, &mut Swimmer)>,
) {
    let Ok(cursor) = windows.single() else {
        return;
    };
    if cursor.grab_mode != CursorGrabMode::Locked {
        return;
    }

    let delta = motion.delta;
    if delta == Vec2::ZERO {
        return;
    }

    for (mut transform, mut swimmer) in &mut swimmers {
        swimmer.yaw -= delta.x * swimmer.sensitivity;
        swimmer.pitch -= delta.y * swimmer.sensitivity;
        swimmer.pitch = swimmer.pitch.clamp(
            -std::f32::consts::FRAC_PI_2 + 0.01,
            std::f32::consts::FRAC_PI_2 - 0.01,
        );

        transform.rotation = Quat::from_axis_angle(Vec3::Y, swimmer.yaw)
            * Quat::from_axis_angle(Vec3::X, swimmer.pitch);
    }
}

fn apply_swimmer_motion(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    chunk_map: Res<ChunkMap>,
    chunks: Query<&Chunk>,
    mut swimmers: Query<(&mut Transform, &mut Swimmer)>,
) {
    let dt = time.delta_secs().min(0.05);
    if dt <= 0.0 {
        return;
    }

    for (mut transform, mut swimmer) in &mut swimmers {
        let wish = wish_direction(&keys, swimmer.yaw);
        let boost = if keys.pressed(KeyCode::ControlLeft) {
            swimmer.sprint
        } else {
            1.0
        };
        let target = wish * swimmer.speed * boost;

        let submerged = is_submerged(transform.translation, swimmer.aabb_half);
        let tau = if submerged { WATER_TAU } else { AIR_TAU };
        let alpha = 1.0 - (-dt / tau).exp();
        let current_v = swimmer.velocity;
        swimmer.velocity += (target - current_v) * alpha;

        if !submerged {
            // Above water we sink under gravity; the damping term above
            // still fights the keyboard wish-vector, so holding Space
            // lets the player climb back out.
            swimmer.velocity.y -= AIR_GRAVITY * dt;
        }

        let resolved = resolve_collisions(
            transform.translation,
            swimmer.velocity * dt,
            swimmer.aabb_half,
            |p| lookup_block_solid(p, &chunk_map, &chunks),
        );
        transform.translation = resolved.position;
        // Kill velocity on axes that bumped into geometry, so the player
        // doesn't "stick" to walls while input keeps pushing.
        if resolved.hit.x {
            swimmer.velocity.x = 0.0;
        }
        if resolved.hit.y {
            swimmer.velocity.y = 0.0;
        }
        if resolved.hit.z {
            swimmer.velocity.z = 0.0;
        }
    }
}

fn tick_oxygen(time: Res<Time>, mut swimmers: Query<(&Transform, &mut Oxygen), With<Swimmer>>) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    for (transform, mut oxygen) in &mut swimmers {
        step_oxygen(&mut oxygen, dt, transform.translation.y);
    }
}

/// Keyboard + yaw → desired velocity direction. Horizontal input is
/// normalised so diagonal sprinting isn't √2 faster; vertical input is
/// added after normalisation so Space / Shift stack with WASD cleanly.
pub fn wish_direction(keys: &ButtonInput<KeyCode>, yaw: f32) -> Vec3 {
    let mut horizontal = Vec2::ZERO;
    if keys.pressed(KeyCode::KeyW) {
        horizontal.y -= 1.0;
    }
    if keys.pressed(KeyCode::KeyS) {
        horizontal.y += 1.0;
    }
    if keys.pressed(KeyCode::KeyA) {
        horizontal.x -= 1.0;
    }
    if keys.pressed(KeyCode::KeyD) {
        horizontal.x += 1.0;
    }
    if horizontal != Vec2::ZERO {
        horizontal = horizontal.normalize();
    }

    let yaw_rot = Quat::from_axis_angle(Vec3::Y, yaw);
    let planar = yaw_rot * Vec3::new(horizontal.x, 0.0, horizontal.y);

    let mut vertical = 0.0_f32;
    if keys.pressed(KeyCode::Space) {
        vertical += 1.0;
    }
    if keys.pressed(KeyCode::ShiftLeft) {
        vertical -= 1.0;
    }

    planar + Vec3::Y * vertical
}

/// Per-axis collision report returned by [`resolve_collisions`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HitAxes {
    pub x: bool,
    pub y: bool,
    pub z: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CollisionResult {
    pub position: Vec3,
    pub hit: HitAxes,
}

/// Sweep `position` by `delta` against the voxel grid described by the
/// `solid` closure. Axes are resolved sequentially (X, Z, Y) so the
/// swimmer can slide along walls without snagging on block edges.
pub fn resolve_collisions(
    position: Vec3,
    delta: Vec3,
    half: Vec3,
    mut solid: impl FnMut(IVec3) -> bool,
) -> CollisionResult {
    let mut p = position;
    let mut hit = HitAxes::default();

    for axis in [0_usize, 2, 1] {
        let step = delta[axis];
        if step == 0.0 {
            continue;
        }
        let mut candidate = p;
        candidate[axis] += step;

        if !aabb_hits_solid(candidate, half, &mut solid) {
            p = candidate;
            continue;
        }

        // Snap so the leading face of the AABB lands just inside the
        // nearest voxel boundary crossed by this step. We trust that
        // `step` is under one block per tick (caller clamps `dt`) so
        // the first integer boundary past our pre-move leading face is
        // the face of the block we just collided with.
        let pre_leading = p[axis] + half[axis].copysign(step);
        let snapped_face = if step > 0.0 {
            (pre_leading + COLLISION_EPSILON).ceil()
        } else {
            (pre_leading - COLLISION_EPSILON).floor()
        };
        let snapped_center =
            snapped_face - half[axis].copysign(step) - COLLISION_EPSILON.copysign(step);

        p[axis] = snapped_center;
        match axis {
            0 => hit.x = true,
            1 => hit.y = true,
            _ => hit.z = true,
        }
    }

    CollisionResult { position: p, hit }
}

fn aabb_hits_solid(center: Vec3, half: Vec3, mut solid: impl FnMut(IVec3) -> bool) -> bool {
    let min = center - half;
    let max = center + half;
    let bx0 = min.x.floor() as i32;
    let by0 = min.y.floor() as i32;
    let bz0 = min.z.floor() as i32;
    // `max - EPSILON` so touching a face (open interval) doesn't count.
    let bx1 = (max.x - COLLISION_EPSILON).floor() as i32;
    let by1 = (max.y - COLLISION_EPSILON).floor() as i32;
    let bz1 = (max.z - COLLISION_EPSILON).floor() as i32;

    for bx in bx0..=bx1 {
        for by in by0..=by1 {
            for bz in bz0..=bz1 {
                if solid(IVec3::new(bx, by, bz)) {
                    return true;
                }
            }
        }
    }
    false
}

/// Look up a single voxel and return whether it's collidable. Unloaded
/// chunks (past the streaming horizon) are treated as empty so the
/// player doesn't crash into an invisible wall at the load boundary.
fn lookup_block_solid(block: IVec3, chunk_map: &ChunkMap, chunks: &Query<&Chunk>) -> bool {
    let (chunk_pos, local) = world_block_to_chunk(block);
    let Some(entity) = chunk_map.get(chunk_pos) else {
        return false;
    };
    let Ok(chunk) = chunks.get(entity) else {
        return false;
    };
    let block = chunk.get(local.x as usize, local.y as usize, local.z as usize);
    block.is_opaque()
}

/// True when the swimmer's body centre is submerged. Using the centre
/// (not just "head below water") makes the air/water transition kick in
/// once the player is more-than-half under, which reads as natural.
pub fn is_submerged(pos: Vec3, _half: Vec3) -> bool {
    pos.y < WATER_LEVEL
}

/// Pure oxygen update: decays underwater, regens above, clamped to
/// `[0, max]`. Factored out so tests can drive it without a Bevy world.
pub fn step_oxygen(oxygen: &mut Oxygen, dt: f32, head_y: f32) {
    if head_y < WATER_LEVEL {
        oxygen.current = (oxygen.current - oxygen.depletion_per_sec * dt).max(0.0);
    } else {
        oxygen.current = (oxygen.current + oxygen.regen_per_sec * dt).min(oxygen.max);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn no_blocks(_: IVec3) -> bool {
        false
    }

    fn solid_floor_at_y(y: i32) -> impl FnMut(IVec3) -> bool {
        move |p: IVec3| p.y == y
    }

    #[test]
    fn open_space_moves_the_full_delta() {
        let result = resolve_collisions(
            Vec3::new(0.0, 10.0, 0.0),
            Vec3::new(0.5, 0.2, -0.3),
            Vec3::splat(0.35),
            no_blocks,
        );
        assert_eq!(result.hit, HitAxes::default());
        assert!((result.position - Vec3::new(0.5, 10.2, -0.3)).length() < 1e-5);
    }

    #[test]
    fn downward_motion_stops_on_a_floor() {
        // Floor at y = 5 (voxel occupies [5, 6]). Player centre starts
        // at y = 7 with half-extent 0.35, so the AABB bottom is at 6.65.
        // A full -1.0 step would put the bottom at 5.65, inside the
        // block — collision should snap the bottom to y = 6.
        let result = resolve_collisions(
            Vec3::new(0.0, 7.0, 0.0),
            Vec3::new(0.0, -1.0, 0.0),
            Vec3::splat(0.35),
            solid_floor_at_y(5),
        );
        assert!(result.hit.y);
        // Bottom of the AABB should be on/just above y = 6 (top of the
        // solid voxel).
        let bottom = result.position.y - 0.35;
        assert!(
            bottom >= 6.0 - 1e-4 && bottom < 6.0 + 1e-2,
            "bottom={bottom}"
        );
    }

    #[test]
    fn horizontal_motion_slides_along_a_wall() {
        // Single wall voxel at (2, 5, 0). Player at (1.5, 5.0, 0.0)
        // trying to move +X into it while also moving +Z.
        let wall = IVec3::new(2, 5, 0);
        let result = resolve_collisions(
            Vec3::new(1.5, 5.0, 0.0),
            Vec3::new(1.0, 0.0, 0.5),
            Vec3::splat(0.35),
            move |p| p == wall,
        );
        assert!(result.hit.x, "expected X-axis collision");
        assert!(!result.hit.z, "Z should slide freely");
        // Z should have advanced the full 0.5.
        assert!((result.position.z - 0.5).abs() < 1e-4);
        // X should have stopped with the right face of the AABB at x = 2.
        let right_face = result.position.x + 0.35;
        assert!(right_face <= 2.0 + 1e-3);
    }

    #[test]
    fn upward_motion_stops_on_a_ceiling() {
        // Ceiling at y = 10.
        let result = resolve_collisions(
            Vec3::new(0.0, 9.0, 0.0),
            Vec3::new(0.0, 2.0, 0.0),
            Vec3::splat(0.35),
            solid_floor_at_y(10),
        );
        assert!(result.hit.y);
        let top = result.position.y + 0.35;
        assert!(top <= 10.0 + 1e-3);
    }

    #[test]
    fn oxygen_depletes_underwater() {
        let mut o = Oxygen {
            current: 30.0,
            max: 30.0,
            depletion_per_sec: 1.0,
            regen_per_sec: 5.0,
        };
        // WATER_LEVEL - 1 is safely below the surface.
        step_oxygen(&mut o, 5.0, WATER_LEVEL - 1.0);
        assert!((o.current - 25.0).abs() < 1e-5, "current={}", o.current);
    }

    #[test]
    fn oxygen_regenerates_above_water() {
        let mut o = Oxygen {
            current: 10.0,
            max: 30.0,
            depletion_per_sec: 1.0,
            regen_per_sec: 5.0,
        };
        step_oxygen(&mut o, 2.0, WATER_LEVEL + 0.5);
        assert!((o.current - 20.0).abs() < 1e-5, "current={}", o.current);
    }

    #[test]
    fn oxygen_clamps_to_bounds() {
        let mut o = Oxygen {
            current: 0.5,
            max: 30.0,
            depletion_per_sec: 1.0,
            regen_per_sec: 5.0,
        };
        // 5 seconds underwater with only 0.5s of oxygen → clamps to 0.
        step_oxygen(&mut o, 5.0, WATER_LEVEL - 1.0);
        assert_eq!(o.current, 0.0);

        o.current = 29.9;
        // 10 seconds above water → clamps to max.
        step_oxygen(&mut o, 10.0, WATER_LEVEL + 1.0);
        assert_eq!(o.current, o.max);
    }
}
