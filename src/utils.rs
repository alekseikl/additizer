use crate::synth_engine::Sample;

const SEMITONE_TO_OCTAVE_MULT: Sample = 12.0f32.recip();

#[inline]
pub fn from_ms(ms: f32) -> f32 {
    ms * 0.001
}

#[inline(always)]
pub fn st_to_harmonic(semitones: Sample) -> Sample {
    (semitones * SEMITONE_TO_OCTAVE_MULT).exp2()
}
