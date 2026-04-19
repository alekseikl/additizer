use crate::{synth_engine::Sample, utils::from_ms};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

const SMOOTHING_TIME: Sample = from_ms(300.0);

#[derive(Clone, Copy)]
pub struct SmoothedSampleParams {
    smooth_mult: Sample,
    smooth_steps: u32,
}

impl SmoothedSampleParams {
    pub fn new(sample_rate: Sample) -> Self {
        Self {
            smooth_mult: 0.0001f64.powf((sample_rate * SMOOTHING_TIME).recip() as f64) as f32,
            smooth_steps: (sample_rate * SMOOTHING_TIME).round() as u32,
        }
    }
}

#[derive(Default, Clone, Copy)]
pub struct SmoothedSample {
    value: Sample,
    prev_value: Sample,
    steps: u32,
}

impl SmoothedSample {
    pub fn new(value: Sample) -> Self {
        Self {
            value,
            prev_value: value,
            steps: u32::MAX,
        }
    }

    pub fn get(&self) -> Sample {
        self.value
    }

    pub fn set(&mut self, new_value: Sample) {
        if self.steps == u32::MAX {
            self.prev_value = self.value;
        }

        self.value = new_value;
        self.steps = 0;
    }

    pub fn check_needs_smoothing(&mut self, params: &SmoothedSampleParams) -> bool {
        let need = self.steps < params.smooth_steps;

        if !need {
            self.steps = u32::MAX;
        }

        need
    }

    pub fn smoothed_buff(&mut self, buff: &mut [Sample], params: &SmoothedSampleParams) {
        let rev_smooth_mult = 1.0 - params.smooth_mult;

        for out in buff.iter_mut() {
            self.prev_value = self
                .value
                .mul_add(rev_smooth_mult, self.prev_value * params.smooth_mult);
            *out = self.prev_value;
        }

        self.steps = self.steps.wrapping_add(buff.len() as u32);
    }

    pub fn smoothed_iter(&mut self, params: &SmoothedSampleParams) -> SmoothedSampleIter<'_> {
        SmoothedSampleIter {
            smoother: self,
            smooth_mult: params.smooth_mult,
        }
    }
}

pub struct SmoothedSampleIter<'a> {
    smoother: &'a mut SmoothedSample,
    smooth_mult: Sample,
}

impl Iterator for SmoothedSampleIter<'_> {
    type Item = Sample;

    #[inline(always)]
    fn next(&mut self) -> Option<Sample> {
        self.smoother.prev_value = self.smoother.value.mul_add(
            1.0 - self.smooth_mult,
            self.smoother.prev_value * self.smooth_mult,
        );
        self.smoother.steps = self.smoother.steps.wrapping_add(1);
        Some(self.smoother.prev_value)
    }
}

impl Serialize for SmoothedSample {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.value.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for SmoothedSample {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = Sample::deserialize(deserializer)?;
        Ok(Self::new(value))
    }
}

impl From<f32> for SmoothedSample {
    fn from(value: f32) -> Self {
        Self::new(value)
    }
}
