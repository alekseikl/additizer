use crate::synth_engine::Sample;

const ST_TO_OCTAVE_MULT: Sample = 12.0f32.recip();

#[inline]
pub const fn from_ms(ms: f32) -> f32 {
    ms * 0.001
}

#[inline(always)]
pub const fn note_to_octave(note: Sample) -> Sample {
    (note - 69.0) / 12.0
}

#[inline(always)]
pub fn octave_to_freq(octave: Sample) -> Sample {
    octave.exp2() * 440.0
}

#[inline(always)]
pub const fn st_to_octave(st: Sample) -> Sample {
    st * ST_TO_OCTAVE_MULT
}

pub struct NthElement {
    mul: isize,
    add: isize,
    inverted: bool,
}

impl NthElement {
    pub fn new(mul: isize, add: isize, inverted: bool) -> Self {
        Self { mul, add, inverted }
    }

    pub fn matches(&self, idx: usize) -> bool {
        let i = idx as isize + 1;
        let result = if self.mul == 0 {
            i == self.add
        } else {
            let scaled = (i - self.add) as f32 / self.mul as f32;

            scaled >= 0.0 && scaled.fract().abs() < f32::EPSILON
        };

        result ^ self.inverted
    }
}
