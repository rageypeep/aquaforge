//! Player inventory and hotbar UI.
//!
//! Owns the list of 5 hotbar slots (block type + count), tracks the active
//! slot, and owns the on-screen hotbar: a row of 5 boxes at bottom center
//! with a colored icon per block, a count, and a yellow border on the
//! active slot.

use bevy::input::ButtonInput;
use bevy::input::keyboard::KeyCode;
use bevy::prelude::*;

use super::blocks::BlockType;

/// Registers the inventory resource and hotbar UI systems.
pub struct InventoryPlugin;

impl Plugin for InventoryPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Inventory>()
            .add_systems(Startup, (spawn_hotbar_ui, spawn_crosshair))
            .add_systems(Update, (select_slot_with_keys, refresh_hotbar_ui));
    }
}

/// Number of hotbar slots.
pub const HOTBAR_SLOTS: usize = 5;

/// One slot in the hotbar: holds a specific block type and a stack count.
#[derive(Clone, Copy, Debug)]
pub struct Slot {
    pub block: BlockType,
    pub count: u32,
}

/// Hotbar + selection state.
#[derive(Resource)]
pub struct Inventory {
    pub slots: [Slot; HOTBAR_SLOTS],
    pub selected: usize,
}

impl Default for Inventory {
    fn default() -> Self {
        Self {
            slots: [
                Slot {
                    block: BlockType::Stone,
                    count: 32,
                },
                Slot {
                    block: BlockType::Sand,
                    count: 32,
                },
                Slot {
                    block: BlockType::Dirt,
                    count: 32,
                },
                Slot {
                    block: BlockType::Coral,
                    count: 16,
                },
                Slot {
                    block: BlockType::Kelp,
                    count: 16,
                },
            ],
            selected: 0,
        }
    }
}

impl Inventory {
    /// Take one unit from the active slot. Returns the block type that was
    /// taken, or `None` if the slot is empty.
    pub fn take_selected(&mut self) -> Option<BlockType> {
        let slot = &mut self.slots[self.selected];
        if slot.count == 0 {
            return None;
        }
        slot.count -= 1;
        Some(slot.block)
    }

    /// Put one unit of `block` into the first matching slot.
    ///
    /// If no slot handles this block type, the pickup is dropped — the
    /// player never loses an existing slot.
    pub fn deposit(&mut self, block: BlockType) {
        for slot in &mut self.slots {
            if slot.block == block {
                slot.count = slot.count.saturating_add(1);
                return;
            }
        }
    }
}

const HOTBAR_KEYS: [KeyCode; HOTBAR_SLOTS] = [
    KeyCode::Digit1,
    KeyCode::Digit2,
    KeyCode::Digit3,
    KeyCode::Digit4,
    KeyCode::Digit5,
];

fn select_slot_with_keys(keys: Res<ButtonInput<KeyCode>>, mut inventory: ResMut<Inventory>) {
    for (i, key) in HOTBAR_KEYS.iter().enumerate() {
        if keys.just_pressed(*key) {
            inventory.selected = i;
        }
    }
}

/// Marker for each of the 5 slot root nodes. `.0` is the slot index.
#[derive(Component)]
struct HotbarSlotNode(usize);

/// Marker for each slot's count text. `.0` is the slot index.
#[derive(Component)]
struct HotbarSlotCount(usize);

const SLOT_SIZE: f32 = 56.0;
const SLOT_BORDER: f32 = 3.0;
const SLOT_GAP: f32 = 6.0;
const ICON_SIZE: f32 = 30.0;

fn selected_border() -> Color {
    Color::srgb(1.0, 0.92, 0.28)
}

fn idle_border() -> Color {
    Color::srgba(0.0, 0.0, 0.0, 0.6)
}

fn slot_background() -> Color {
    Color::srgba(0.03, 0.08, 0.14, 0.78)
}

fn spawn_hotbar_ui(mut commands: Commands, inventory: Res<Inventory>) {
    // Total hotbar width used to horizontally center the container.
    let total_width =
        HOTBAR_SLOTS as f32 * SLOT_SIZE + (HOTBAR_SLOTS as f32 - 1.0) * SLOT_GAP + 8.0;

    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(16.0),
                left: Val::Percent(50.0),
                margin: UiRect {
                    left: Val::Px(-total_width / 2.0),
                    ..default()
                },
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(SLOT_GAP),
                ..default()
            },
            Name::new("Hotbar"),
        ))
        .with_children(|parent| {
            for (i, slot) in inventory.slots.iter().enumerate() {
                let border = if i == inventory.selected {
                    selected_border()
                } else {
                    idle_border()
                };

                parent
                    .spawn((
                        Node {
                            width: Val::Px(SLOT_SIZE),
                            height: Val::Px(SLOT_SIZE),
                            border: UiRect::all(Val::Px(SLOT_BORDER)),
                            flex_direction: FlexDirection::Column,
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BorderColor::all(border),
                        BackgroundColor(slot_background()),
                        HotbarSlotNode(i),
                        Name::new(format!("HotbarSlot {i}")),
                    ))
                    .with_children(|slot_parent| {
                        // Block icon swatch.
                        let icon_color = Color::LinearRgba(slot.block.color());
                        slot_parent.spawn((
                            Node {
                                width: Val::Px(ICON_SIZE),
                                height: Val::Px(ICON_SIZE),
                                margin: UiRect::top(Val::Px(2.0)),
                                ..default()
                            },
                            BackgroundColor(icon_color),
                        ));
                        // Count label.
                        slot_parent.spawn((
                            Text::new(format!("{}", slot.count)),
                            TextFont {
                                font_size: 13.0,
                                ..default()
                            },
                            TextColor(Color::srgba(1.0, 1.0, 1.0, 0.95)),
                            Node {
                                margin: UiRect::top(Val::Px(2.0)),
                                ..default()
                            },
                            HotbarSlotCount(i),
                        ));
                    });
            }
        });
}

fn spawn_crosshair(mut commands: Commands) {
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

/// Keep the hotbar borders and counts in sync with the inventory each frame.
fn refresh_hotbar_ui(
    inventory: Res<Inventory>,
    mut slots: Query<(&HotbarSlotNode, &mut BorderColor)>,
    mut counts: Query<(&HotbarSlotCount, &mut Text)>,
) {
    if !inventory.is_changed() {
        return;
    }

    for (slot, mut border) in &mut slots {
        let color = if slot.0 == inventory.selected {
            selected_border()
        } else {
            idle_border()
        };
        *border = BorderColor::all(color);
    }

    for (count, mut text) in &mut counts {
        text.0 = format!("{}", inventory.slots[count.0].count);
    }
}
