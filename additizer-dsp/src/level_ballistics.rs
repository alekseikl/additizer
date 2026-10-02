use crate::{NUM_CHANNELS, Sample, units::from_ms};

const ATTACK_TIME: Sample = from_ms(2.0);
const RELEASE_TIME: Sample = from_ms(250.0);

#[derive(Default, Clone, Copy)]
pub struct LevelBallistics {
    level: Sample,
}

impl LevelBallistics {
    fn coeff(sample_rate: Sample, time: Sample) -> Sample {
        (-5.0 / (sample_rate * time.max(from_ms(1.0)))).exp2()
    }

    pub fn process(&mut self, samples: &[Sample], sample_rate: Sample) -> Sample {
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
    channels: [LevelBallistics; NUM_CHANNELS],
}

impl Default for StereoLevelBallistics {
    fn default() -> Self {
        Self {
            channels: [LevelBallistics::default(); NUM_CHANNELS],
        }
    }
}

impl StereoLevelBallistics {
    pub fn process(
        &mut self,
        channels: [&[Sample]; NUM_CHANNELS],
        sample_rate: Sample,
    ) -> [Sample; NUM_CHANNELS] {
        let mut levels = [0.0; NUM_CHANNELS];

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
