use super::Envelope;

pub struct ADSR {
    attack_time: f32,
    decay_time: f32,
    sustain_level: f32,
    release_time: f32,
    current_time: f32,
    releasing: bool,
    release_from: f32,
}

impl ADSR {
    pub fn new(attack: f32, decay: f32, sustain: f32, release: f32) -> Self {
        Self {
            attack_time: attack,
            decay_time: decay,
            sustain_level: sustain,
            release_time: release,
            current_time: 0.0,
            releasing: false,
            release_from: 0.0,
        }
    }
}

impl Envelope for ADSR {
    fn value(&self) -> f32 {
        if self.releasing {
            self.release_from * (1.0 - self.current_time / self.release_time)
        } else if self.current_time < self.attack_time {
            self.current_time / self.attack_time
        } else if self.current_time < self.decay_time {
            1.0 - (1.0 - self.sustain_level)
                * ((self.current_time - self.attack_time) / self.decay_time)
        } else {
            self.sustain_level
        }
    }

    fn is_done(&self) -> bool {
        self.releasing && self.current_time >= self.release_time
    }

    fn advance(&mut self, sample_rate: f32) {
        self.current_time += 1000.0 / sample_rate;
    }

    fn release(&mut self) {
        if !self.releasing {
            self.release_from = self.value();
            self.releasing = true;
            self.current_time = 0.0
        }
    }
}
