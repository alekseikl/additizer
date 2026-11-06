//! Phase curves ([`rel_segments::RelSegments`]) and curves in seconds ([`time_segments::TimeSegments`]).

mod curve;
pub mod rel_segments;
pub mod time_segments;

pub use curve::{CurveParam, render_segment};
