use std::f32::consts;

use crate::{phase::Phasor, stereo_sample::StereoSample};

pub struct AdditiveOscillator {
    phasor: Phasor,
    frequency: f32,
}

impl AdditiveOscillator {
    pub fn new(initial_phase: f32, frequency: f32) -> Self {
        Self {
            phasor: Phasor::new(initial_phase),
            frequency,
        }
    }

    pub fn phasor(&self) -> &Phasor {
        &self.phasor
    }

    pub fn tick(&mut self, sample_rate: f32, phase_shift: f32) -> StereoSample {
        let phase = self.phasor.next(sample_rate, self.frequency, phase_shift);
        let value = 0.5 * (phase.for_harmonic(1) * consts::TAU).sin();

        StereoSample(value, value)
    }
}
