use egui::{Color32, ecolor::Hsva};

use addi_engine::Smp;

use crate::utils::hsva;

pub(super) const EDITOR_HEIGHT: f32 = 220.0;
pub(super) const GRAB_RADIUS: f32 = 8.0;
pub(super) const POWER_GRAB_RADIUS: f32 = 6.0;
pub(super) const POINT_RADIUS: f32 = 4.0;
pub(super) const PLAYHEAD_RADIUS: f32 = 6.0;
/// Right playhead draws once it is more than half a pixel from the left.
pub(super) const PLAYHEAD_OVERLAP_SQ: f32 = 0.25;
/// Yellow. Saturation and value are taken from the left playhead.
pub(super) const RIGHT_PLAYHEAD_HUE: f32 = 1.0 / 6.0;
pub(super) const POWER_RADIUS: f32 = 3.0;
pub(super) const MIN_POWER_WIDTH: f32 = 3.0;
/// Snap when the pointer is within this many pixels of a grid line.
pub(super) const SNAP: f32 = 8.0;
/// Power change for a pointer move of one editor height. Matches Vital's line editor.
pub(super) const POWER_MOUSE_MULTIPLIER: Smp = 9.0;
/// A press stays a click until the pointer moves this far.
pub(super) const MAX_CLICK_DIST: f32 = 2.0;
/// A press stays a click until it has been held this long.
pub(super) const MAX_CLICK_DURATION: f64 = 0.2;

pub(super) const WAVE_COLOR: Color32 = Color32::from_rgb(0x4a, 0xb0, 0xff);
pub(super) const LOOP_COLOR: Color32 = Color32::from_rgb(0xf0, 0xc0, 0x4a);
pub(super) const LOOP_FILL_COLOR: Hsva = hsva(0.0, 0.9092, 1.0, 0.05);
pub(super) const RELEASE_COLOR: Color32 = Color32::from_rgb(0xff, 0x7a, 0x59);
pub(super) const FILL_TOP_ALPHA: u8 = 0x47;
pub(super) const FILL_BOTTOM_ALPHA: u8 = 0x28;
pub(super) const ENVELOPE_WAVE: Hsva = hsva(0.3374, 0.9, 0.2, 1.0);
pub(super) const ENVELOPE_POWER: Hsva = hsva(0.348, 0.7, 0.4, 1.0);
pub(super) const GRID_COLOR: Color32 = Color32::from_rgba_unmultiplied_const(255, 255, 255, 28);
pub(super) const POINT_COLOR: Color32 = Color32::from_rgb(0xe8, 0xf4, 0xff);
pub(super) const POWER_COLOR: Color32 = Color32::from_rgb(0x9a, 0xd0, 0xff);
pub(super) const LABEL_BG_COLOR: Hsva = hsva(0.115, 0.05, 0.01, 1.0);
pub(super) const LABEL_BORDER_COLOR: Hsva = hsva(0.115, 0.05, 0.2, 1.0);
pub(super) const LABEL_TEXT_COLOR: Hsva = hsva(0.115, 0.05, 0.5, 1.0);
pub(super) const LABEL_PADDING: f32 = 6.0;
pub(super) const LABEL_POINTER_GAP: f32 = 4.0;
pub(super) const LABEL_CORNER_RADIUS: f32 = 2.0;
/// Space between a point and its loop or release letter.
pub(super) const MARKER_GAP: f32 = 3.0;

pub(super) const HELP: &str = "Double-click to add or remove a point. Drag a midpoint to bend the segment. Hold Shift to mirror that bend onto the next segment. Right-click to flip.";
pub(super) const HELP_MARKERS: &str = "Double-click to add or remove a point. Drag a midpoint to bend the segment. Hold Shift to mirror that bend onto the next segment. Right-click a point to set the loop or release.";
pub(super) const HELP_PAINT: &str = "Drag to paint this shape into each grid column, up to the pointer. Hold Shift to ignore the vertical grid.";
pub(super) const SMOOTH_TIP: &str = "Ease this segment with a sine curve.";
pub(super) const LOOP_TIP: &str = "Repeat from here. With no release point the loop runs to the end. With one, it runs until release while the key is held.";
pub(super) const RELEASE_TIP: &str = "Sustain this level while the key is held. On release, continue from here starting at the current level.";

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
