//! Phase curves ([`rel_segments::RelSegmentsEditor`]) and curves in seconds
//! ([`time_segments::TimeSegmentsEditor`]).

mod common;
pub mod rel_segments;
pub mod time_segments;

pub use rel_segments::RelSegmentsEditor;
pub use time_segments::TimeSegmentsEditor;
