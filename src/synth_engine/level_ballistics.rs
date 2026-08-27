use crate::{
    synth_engine::{NUM_CHANNELS, Sample},
    utils::from_ms,
};

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
mod tests {
    use super::*;
    use crate::utils::from_ms;

    const SAMPLE_RATE: Sample = 48_000.0;

    #[test]
    fn silence_stays_at_zero() {
        let mut ballistics = LevelBallistics::default();

        assert_eq!(ballistics.process(&[0.0; 64], SAMPLE_RATE), 0.0);
    }

    #[test]
    fn empty_buffer_keeps_level() {
        let mut ballistics = LevelBallistics::default();

        let level = ballistics.process(&[1.0; 64], SAMPLE_RATE);

        assert!(level > 0.0);
        assert_eq!(ballistics.process(&[], SAMPLE_RATE), level);
    }

    #[test]
    fn attack_rises_faster_than_release_falls() {
        let short_block = [1.0; (SAMPLE_RATE * from_ms(5.0)) as usize];
        let silence = [0.0; (SAMPLE_RATE * from_ms(5.0)) as usize];

        let attack_level = LevelBallistics::default().process(&short_block, SAMPLE_RATE);

        let mut release = LevelBallistics { level: 1.0 };
        let release_level = release.process(&silence, SAMPLE_RATE);

        assert!(attack_level > 1.0 - release_level);
    }

    #[test]
    fn stereo_processes_channels_independently() {
        let mut ballistics = StereoLevelBallistics::default();

        let levels = ballistics.process([&[1.0; 32], &[0.0; 32]], SAMPLE_RATE);

        assert!(levels[0] > 0.0);
        assert_eq!(levels[1], 0.0);
    }
}
