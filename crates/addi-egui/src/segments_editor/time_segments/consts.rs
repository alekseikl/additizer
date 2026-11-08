use addi_engine::Smp;

pub(super) use super::super::common::{
    EDITOR_HEIGHT, ENVELOPE_POWER, ENVELOPE_WAVE, GRAB_RADIUS, GRID_COLOR, LOOP_COLOR,
    LOOP_FILL_COLOR, MIN_POWER_WIDTH, POINT_COLOR, POINT_RADIUS, POWER_GRAB_RADIUS, POWER_RADIUS,
    RELEASE_COLOR,
};

pub const POINT_ID: &str = "time-segments-point";
pub const POWER_ID: &str = "time-segments-power";

/// Seconds across the plot until the zoom handle or a drag past the right edge changes it.
pub const DEFAULT_SPAN: Smp = 4.0;
/// Shortest time window, in seconds. Matches a zoomed-in envelope ruler.
pub const MIN_WINDOW: Smp = 0.125;
/// Ruler steps grow by this factor. Major lines sit on every fourth minor line.
pub const RULER_DIVISION: Smp = 4.0;
/// Pointer travel, in points, that doubles or halves the time window.
pub const ZOOM_POINTS: f32 = 320.0;
pub const ZOOM_ID: &str = "time-segments-zoom";
/// Gap between the magnifying glass and the top-right corner of the plot.
pub const ZOOM_MARGIN: f32 = 8.0;
/// Drawn size relative to the lens, handle, and stroke below.
pub const ZOOM_SCALE: f32 = 0.75;
/// Lens radius, measured to the middle of the stroke.
pub const ZOOM_LENS: f32 = 6.0;
/// Handle length from the lens stroke to the middle of its round cap.
pub const ZOOM_HANDLE: f32 = 9.5;
pub const ZOOM_STROKE: f32 = 2.0;
pub const TIME_LABEL_SIZE: f32 = 9.0;
pub const TIME_LABEL_GAP: f32 = 4.0;
