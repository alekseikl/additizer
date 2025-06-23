pub struct GlobalParamValues<'a> {
    pub volume: f32,
    pub harmonics: &'a Vec<f32>,
    pub subharmonics: &'a Vec<f32>,
    pub tail_harmonics: f32,
}
