use itertools::izip;

use crate::synth_engine::{
    buffer::{Buffer, ZEROES_BUFFER, make_zero_buffer},
    routing::{ModuleId, ModuleInput, Router},
    synth_module::{ProcessParams, SynthModule},
};

pub struct OutputModule {
    voice_input_buffer: Buffer,
    output: Buffer,
}

impl OutputModule {
    #![allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            voice_input_buffer: make_zero_buffer(),
            output: make_zero_buffer(),
        }
    }
}

impl SynthModule for OutputModule {
    fn get_id(&self) -> ModuleId {
        0
    }

    fn get_output(&self, _: usize) -> &Buffer {
        &self.output
    }

    fn note_on(&mut self, _: &super::synth_module::NoteOnParams) {}
    fn note_off(&mut self, _: &super::synth_module::NoteOffParams) {}

    fn process(&mut self, ctx: &ProcessParams, router: &dyn Router) {
        self.output.fill(0.0);

        for voice_idx in &ctx.active_voices {
            let input = router
                .get_input(
                    ModuleInput::Output,
                    *voice_idx,
                    &mut self.voice_input_buffer,
                )
                .unwrap_or(&ZEROES_BUFFER);

            for (out, input, _) in izip!(&mut self.output, input, 0..ctx.samples) {
                *out += input;
            }
        }
    }
}
