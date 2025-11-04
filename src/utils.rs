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

pub const fn st_to_octave(st: Sample) -> Sample {
    st * ST_TO_OCTAVE_MULT
}
