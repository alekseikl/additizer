use crate::synth_engine::{NUM_CHANNELS, Sample, Smoother, StereoSample};

const UI_SAMPLE_RATE: Sample = 60.0;

pub struct StereoSmoother {
    channels: [Smoother; NUM_CHANNELS],
    smooth_time: Sample,
}

impl StereoSmoother {
    pub fn new(initial: StereoSample, smooth_time: Sample) -> Self {
        let mut channels = [Smoother::new(), Smoother::new()];

        for (smoother, value) in channels.iter_mut().zip(initial.iter()) {
            smoother.reset(*value);
        }

        Self {
            channels,
            smooth_time,
        }
    }

    pub fn tick(&mut self, value: StereoSample) -> StereoSample {
        for smoother in &mut self.channels {
            smoother.update(UI_SAMPLE_RATE, self.smooth_time);
        }

        StereoSample::from_iter(
            self.channels
                .iter_mut()
                .zip(value.iter())
                .map(|(smoother, &channel_value)| smoother.tick(channel_value)),
        )
    }
}
