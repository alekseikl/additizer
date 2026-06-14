use crate::synth_engine::Sample;

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

    pub fn apply_if_needed(
        &mut self,
        samples: usize,
        sample_rate: Sample,
        time: Sample,
        buff: &mut [Sample],
    ) {
        if time >= SMOOTHING_TIME_THRESHOLD {
            self.update(sample_rate, time);

            for sample in buff.iter_mut().take(samples) {
                *sample = self.tick(*sample);
            }
        }
    }

    #[inline(always)]
    pub fn tick(&mut self, value: Sample) -> Sample {
        self.prev_value = value.mul_add(1.0 - self.smooth_mult, self.prev_value * self.smooth_mult);

        self.prev_value
    }
}

impl Default for Smoother {
    fn default() -> Self {
        Self::new()
    }
}
