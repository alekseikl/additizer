mod common;
pub mod rel_segments;
mod rel_widget_plot;
pub mod time_segments;

pub(crate) use common::{PLAYHEAD_OVERLAP_SQ, RIGHT_PLAYHEAD_HUE, horizontal_span, paint_fill};
pub use rel_segments::RelSegmentsEditor;
pub(crate) use rel_widget_plot::{CurveMap, RelWidgetPlot};
pub use time_segments::TimeSegmentsEditor;
