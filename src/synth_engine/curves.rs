use crate::synth_engine::Sample;

pub trait CurveFunction {
    fn calc(&self, arg: Sample) -> Sample;
}

pub struct PowerIn {
    power: Sample,
}

impl PowerIn {
    pub fn new(curvature: Sample) -> Self {
        Self {
            power: 1.0 + curvature.clamp(0.0, 1.0) * 9.0,
        }
    }
}

impl CurveFunction for PowerIn {
    fn calc(&self, arg: Sample) -> Sample {
        arg.powf(self.power)
    }
}

pub struct PowerOut {
    power: Sample,
}

impl PowerOut {
    pub fn new(curvature: Sample) -> Self {
        Self {
            power: 1.0 + curvature.clamp(0.0, 1.0) * 9.0,
        }
    }
}

impl CurveFunction for PowerOut {
    fn calc(&self, arg: Sample) -> Sample {
        1.0 - (1.0 - arg).powf(self.power)
    }
}

pub struct ExponentialIn;

impl ExponentialIn {
    const RATE: Sample = 5.0;
    const POWER: Sample = 10.0;

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
    const RATE: Sample = 5.0;
    const POWER: Sample = 10.0;

    pub fn new() -> Self {
        Self
    }
}

impl CurveFunction for ExponentialOut {
    fn calc(&self, arg: Sample) -> Sample {
        1.0 - ((-Self::RATE * arg).exp() * (1.0 - arg.powf(Self::POWER)))
    }
}
