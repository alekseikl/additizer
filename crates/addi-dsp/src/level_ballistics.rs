use crate::{Smp, math::from_ms};

const CHANNELS: usize = 2;

const ATTACK_TIME: Smp = from_ms(2.0);
const RELEASE_TIME: Smp = from_ms(250.0);

#[derive(Default, Clone, Copy)]
pub struct LevelBallistics {
    level: Smp,
}

impl LevelBallistics {
    fn coeff(sample_rate: Smp, time: Smp) -> Smp {
        (-5.0 / (sample_rate * time.max(from_ms(1.0)))).exp2()
    }

    pub fn process(&mut self, samples: &[Smp], sample_rate: Smp) -> Smp {
        if samples.is_empty() {
            return self.level;
        }

        let attack_coeff = Self::coeff(sample_rate, ATTACK_TIME);
        let release_coeff = Self::coeff(sample_rate, RELEASE_TIME);

        for &sample in samples {
            let input = sample.abs();
            let coeff = if input > self.level {
                attack_coeff
            } else {
                release_coeff
            };
            self.level = input.mul_add(1.0 - coeff, self.level * coeff);
        }

        self.level
    }
}

pub struct StereoLevelBallistics {
    channels: [LevelBallistics; CHANNELS],
}

impl Default for StereoLevelBallistics {
    fn default() -> Self {
        Self {
            channels: [LevelBallistics::default(); CHANNELS],
        }
    }
}

impl StereoLevelBallistics {
    pub fn process(&mut self, channels: [&[Smp]; CHANNELS], sample_rate: Smp) -> [Smp; CHANNELS] {
        let mut levels = [0.0; CHANNELS];

        for ((samples, ballistics), level) in channels
            .into_iter()
            .zip(self.channels.iter_mut())
            .zip(levels.iter_mut())
        {
            *level = ballistics.process(samples, sample_rate);
        }

        levels
    }
}

#[cfg(test)]
mod tests;
