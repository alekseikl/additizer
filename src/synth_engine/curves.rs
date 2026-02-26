use crate::{synth_engine::Sample, utils::power_scale};

pub trait CurveFunction {
    fn calc(&self, arg: Sample) -> Sample;
}

pub struct Exponential {
    power: Sample,
}

impl Exponential {
    pub fn new(curvature: Sample) -> Self {
        Self {
            power: curvature.clamp(-1.0, 1.0) * -10.0,
        }
    }
}

impl CurveFunction for Exponential {
    fn calc(&self, arg: Sample) -> Sample {
        power_scale(arg, self.power)
    }
}

pub struct ExponentialIn;

impl ExponentialIn {
    const RATE: Sample = 6.0;
    const POWER: Sample = 15.0;

    pub fn new() -> Self {
        Self
    }
}

impl CurveFunction for ExponentialIn {
    fn calc(&self, arg: Sample) -> Sample {
        (Self::RATE * (arg - 1.0)).exp() * (1.0 - (1.0 - arg).powf(Self::POWER))
    }
}

pub struct ExponentialOut;

impl ExponentialOut {
    const RATE: Sample = 6.0;
    const POWER: Sample = 15.0;

    pub fn new() -> Self {
        Self
    }
}

impl CurveFunction for ExponentialOut {
    fn calc(&self, arg: Sample) -> Sample {
        1.0 - ((-Self::RATE * arg).exp() * (1.0 - arg.powf(Self::POWER)))
    }
}
