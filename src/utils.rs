pub struct GlobalParamValues<'a> {
    pub volume: f32,
    pub harmonics: &'a Vec<f32>,
    pub subharmonics: &'a Vec<f32>,
    pub tail_harmonics: f32,
    pub detune: f32,
}

#[inline]
pub fn from_ms(ms: f32) -> f32 {
    ms * 0.001
}
