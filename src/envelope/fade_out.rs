use super::Envelope;

pub struct FadeOutEnvelope {
    release_time: f32,
    from_value: f32,
    current_time: f32,
}

impl FadeOutEnvelope {
    pub fn new(from_value: f32) -> Self {
        Self {
            release_time: 5.0,
            from_value,
            current_time: 0.0,
        }
    }
}

impl Envelope for FadeOutEnvelope {
    fn value(&self) -> f32 {
        self.from_value * (1.0 - self.current_time / self.release_time)
    }

    fn is_done(&self) -> bool {
        self.current_time >= self.release_time
    }

    fn advance(&mut self, sample_rate: f32) {
        self.current_time += 1000.0 / sample_rate;
    }

    fn release(&mut self) {}
}
