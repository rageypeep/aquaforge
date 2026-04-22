//! Minimal HUD: an O2-reserve meter anchored to the bottom-centre of
//! the screen. Re-reads the current sub's [`Oxygen`] component each
//! frame and resizes a coloured bar. Stays hidden while the reserve
//! is full so the viewport is clean when the sub is surfaced.
//!
//! The HUD is intentionally dependency-free — just two `Node` entities
//! and a [`Text`]. Anything fancier (icons, textures, animated fill
//! gradient) should live in its own plugin and leave this one as a
//! fallback for headless runs / tests.

use bevy::prelude::*;

use crate::systems::sub::{Oxygen, Sub};

/// Plugin that spawns the HUD and keeps it in sync with the sub.
pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_oxygen_hud)
            .add_systems(Update, update_oxygen_hud);
    }
}

/// Root container for the oxygen HUD.
#[derive(Component)]
struct OxygenHud;

/// The coloured fill portion of the oxygen bar. Width is a percentage
/// driven by [`update_oxygen_hud`].
#[derive(Component)]
struct OxygenFill;

/// The oxygen-percent readout ("72%").
#[derive(Component)]
struct OxygenLabel;

/// Overall size of the meter frame, in logical pixels.
const HUD_WIDTH: f32 = 240.0;
const HUD_HEIGHT: f32 = 18.0;

fn spawn_oxygen_hud(mut commands: Commands) {
    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(24.0),
                left: Val::Percent(50.0),
                // Center horizontally by pulling left by half our width.
                margin: UiRect::left(Val::Px(-HUD_WIDTH / 2.0)),
                width: Val::Px(HUD_WIDTH),
                height: Val::Px(HUD_HEIGHT),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.35)),
            Visibility::Hidden,
            OxygenHud,
            Name::new("Oxygen HUD"),
        ))
        .id();

    let fill = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(0.0),
                left: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Px(HUD_HEIGHT),
                ..default()
            },
            BackgroundColor(Color::srgb(0.35, 0.75, 0.95)),
            OxygenFill,
        ))
        .id();

    let label = commands
        .spawn((
            Text::new("O2 100%"),
            TextFont {
                font_size: 12.0,
                ..default()
            },
            TextColor(Color::srgb(0.92, 0.96, 1.0)),
            Node {
                position_type: PositionType::Absolute,
                ..default()
            },
            OxygenLabel,
        ))
        .id();

    commands.entity(root).add_children(&[fill, label]);
}

fn update_oxygen_hud(
    subs: Query<&Oxygen, With<Sub>>,
    mut root: Query<&mut Visibility, (With<OxygenHud>, Without<OxygenFill>)>,
    mut fill: Query<&mut Node, (With<OxygenFill>, Without<OxygenHud>)>,
    mut label: Query<&mut Text, With<OxygenLabel>>,
) {
    let Ok(oxygen) = subs.single() else {
        return;
    };
    let Ok(mut root_vis) = root.single_mut() else {
        return;
    };
    let Ok(mut fill_node) = fill.single_mut() else {
        return;
    };
    let Ok(mut text) = label.single_mut() else {
        return;
    };

    let pct = (oxygen.current / oxygen.max).clamp(0.0, 1.0);

    // Hide the meter when the reserve is essentially full, so the HUD
    // doesn't hog screen space while the sub is parked at the surface.
    *root_vis = if pct >= 0.999 {
        Visibility::Hidden
    } else {
        Visibility::Visible
    };

    fill_node.width = Val::Percent(pct * 100.0);
    *text = Text::new(format!("O2 {:>3.0}%", pct * 100.0));
}
