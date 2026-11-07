use egui::Color32;

pub(super) use super::super::common::{
    EDITOR_HEIGHT, ENVELOPE_POWER, ENVELOPE_WAVE, GRAB_RADIUS, GRID_COLOR, LOOP_COLOR,
    LOOP_FILL_COLOR, MIN_POWER_WIDTH, POINT_COLOR, POINT_RADIUS, POWER_GRAB_RADIUS, POWER_RADIUS,
    RELEASE_COLOR,
};

pub(super) const POINT_ID: &str = "segments-point";
pub(super) const POWER_ID: &str = "segments-power";

pub(super) const WAVE_COLOR: Color32 = Color32::from_rgb(0x4a, 0xb0, 0xff);
pub(super) const POWER_COLOR: Color32 = Color32::from_rgb(0x9a, 0xd0, 0xff);

/// Stroke of the outlined tool icons.
pub(super) const TOOL_ICON_STROKE: f32 = 1.5;
/// Icon inset inside a tool button.
pub(super) const TOOL_ICON_INSET: f32 = 3.0;
/// Mouse-cursor arrow of the edit tool, `(x, y)` within its own bounds, `y` up.
pub(super) const CURSOR_ICON: [(f32, f32); 7] = [
    (0.0, 1.0),   // tip
    (0.0, 0.19),  // left edge bottom
    (0.32, 0.41), // tail clicks in here
    (0.54, 0.0),  // tail tip
    (0.79, 0.08),
    (0.57, 0.46),
    (1.0, 0.46), // right edge bottom
];
/// Triangulation of [`CURSOR_ICON`]; the arrow is concave at the notch and the tail joint.
pub(super) const CURSOR_TRIANGLES: [(usize, usize, usize); 5] = [
    (0, 1, 2), // head left
    (0, 2, 5), // head middle
    (0, 5, 6), // head right
    (2, 3, 4), // tail
    (2, 4, 5),
];
/// Width of the cursor arrow relative to its height.
pub(super) const CURSOR_ICON_ASPECT: f32 = 14.0 / 18.5;
pub(super) const PAINT_COLUMN_ALPHA: u8 = 36;
