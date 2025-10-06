use crate::synth_engine::{buffer::Buffer, routing::ModuleInput};

pub trait Context {
    fn get_sample_rate(&self) -> f32;
    fn get_active_voices(&self) -> &[usize];
    fn get_input<'a>(
        &'a self,
        input: ModuleInput,
        voice_idx: usize,
        input_buffer: &'a mut Buffer,
    ) -> Option<&'a Buffer>;
}
