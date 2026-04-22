//! Submarine headlights: two forward-facing spot-lights parented to the
//! player camera, toggled on/off by the player.
//!
//! The player is a small sub, not a swimmer, so a pair of cone beams in front
//! of the camera sells the vehicle and doubles as practical illumination in
//! the deeper / more heavily fogged corners of the map.
//!
//! Parented to the camera means the beams follow look direction for free; we
//! only need to set the local offsets and a small outward splay at spawn.

use bevy::light::SpotLight;
use bevy::prelude::*;

/// Plugin that spawns the sub headlights and wires up the on/off toggle.
pub struct HeadlightsPlugin;

impl Plugin for HeadlightsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Headlights::default())
            // The camera is spawned in `AtmospherePlugin`'s `Startup`
            // systems. Bevy only flushes deferred `Commands` between
            // schedules, so querying for `Camera3d` in the same `Startup`
            // pass races the camera's spawn. Running in `PostStartup`
            // guarantees the camera entity is live before we parent the
            // headlights to it.
            .add_systems(PostStartup, attach_headlights)
            .add_systems(Startup, spawn_headlights_hud)
            .add_systems(
                Update,
                (
                    toggle_headlights,
                    apply_headlights_state,
                    update_headlights_hud,
                )
                    .chain(),
            );
    }
}

/// Global toggle state for the sub's headlights. `true` at startup so the
/// world reads as clearly lit on the very first frame.
#[derive(Resource)]
pub struct Headlights {
    pub on: bool,
}

impl Default for Headlights {
    fn default() -> Self {
        Self { on: true }
    }
}

/// Marker for the two headlight entities so `apply_headlights_state` can
/// find them again.
#[derive(Component)]
struct Headlight {
    /// Intensity to restore when toggled back on, in lumens.
    full_intensity: f32,
}

/// Lateral offset of each light from the camera centre, in metres.
const HEADLIGHT_SIDE_OFFSET: f32 = 0.8;

/// Forward offset — slightly in front of the camera so the cones start
/// outside the camera's near plane.
const HEADLIGHT_FORWARD_OFFSET: f32 = 0.6;

/// Outward yaw of each beam, in radians (~4°). Small enough that the cones
/// still overlap at the distance we typically care about (~5–15 m).
const HEADLIGHT_SPLAY: f32 = 0.07;

/// Peak intensity of each bulb when the headlights are on, in lumens.
///
/// Underwater fog is dense (exponential density 0.035), so we need a much
/// brighter source than a real car headlight (~1 000 lm) for the beam to be
/// readable more than a few metres out. 200 klm is roughly a lighthouse-class
/// source — plausible for a working sub.
const HEADLIGHT_INTENSITY: f32 = 200_000.0;

/// Beam geometry. `outer_angle` is the half-angle of the outer cone; the
/// inner cone is the no-falloff core.
const HEADLIGHT_OUTER_ANGLE: f32 = 0.40;
const HEADLIGHT_INNER_ANGLE: f32 = 0.28;

/// Range beyond which the spot-light is culled. 60 m comfortably covers the
/// visible fog depth without casting influence across the whole scene.
const HEADLIGHT_RANGE: f32 = 60.0;

/// When shadows are enabled the extra 2×shadow-map allocations hurt
/// performance on llvmpipe more than they buy visually — fog already
/// occludes the far ends of the cones. Leave them off for now.
const HEADLIGHT_SHADOWS: bool = false;

fn attach_headlights(mut commands: Commands, cameras: Query<Entity, With<Camera3d>>) {
    let Ok(camera) = cameras.single() else {
        return;
    };

    for (side, sign) in [("Port", -1.0f32), ("Starboard", 1.0f32)] {
        let transform =
            Transform::from_xyz(sign * HEADLIGHT_SIDE_OFFSET, 0.0, -HEADLIGHT_FORWARD_OFFSET)
                // `SpotLight` aims down its local `-Z`; camera forward is also `-Z`,
                // so the default rotation already aims along the camera's look
                // direction. Rotating by `+θ` around `+Y` tilts `-Z` toward `-X`
                // (left); we want the port light (sign=-1) to tilt further to `-X`
                // and the starboard light (sign=+1) to tilt to `+X`, which means
                // yaw = `-sign * splay`.
                .with_rotation(Quat::from_axis_angle(Vec3::Y, -sign * HEADLIGHT_SPLAY));

        commands.spawn((
            SpotLight {
                color: Color::srgb(0.95, 0.98, 1.0),
                intensity: HEADLIGHT_INTENSITY,
                range: HEADLIGHT_RANGE,
                outer_angle: HEADLIGHT_OUTER_ANGLE,
                inner_angle: HEADLIGHT_INNER_ANGLE,
                shadows_enabled: HEADLIGHT_SHADOWS,
                ..default()
            },
            transform,
            Headlight {
                full_intensity: HEADLIGHT_INTENSITY,
            },
            Name::new(format!("Sub Headlight ({side})")),
            ChildOf(camera),
        ));
    }
}

fn toggle_headlights(keys: Res<ButtonInput<KeyCode>>, mut headlights: ResMut<Headlights>) {
    if keys.just_pressed(KeyCode::KeyL) {
        headlights.on = !headlights.on;
    }
}

fn apply_headlights_state(
    headlights: Res<Headlights>,
    mut lights: Query<(&mut SpotLight, &Headlight)>,
) {
    if !headlights.is_changed() {
        return;
    }
    for (mut spot, meta) in &mut lights {
        spot.intensity = if headlights.on {
            meta.full_intensity
        } else {
            0.0
        };
    }
}

/// Marker for the on-screen "L: Lights …" label so `update_headlights_hud`
/// can rewrite it when the toggle flips.
#[derive(Component)]
struct HeadlightsLabel;

fn spawn_headlights_hud(mut commands: Commands, headlights: Res<Headlights>) {
    commands.spawn((
        Text::new(headlights_label_text(headlights.on)),
        TextFont {
            font_size: 16.0,
            ..default()
        },
        TextColor(Color::srgba(0.9, 0.97, 1.0, 0.9)),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(36.0),
            left: Val::Px(12.0),
            ..default()
        },
        HeadlightsLabel,
        Name::new("Headlights Label"),
    ));
}

fn update_headlights_hud(
    headlights: Res<Headlights>,
    mut labels: Query<&mut Text, With<HeadlightsLabel>>,
) {
    if !headlights.is_changed() {
        return;
    }
    for mut text in &mut labels {
        text.0 = headlights_label_text(headlights.on);
    }
}

fn headlights_label_text(on: bool) -> String {
    let state = if on { "ON" } else { "OFF" };
    format!("L: Headlights {state}")
}
