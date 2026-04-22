//! Cursor grab / release.
//!
//! The sub controller (`systems::sub`) reads mouse motion only
//! while the cursor is in [`CursorGrabMode::Locked`], so this module is
//! the single source of truth for when it is. Left-click grabs; Escape
//! releases. Block-editing (`game::edit`) also keys off the same locked
//! state, which is why grabs are gated on a click rather than toggled
//! on startup.

use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

use super::sub::SubPlugin;

/// Input + camera controls plugin.
pub struct ControlsPlugin;

impl Plugin for ControlsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(SubPlugin)
            .add_systems(Update, (grab_cursor_on_click, release_cursor_on_escape));
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
