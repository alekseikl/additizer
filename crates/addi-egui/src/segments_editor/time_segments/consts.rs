use addi_engine::Smp;

pub(super) use super::super::common::{
    EDITOR_HEIGHT, ENVELOPE_POWER, ENVELOPE_WAVE, GRAB_RADIUS, GRID_COLOR, LOOP_COLOR,
    LOOP_FILL_COLOR, MIN_POWER_WIDTH, POINT_COLOR, POINT_RADIUS, POWER_GRAB_RADIUS, POWER_RADIUS,
    RELEASE_COLOR,
};

pub(super) const POINT_ID: &str = "time-segments-point";
pub(super) const POWER_ID: &str = "time-segments-power";

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
