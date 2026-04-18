use crate::synth_engine::{Sample, buffer::Buffer, types::ScalarOutput};

const SMOOTHING_TIME_THRESHOLD: Sample = 0.0005;

pub struct Smoother {
    smooth_mult: Sample,
    prev_value: Sample,
}

impl Smoother {
    pub fn new() -> Self {
        Self {
            smooth_mult: 0.0,
            prev_value: 0.0,
        }
    }

    pub fn reset(&mut self, initial_value: Sample) {
        self.prev_value = initial_value;
    }

    pub fn update(&mut self, sample_rate: Sample, time: Sample) {
        self.smooth_mult = Sample::from(time > 0.0)
            * (-5.0 / (sample_rate * time.max(SMOOTHING_TIME_THRESHOLD))).exp2();
    }

    #[inline(always)]
    pub fn tick(&mut self, value: Sample) -> Sample {
        self.prev_value = value.mul_add(1.0 - self.smooth_mult, self.prev_value * self.smooth_mult);

        self.prev_value
    }

    #[inline]
    pub fn segment(&mut self, scalar: &ScalarOutput, samples: usize, output: &mut Buffer) {
        let from = scalar.previous();
        let step = (scalar.current() - from) / samples as Sample;

        for (idx, out) in output.iter_mut().enumerate().take(samples) {
            *out = self.tick(step.mul_add(idx as Sample, from));
        }
    }
}

impl Default for Smoother {
    fn default() -> Self {
        Self::new()
    }
}
