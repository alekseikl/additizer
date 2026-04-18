use crate::synth_engine::Sample;

#[derive(Default, Clone, Copy)]
pub struct InfiniteSmoothed {
    value: Sample,
    prev_value: Sample,
}

impl InfiniteSmoothed {
    pub fn new(value: Sample) -> Self {
        Self {
            value,
            prev_value: value,
        }
    }

    pub fn smooth_mult(sample_rate: Sample, time: Sample) -> Sample {
        (-5.0 / (sample_rate * time.max(0.001))).exp2()
    }

    pub fn set(&mut self, new_value: Sample) {
        self.value = new_value;
    }

    pub fn iter(&mut self, smooth_mult: Sample) -> InfiniteSmoothedIter<'_> {
        InfiniteSmoothedIter {
            smoother: self,
            smooth_mult,
        }
    }
}

impl From<f32> for InfiniteSmoothed {
    fn from(value: f32) -> Self {
        Self::new(value)
    }
}

pub struct InfiniteSmoothedIter<'a> {
    smoother: &'a mut InfiniteSmoothed,
    smooth_mult: Sample,
}

impl Iterator for InfiniteSmoothedIter<'_> {
    type Item = Sample;

    #[inline(always)]
    fn next(&mut self) -> Option<Sample> {
        self.smoother.prev_value = self.smoother.value.mul_add(
            1.0 - self.smooth_mult,
            self.smoother.prev_value * self.smooth_mult,
        );
        Some(self.smoother.prev_value)
    }
}
