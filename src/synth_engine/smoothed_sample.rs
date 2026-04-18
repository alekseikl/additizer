use crate::{synth_engine::Sample, utils::from_ms};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

const SMOOTHING_TIME: Sample = from_ms(20.0);

#[derive(Default, Clone, Copy)]
pub struct SmoothedSample {
    value: Sample,
    prev_value: Sample,
}

impl SmoothedSample {
    pub fn new(value: Sample) -> Self {
        Self {
            value,
            prev_value: value,
        }
    }

    pub fn calc_smooth_mult(sample_rate: Sample) -> Sample {
        (-5.0 / (sample_rate * SMOOTHING_TIME)).exp2()
    }

    pub fn get(&self) -> Sample {
        self.value
    }

    pub fn set(&mut self, new_value: Sample) {
        self.value = new_value;
    }

    pub fn smoothed_buff(&mut self, buff: &mut [Sample], smooth_mult: Sample) {
        let rev_smooth_mult = 1.0 - smooth_mult;

        for out in buff {
            self.prev_value = self
                .value
                .mul_add(rev_smooth_mult, self.prev_value * smooth_mult);
            *out = self.prev_value;
        }
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
