use crate::{
    synth_engine::{Sample, envelope::SLOPE_POWER_SCALE},
    utils::power_scale,
};

pub trait CurveFunction {
    fn calc(&self, arg: Sample) -> Sample;
}

pub struct Exponential {
    power: Sample,
}

impl Exponential {
    pub fn new(curvature: Sample) -> Self {
        Self {
            power: -curvature.clamp(-1.0, 1.0) * SLOPE_POWER_SCALE,
        }
    }
}

impl CurveFunction for Exponential {
    fn calc(&self, arg: Sample) -> Sample {
        power_scale(arg, self.power)
    }
}
