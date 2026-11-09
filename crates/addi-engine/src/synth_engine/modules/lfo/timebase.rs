use serde::{Deserialize, Serialize};

use crate::synth_engine::{HostTimeline, Smp, phase::Phase};

/// Shortest musical cycle, as a fraction of the selected unit.
const MIN_MUSICAL_RATE: Smp = 0.01;

/// Unit of the LFO rate.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Timebase {
    #[default]
    Hertz,
    Kilohertz,
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
    pub const ALL: [Self; 19] = [
        Self::Hertz,
        Self::Kilohertz,
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
            Self::Hertz => "Hertz",
            Self::Kilohertz => "Kilohertz",
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

    /// Hertz for a rate of `1` at `tempo_bpm`.
    pub fn hz_per_unit(self, tempo_bpm: Smp) -> Smp {
        match self {
            Self::Hertz => 1.0,
            Self::Kilohertz => 1_000.0,
            Self::Hold => 0.0,
            _ => {
                let Some((num, den)) = self.cycle_quarters() else {
                    return 0.0;
                };

                tempo_bpm * (den as Smp) / (60.0 * (num as Smp))
            }
        }
    }

    /// Quarter-note length of one cycle at rate 1, as `(numerator, denominator)`.
    fn cycle_quarters(self) -> Option<(u32, u32)> {
        Some(match self {
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
            Self::Hertz | Self::Kilohertz | Self::Hold => return None,
        })
    }

    /// Phase a steady `rate` would have reached at `timeline`.
    ///
    /// Musical units follow beats. Hertz and kilohertz follow seconds. The rate is resolved the
    /// same way as [`Self::map_phase_inc`], so a constant rate stays on this phase.
    pub(super) fn transport_phase(
        self,
        rate: Smp,
        timeline: &HostTimeline,
        sample_offset: usize,
    ) -> Phase {
        let cycles = if let Some((num, den)) = self.cycle_quarters() {
            let beats = timeline.beats_at(sample_offset);
            let rate = (rate as f64).max(MIN_MUSICAL_RATE as f64);

            beats * (den as f64) / ((num as f64) * rate)
        } else {
            let seconds = timeline.seconds_at(sample_offset);
            let hz = match self {
                Self::Kilohertz => 1_000.0,
                Self::Hold => 0.0,
                _ => 1.0,
            };

            seconds * (rate as f64) * hz
        };

        if !cycles.is_finite() {
            return Phase::ZERO;
        }

        // `floor` keeps the fraction in `[0, 1)`, including for a negative position.
        let fraction = cycles - cycles.floor();

        Phase::from_normalized(fraction as Smp)
    }

    /// Replaces each rate with its phase increment. The timebase is resolved once.
    pub(super) fn map_phase_inc(self, rates: &mut [Smp], tempo_bpm: Smp, phase_scale: Smp) {
        let step = self.hz_per_unit(tempo_bpm) * phase_scale;

        if self.cycle_quarters().is_some() {
            for rate in rates {
                *rate = step / (*rate).max(MIN_MUSICAL_RATE);
            }
        } else {
            for rate in rates {
                *rate *= step;
            }
        }
    }
}

#[cfg(test)]
mod tests;
