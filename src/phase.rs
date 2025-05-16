pub const MAX_SUBHARMONIC: i32 = 4;
const MAX_PERIODS: i32 = 12;

#[derive(Debug, Clone, Copy)]
pub struct Phase {
    phase: f32,
    full_periods: i32,
}

impl Phase {
    pub fn new(initial_phase: f32) -> Self {
        Self {
            phase: Self::wrap(initial_phase),
            full_periods: 0,
        }
    }

    pub fn wrap(phase: f32) -> f32 {
        if phase < 0.0 {
            1.0 + phase.fract()
        } else {
            phase.fract()
        }
    }

    pub fn value(&self) -> f32 {
        self.phase
    }

    pub fn for_harmonic(&self, harmonic: i32) -> f32 {
        Self::wrap(self.phase * harmonic as f32)
    }

    pub fn for_split_harmonic(&self, harmonic: i32, split: i32) -> f32 {
        let phase = (self.full_periods % split) as f32 + self.phase;

        Self::wrap(phase * (harmonic as f32 + 1.0 / split as f32))
    }

    pub fn for_subharmonic(&self, subharmonic: i32) -> f32 {
        let phase = (self.full_periods % subharmonic) as f32 + self.phase;

        Self::wrap(phase / subharmonic as f32)
    }

    fn modulated(&self, phase_shift: f32) -> Self {
        let shifted = self.phase + phase_shift;

        Self {
            phase: Self::wrap(shifted),
            full_periods: self.full_periods + shifted.floor() as i32,
        }
    }

    fn advance(&mut self, sample_rate: f32, frequency: f32) {
        self.phase += frequency / sample_rate;

        if self.phase >= 1.0 {
            self.phase -= 1.0;
            self.full_periods += 1;

            if self.full_periods >= MAX_PERIODS {
                self.full_periods = 0;
            }
        }
    }
}

pub struct Phasor {
    phase: Phase,
}

impl Phasor {
    pub fn new(initial_phase: f32) -> Self {
        Self {
            phase: Phase::new(initial_phase),
        }
    }

    pub fn current(&self) -> Phase {
        self.phase
    }

    pub fn next(&mut self, sample_rate: f32, frequency: f32, phase_shift: f32) -> Phase {
        let next_phase = self.phase.modulated(phase_shift);

        self.phase.advance(sample_rate, frequency);
        next_phase
    }
}
