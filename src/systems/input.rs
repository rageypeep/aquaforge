use bevy::prelude::*;
use bevy::window::{CursorGrabMode, PrimaryWindow};
use bevy::input::mouse::MouseMotion;

pub struct ControlsPlugin;

impl Plugin for ControlsPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_systems(Startup, attach_mouse_look)
            .add_systems(Update, (attach_mouse_look, camera_movement, mouse_look, esc_to_unlock_mouse));
    }
}
// Camera movement (WASD + up/down)
fn camera_movement(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut query: Query<&mut Transform, With<Camera3d>>,
    time: Res<Time>,
) {
    let speed = 8.0;
    let dt = time.delta_seconds();

    for mut transform in query.iter_mut() {
        let mut direction = Vec3::ZERO;
        if keyboard_input.pressed(KeyCode::KeyW) { direction.z -= 1.0; }
        if keyboard_input.pressed(KeyCode::KeyS) { direction.z += 1.0; }
        if keyboard_input.pressed(KeyCode::KeyA) { direction.x -= 1.0; }
        if keyboard_input.pressed(KeyCode::KeyD) { direction.x += 1.0; }
        if keyboard_input.pressed(KeyCode::Space) { direction.y += 1.0; }
        if keyboard_input.pressed(KeyCode::ShiftLeft) { direction.y -= 1.0; }

        if direction != Vec3::ZERO {
            direction = direction.normalize();
            let rotation = transform.rotation;
            transform.translation += rotation * direction * speed * dt;
        }
    }
}

// Mouse look state component
#[derive(Component)]
pub struct MouseLookState {
    pub yaw: f32,
    pub pitch: f32,
}

impl Default for MouseLookState {
    fn default() -> Self {
        Self { yaw: 0.0, pitch: 0.0 }
    }
}

// Attach MouseLookState to the camera on spawn
pub fn attach_mouse_look(
    mut commands: Commands,
    query: Query<Entity, (With<Camera3d>, Without<MouseLookState>)>,
    transforms: Query<&Transform, With<Camera3d>>,
) {
    for entity in query.iter() {
        println!("Found camera entity {:?}, attaching MouseLookState", entity);
        let transform = transforms.get(entity).unwrap();
        // Yaw: rotation around Y axis, Pitch: rotation around X axis
        let (yaw, pitch, _roll) = transform.rotation.to_euler(EulerRot::YXZ);
        commands.entity(entity).insert(MouseLookState { yaw, pitch });
        println!("Attached MouseLookState with yaw: {}, pitch: {}", yaw, pitch);
    }
}

// Mouse look system
fn mouse_look(
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    mut motion_evr: EventReader<MouseMotion>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mut query: Query<(&mut Transform, &mut MouseLookState), With<Camera3d>>,
) {
    let mut window = windows.single_mut();

    // Re-capture mouse if left-clicked in window and not already grabbed
    if mouse_buttons.just_pressed(MouseButton::Left)
        && window.cursor.grab_mode != CursorGrabMode::Locked
    {
        window.cursor.grab_mode = CursorGrabMode::Locked;
        window.cursor.visible = false;
        println!("Mouse captured!");
    }

    if window.cursor.grab_mode == CursorGrabMode::Locked {
        let sensitivity = 0.12;
        let mut delta = Vec2::ZERO;

        for ev in motion_evr.read() {
            delta += ev.delta;
        }

        if delta != Vec2::ZERO {
            println!("Mouse delta: {:?}", delta);
        }

        for (mut transform, mut look) in query.iter_mut() {
            if delta != Vec2::ZERO {
                println!("Applying mouse look to camera");
                look.yaw   -= delta.x * sensitivity * 0.01;
                look.pitch -= delta.y * sensitivity * 0.01;
                look.pitch = look.pitch.clamp(-1.54, 1.54); // Up/down clamp

                transform.rotation = Quat::from_axis_angle(Vec3::Y, look.yaw)
                    * Quat::from_axis_angle(Vec3::X, look.pitch);
                println!("New yaw: {}, pitch: {}", look.yaw, look.pitch);
            }
        }
    }
}

// Unlock mouse with Esc
fn esc_to_unlock_mouse(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    let mut window = windows.single_mut();
    if keyboard_input.just_pressed(KeyCode::Escape) {
        window.cursor.grab_mode = CursorGrabMode::None;
        window.cursor.visible = true;
    }
}
