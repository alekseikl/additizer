use std::sync::Arc;

use crate::{
    phase::{MAX_SUBHARMONIC, Phasor},
    stereo_sample::StereoSample,
    utils::GlobalParamValues,
};

pub struct AdditiveOscillator {
    phasor: Phasor,
    frequency: f32,
    subharmonics: i32,
    sine_table: Arc<Vec<f32>>,
}

impl AdditiveOscillator {
    pub fn new(initial_phase: f32, frequency: f32, sine_table: &Arc<Vec<f32>>) -> Self {
        Self {
            phasor: Phasor::new(initial_phase),
            frequency,
            subharmonics: MAX_SUBHARMONIC,
            sine_table: sine_table.clone(),
        }
    }

    pub fn phasor(&self) -> &Phasor {
        &self.phasor
    }

    pub fn set_subharmonics(&mut self, subharmonics: i32) {
        self.subharmonics = subharmonics
    }

    pub fn tick(
        &mut self,
        sample_rate: f32,
        phase_shift: f32,
        global_params: &GlobalParamValues,
    ) -> StereoSample {
        let phase = self.phasor.next(sample_rate, self.frequency, phase_shift);
        let max_harmonic = (0.5 * sample_rate / self.frequency).floor() as i32;
        let mut sum: f32 = 0.0;

        for i in 1..max_harmonic {
            sum += 0.5 * self.sine_table[(phase.for_harmonic(i) * sample_rate).floor() as usize]
                / i as f32
        }

        for i in 2..(2 + global_params.subharmonics).min(MAX_SUBHARMONIC + 1) {
            sum += 0.5 * self.sine_table[(phase.for_subharmonic(i) * sample_rate).floor() as usize]
                / i as f32
        }

        StereoSample(sum, sum)
    }
}
