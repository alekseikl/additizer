use crate::synth_engine::Sample;

pub trait CurveFunction {
    fn calc(&self, arg: Sample) -> Sample;
    fn calc_inverse(&self, value: Sample) -> Sample;
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

    fn calc_inverse(&self, value: Sample) -> Sample {
        value.powf(self.power.recip())
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

    fn calc_inverse(&self, value: Sample) -> Sample {
        1.0 - (1.0 - value).powf(self.power.recip())
    }
}

pub struct ExponentialIn {
    rate: Sample,
    linear_threshold_arg: Sample,
    linear_rate: Sample,
    exp_from_value: Sample,
}

impl ExponentialIn {
    pub fn new() -> Self {
        let rate = 5.0;
        let linear_threshold_arg = 0.05;
        let exp_from_value = Self::calc_exp(rate, linear_threshold_arg);
        let linear_rate = exp_from_value / linear_threshold_arg;

        Self {
            rate,
            linear_threshold_arg,
            linear_rate,
            exp_from_value,
        }
    }

    fn calc_exp(rate: Sample, arg: Sample) -> Sample {
        (rate * (arg - 1.0)).exp()
    }
}

impl CurveFunction for ExponentialIn {
    fn calc(&self, arg: Sample) -> Sample {
        if arg < self.linear_threshold_arg {
            self.linear_rate * arg
        } else {
            Self::calc_exp(self.rate, arg)
        }
    }

    fn calc_inverse(&self, value: Sample) -> Sample {
        if value < self.exp_from_value {
            value / self.linear_rate
        } else {
            value.ln() / self.rate + 1.0
        }
    }
}

pub struct ExponentialOut {
    rate: Sample,
    linear_threshold_arg: Sample,
    linear_from_value: Sample,
    linear_rate: Sample,
}

impl ExponentialOut {
    pub fn new() -> Self {
        let rate = 5.0;
        let linear_threshold_arg = 0.95;
        let linear_from_value = Self::calc_exp(rate, linear_threshold_arg);
        let linear_rate = (1.0 - linear_from_value) / (1.0 - linear_threshold_arg);

        Self {
            rate,
            linear_threshold_arg,
            linear_from_value,
            linear_rate,
        }
    }

    fn calc_exp(rate: Sample, arg: Sample) -> Sample {
        1.0 - (-rate * arg).exp()
    }
}

impl CurveFunction for ExponentialOut {
    fn calc(&self, arg: Sample) -> Sample {
        if arg < self.linear_threshold_arg {
            Self::calc_exp(self.rate, arg)
        } else {
            self.linear_from_value + (arg - self.linear_threshold_arg) * self.linear_rate
        }
    }

    fn calc_inverse(&self, value: Sample) -> Sample {
        if value < self.linear_from_value {
            -(1.0 - value).ln() / self.rate
        } else {
            self.linear_threshold_arg + (value - self.linear_from_value) / self.linear_rate
        }
    }
}
