use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{Smp, math::from_ms};

const SMOOTHING_TIME: Smp = from_ms(300.0);

#[derive(Clone, Copy)]
pub struct SmoothedSampleParams {
    smooth_mult: Smp,
    smooth_steps: u32,
}

impl SmoothedSampleParams {
    pub fn new(sample_rate: Smp) -> Self {
        Self {
            smooth_mult: 0.0001f64.powf((sample_rate * SMOOTHING_TIME).recip() as f64) as f32,
            smooth_steps: (sample_rate * SMOOTHING_TIME).round() as u32,
        }
    }
}

#[derive(Default, Clone)]
pub struct SmoothedSample {
    value: Smp,
    prev_value: Smp,
    steps: u32,
}

impl SmoothedSample {
    pub fn new(value: Smp) -> Self {
        Self {
            value,
            prev_value: value,
            steps: u32::MAX,
        }
    }

    pub fn get(&self) -> Smp {
        self.value
    }

    pub fn set(&mut self, new_value: Smp) {
        if self.steps == u32::MAX {
            self.prev_value = self.value;
        }

        self.value = new_value;
        self.steps = 0;
    }

    pub fn check_needs_smoothing(&self, params: &SmoothedSampleParams) -> bool {
        self.steps < params.smooth_steps
    }

    pub fn advance(&mut self, params: &SmoothedSampleParams, samples: usize) {
        if self.steps < params.smooth_steps {
            // Instead of sharing smoothing result across voices via buffer - calculate it again
            // for every voice and at the advance stage.
            // Since this is a rare operation initiated by the UI, it wont't hurt performance.
            let rev_smooth_mult = 1.0 - params.smooth_mult;

            for _ in 0..samples {
                self.prev_value = self
                    .value
                    .mul_add(rev_smooth_mult, self.prev_value * params.smooth_mult);
            }

            self.steps = self.steps.wrapping_add(samples as u32);
        } else {
            // Set u32::MAX to indicate for set() method that smoothing has been finished
            self.steps = u32::MAX;
        }
    }

    pub fn smoothed_buff(&self, buff: &mut [Smp], params: &SmoothedSampleParams) {
        let rev_smooth_mult = 1.0 - params.smooth_mult;
        let mut prev_value = self.prev_value;

        for out in buff.iter_mut() {
            prev_value = self
                .value
                .mul_add(rev_smooth_mult, prev_value * params.smooth_mult);
            *out = prev_value;
        }
    }

    pub fn smoothed_iter(&self, params: &SmoothedSampleParams) -> SmoothedSampleIter<'_> {
        SmoothedSampleIter {
            smoother: self,
            smooth_mult: params.smooth_mult,
            prev_value: self.prev_value,
        }
    }
}

pub struct SmoothedSampleIter<'a> {
    smoother: &'a SmoothedSample,
    smooth_mult: Smp,
    prev_value: Smp,
}

impl Iterator for SmoothedSampleIter<'_> {
    type Item = Smp;

    #[inline(always)]
    fn next(&mut self) -> Option<Smp> {
        self.prev_value = self
            .smoother
            .value
            .mul_add(1.0 - self.smooth_mult, self.prev_value * self.smooth_mult);
        Some(self.prev_value)
    }
}

impl Serialize for SmoothedSample {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.value.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for SmoothedSample {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = Smp::deserialize(deserializer)?;
        Ok(Self::new(value))
    }
}

impl From<f32> for SmoothedSample {
    fn from(value: f32) -> Self {
        Self::new(value)
    }
}
