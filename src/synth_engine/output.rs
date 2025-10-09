use itertools::izip;

use crate::synth_engine::{
    buffer::{Buffer, ZEROES_BUFFER, make_zero_buffer},
    routing::{ModuleId, ModuleInput, Router},
    synth_module::{ProcessParams, SynthModule},
};

pub struct OutputModule {
    voice_input_buffer: Buffer,
}

impl OutputModule {
    pub fn new() -> Self {
        Self {
            voice_input_buffer: make_zero_buffer(),
        }
    }

    pub fn read_output<'a>(
        &mut self,
        params: &ProcessParams,
        router: &dyn Router,
        outputs: impl Iterator<Item = &'a mut [f32]>,
    ) {
        for (channel, output) in outputs.enumerate() {
            output.fill(0.0);

            for voice_idx in params.active_voices {
                let input = router
                    .get_input(
                        ModuleInput::Output,
                        *voice_idx,
                        channel,
                        &mut self.voice_input_buffer,
                    )
                    .unwrap_or(&ZEROES_BUFFER);

                for (out, input, _) in izip!(output.iter_mut(), input, 0..params.samples) {
                    *out += input;
                }
            }
        }
    }
}

impl SynthModule for OutputModule {
    fn get_id(&self) -> ModuleId {
        0
    }

    fn get_output(&self, _: usize, _channel: usize) -> &Buffer {
        panic!("OutputModule::get_output() not implemented.")
    }

    fn note_on(&mut self, _: &super::synth_module::NoteOnParams) {}
    fn note_off(&mut self, _: &super::synth_module::NoteOffParams) {}

    fn process(&mut self, _ctx: &ProcessParams, _router: &dyn Router) {
        panic!("OutputModule::process() not implemented.")
    }
}
