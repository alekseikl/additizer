use serde::{Deserialize, Serialize};

use crate::synth_engine::Smp;

/// Shortest envelope, as a fraction of the selected unit.
const MIN_PERIOD: Smp = 0.01;

/// Unit of the envelope length.
///
/// The rate is a period: `1` lasts one unit, and a larger rate lasts longer.
/// [`Timebase::Hold`] stays on the level it already has.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Timebase {
    #[default]
    Seconds,
    Bar,
    Half,
    Quarter,
    Eighth,
    Sixteenth,
    ThirtySecond,
    DottedHalf,
    DottedQuarter,
    DottedEighth,
    DottedSixteenth,
    DottedThirtySecond,
    TripletHalf,
    TripletQuarter,
    TripletEighth,
    TripletSixteenth,
    TripletThirtySecond,
    Hold,
}

impl Timebase {
    pub const ALL: [Self; 18] = [
        Self::Seconds,
        Self::Bar,
        Self::Half,
        Self::Quarter,
        Self::Eighth,
        Self::Sixteenth,
        Self::ThirtySecond,
        Self::DottedHalf,
        Self::DottedQuarter,
        Self::DottedEighth,
        Self::DottedSixteenth,
        Self::DottedThirtySecond,
        Self::TripletHalf,
        Self::TripletQuarter,
        Self::TripletEighth,
        Self::TripletSixteenth,
        Self::TripletThirtySecond,
        Self::Hold,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Seconds => "Seconds",
            Self::Bar => "Bar",
            Self::Half => "Half note",
            Self::Quarter => "Quarter note",
            Self::Eighth => "8th note",
            Self::Sixteenth => "16th note",
            Self::ThirtySecond => "32nd note",
            Self::DottedHalf => "Dotted half note",
            Self::DottedQuarter => "Dotted quarter note",
            Self::DottedEighth => "Dotted 8th note",
            Self::DottedSixteenth => "Dotted 16th note",
            Self::DottedThirtySecond => "Dotted 32nd note",
            Self::TripletHalf => "Triplet half note",
            Self::TripletQuarter => "Triplet quarter note",
            Self::TripletEighth => "Triplet 8th note",
            Self::TripletSixteenth => "Triplet 16th note",
            Self::TripletThirtySecond => "Triplet 32nd note",
            Self::Hold => "Hold",
        }
    }

    /// Hertz for a length of `1` at `tempo_bpm`.
    pub fn hz_per_unit(self, tempo_bpm: Smp) -> Smp {
        let (num, den) = match self {
            Self::Seconds => return 1.0,
            Self::Hold => return 0.0,
            Self::Bar => (4, 1),
            Self::Half => (2, 1),
            Self::Quarter => (1, 1),
            Self::Eighth => (1, 2),
            Self::Sixteenth => (1, 4),
            Self::ThirtySecond => (1, 8),
            Self::DottedHalf => (3, 1),
            Self::DottedQuarter => (3, 2),
            Self::DottedEighth => (3, 4),
            Self::DottedSixteenth => (3, 8),
            Self::DottedThirtySecond => (3, 16),
            Self::TripletHalf => (4, 3),
            Self::TripletQuarter => (2, 3),
            Self::TripletEighth => (1, 3),
            Self::TripletSixteenth => (1, 6),
            Self::TripletThirtySecond => (1, 12),
        };

        tempo_bpm * (den as Smp) / (60.0 * (num as Smp))
    }

    /// Phase advanced per sample for one block. `rate` is the envelope length.
    pub(super) fn phase_inc(self, rate: Smp, tempo_bpm: Smp, phase_scale: Smp) -> Smp {
        self.hz_per_unit(tempo_bpm) * phase_scale / rate.max(MIN_PERIOD)
    }
}

#[cfg(test)]
mod tests;
