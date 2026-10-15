//! Shared pitch and cutoff constants.

use crate::{Smp, math::note_to_pitch};

/// MIDI note number for middle C (C4).
pub const C4_NOTE: u8 = 60;
/// Pitch of C4 in octave units (relative to A4).
pub const C4_PITCH: Smp = note_to_pitch(C4_NOTE as Smp);
/// Lowest cutoff, in octave units (same space as pitch).
pub const MIN_CUTOFF: Smp = -4.0;
/// Highest cutoff, in octave units (same space as pitch).
pub const MAX_CUTOFF: Smp = 7.0;

pub(crate) const A4_FREQ: Smp = 440.0;
