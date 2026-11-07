use egui::{Color32, ecolor::Hsva};

use addi_engine::Smp;

use crate::utils::hsva;

pub(super) const POINT_ID: &str = "time-segments-point";
pub(super) const POWER_ID: &str = "time-segments-power";

pub(super) const EDITOR_HEIGHT: f32 = 220.0;
pub(super) const GRAB_RADIUS: f32 = 8.0;
pub(super) const POWER_GRAB_RADIUS: f32 = 6.0;
pub(super) const POINT_RADIUS: f32 = 4.0;
pub(super) const PLAYHEAD_RADIUS: f32 = 6.0;
pub(super) const PLAYHEAD_OVERLAP_SQ: f32 = 0.25;
/// Yellow. Saturation and value are taken from the left playhead.
pub(super) const RIGHT_PLAYHEAD_HUE: f32 = 1.0 / 6.0;
pub(super) const POWER_RADIUS: f32 = 3.0;
pub(super) const MIN_POWER_WIDTH: f32 = 3.0;
pub(super) const SNAP: f32 = 8.0;
/// Seconds across the plot until scroll or a drag past the right edge changes it.
pub(super) const DEFAULT_SPAN: Smp = 4.0;
/// Shortest time window, in seconds. Matches a zoomed-in envelope ruler.
pub(super) const MIN_WINDOW: Smp = 0.125;
/// Ruler steps grow by this factor. Major lines sit on every fourth minor line.
pub(super) const RULER_DIVISION: Smp = 4.0;
/// Scroll distance, in points, that doubles or halves the time window.
pub(super) const ZOOM_POINTS: f32 = 320.0;
pub(super) const TIME_LABEL_SIZE: f32 = 9.0;
pub(super) const TIME_LABEL_GAP: f32 = 4.0;
pub(super) const POWER_MOUSE_MULTIPLIER: Smp = 9.0;
pub(super) const MAX_CLICK_DIST: f32 = 2.0;
pub(super) const MAX_CLICK_DURATION: f64 = 0.2;

pub(super) const LOOP_COLOR: Color32 = Color32::from_rgb(0xf0, 0xc0, 0x4a);
pub(super) const LOOP_FILL_COLOR: Hsva = hsva(0.0, 0.9092, 1.0, 0.05);
pub(super) const RELEASE_COLOR: Color32 = Color32::from_rgb(0xff, 0x7a, 0x59);
pub(super) const ENVELOPE_WAVE: Hsva = hsva(0.3374, 0.9, 0.2, 1.0);
pub(super) const ENVELOPE_POWER: Hsva = hsva(0.348, 0.7, 0.4, 1.0);
pub(super) const GRID_COLOR: Color32 = Color32::from_rgba_unmultiplied_const(255, 255, 255, 28);
pub(super) const POINT_COLOR: Color32 = Color32::from_rgb(0xe8, 0xf4, 0xff);
pub(super) const LABEL_BG_COLOR: Hsva = hsva(0.115, 0.05, 0.01, 1.0);
pub(super) const LABEL_BORDER_COLOR: Hsva = hsva(0.115, 0.05, 0.2, 1.0);
pub(super) const LABEL_TEXT_COLOR: Hsva = hsva(0.115, 0.05, 0.5, 1.0);
pub(super) const LABEL_PADDING: f32 = 6.0;
pub(super) const LABEL_POINTER_GAP: f32 = 4.0;
pub(super) const LABEL_CORNER_RADIUS: f32 = 2.0;
pub(super) const MARKER_GAP: f32 = 3.0;

pub(super) const SMOOTH_TIP: &str = "Ease this segment with a sine curve.";
pub(super) const LOOP_TIP: &str = "Repeat from here. With no release point the loop runs to the end. With one, it runs until release while the key is held.";
pub(super) const RELEASE_TIP: &str = "Sustain this level while the key is held. On release, continue from here starting at the current level.";
