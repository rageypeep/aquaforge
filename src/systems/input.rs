//! Fly-cam controls: WASD + Space/Shift to move, mouse to look, Esc to release.
//!
//! This is intentionally minimal — we'll grow it into a full first-person
//! swimmer once the voxel base is in place.

use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

/// Input + camera controls plugin.
pub struct ControlsPlugin;

impl Plugin for ControlsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                attach_fly_cam,
                grab_cursor_on_click,
                release_cursor_on_escape,
                apply_mouse_look,
                apply_keyboard_motion,
            ),
        );
    }
}

/// Per-camera fly-cam state.
#[derive(Component)]
pub struct FlyCam {
    pub yaw: f32,
    pub pitch: f32,
    /// Movement speed in world-units per second.
    pub speed: f32,
    /// Radians of rotation per pixel of mouse motion.
    pub sensitivity: f32,
}

impl Default for FlyCam {
    fn default() -> Self {
        Self {
            yaw: 0.0,
            pitch: 0.0,
            speed: 12.0,
            sensitivity: 0.0025,
        }
    }
}

/// Add a [`FlyCam`] component to any [`Camera3d`] that doesn't have one yet.
fn attach_fly_cam(
    mut commands: Commands,
    cameras: Query<(Entity, &Transform), (With<Camera3d>, Without<FlyCam>)>,
) {
    for (entity, transform) in &cameras {
        let (yaw, pitch, _) = transform.rotation.to_euler(EulerRot::YXZ);
        commands.entity(entity).insert(FlyCam {
            yaw,
            pitch,
            ..default()
        });
    }
}

fn grab_cursor_on_click(
    mouse: Res<ButtonInput<MouseButton>>,
    mut windows: Query<&mut CursorOptions, With<PrimaryWindow>>,
) {
    let Ok(mut cursor) = windows.single_mut() else {
        return;
    };

    if mouse.just_pressed(MouseButton::Left) && cursor.grab_mode != CursorGrabMode::Locked {
        cursor.grab_mode = CursorGrabMode::Locked;
        cursor.visible = false;
    }
}

fn release_cursor_on_escape(
    keys: Res<ButtonInput<KeyCode>>,
    mut windows: Query<&mut CursorOptions, With<PrimaryWindow>>,
) {
    let Ok(mut cursor) = windows.single_mut() else {
        return;
    };

    if keys.just_pressed(KeyCode::Escape) {
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
    }
}

fn apply_mouse_look(
    motion: Res<AccumulatedMouseMotion>,
    windows: Query<&CursorOptions, With<PrimaryWindow>>,
    mut cameras: Query<(&mut Transform, &mut FlyCam)>,
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

    for (mut transform, mut cam) in &mut cameras {
        cam.yaw -= delta.x * cam.sensitivity;
        cam.pitch -= delta.y * cam.sensitivity;
        cam.pitch = cam.pitch.clamp(
            -std::f32::consts::FRAC_PI_2 + 0.01,
            std::f32::consts::FRAC_PI_2 - 0.01,
        );

        transform.rotation = Quat::from_axis_angle(Vec3::Y, cam.yaw)
            * Quat::from_axis_angle(Vec3::X, cam.pitch);
    }
}

fn apply_keyboard_motion(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut cameras: Query<(&mut Transform, &FlyCam)>,
) {
    let dt = time.delta_secs();

    for (mut transform, cam) in &mut cameras {
        let forward = *transform.forward();
        let right = *transform.right();

        let mut motion = Vec3::ZERO;
        if keys.pressed(KeyCode::KeyW) {
            motion += forward;
        }
        if keys.pressed(KeyCode::KeyS) {
            motion -= forward;
        }
        if keys.pressed(KeyCode::KeyD) {
            motion += right;
        }
        if keys.pressed(KeyCode::KeyA) {
            motion -= right;
        }
        if keys.pressed(KeyCode::Space) {
            motion += Vec3::Y;
        }
        if keys.pressed(KeyCode::ShiftLeft) {
            motion -= Vec3::Y;
        }

        let boost = if keys.pressed(KeyCode::ControlLeft) {
            3.0
        } else {
            1.0
        };

        if motion != Vec3::ZERO {
            transform.translation += motion.normalize() * cam.speed * boost * dt;
        }
    }
}
