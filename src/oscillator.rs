use std::sync::Arc;

use crate::{phase::Phase, stereo_sample::StereoSample, utils::GlobalParamValues};

pub struct AdditiveOscillator {
    phase: Phase,
    frequency: f32,
    subharmonics: i32,
    sine_table: Arc<Vec<f32>>,
}

impl AdditiveOscillator {
    pub fn new(initial_phase: Phase, frequency: f32, sine_table: &Arc<Vec<f32>>) -> Self {
        Self {
            phase: initial_phase,
            frequency,
            subharmonics: 4,
            sine_table: sine_table.clone(),
        }
    }

    pub fn phase(&self) -> &Phase {
        &self.phase
    }

    pub fn set_subharmonics(&mut self, subharmonics: i32) {
        self.subharmonics = subharmonics
    }

    pub fn tick(
        &mut self,
        sample_rate: f32,
        _phase_shift: f32,
        global_params: &GlobalParamValues,
    ) -> StereoSample {
        let max_harmonic = (0.5 * sample_rate / self.frequency).floor() as usize;
        let mut sum: f32 = 0.0;

        for i in 1..max_harmonic {
            sum += 0.5 * self.sine_table[self.phase.for_harmonic(i)] / i as f32
                * global_params
                    .harmonics
                    .get(i - 1)
                    .unwrap_or(&global_params.tail_harmonics);
        }

        for i in 2..5 {
            sum += 0.5 * self.sine_table[self.phase.for_subharmonic(i)] / i as f32
                * global_params.subharmonics.get(i - 2).unwrap_or(&0.0);
        }

        self.phase.advance(sample_rate, self.frequency);

        StereoSample(sum, sum)
    }
}
