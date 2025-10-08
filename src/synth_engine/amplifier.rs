use crate::synth_engine::{
    buffer::{Buffer, ONES_BUFFER, ZEROES_BUFFER, make_zero_buffer},
    routing::{MAX_VOICES, ModuleId, ModuleInput, Router},
    synth_module::{ProcessParams, SynthModule},
};
use itertools::izip;

pub struct AmplifierVoice {
    input: Buffer,
    level_mod_input: Buffer,
    output: Buffer,
}

impl AmplifierVoice {
    pub fn new() -> Self {
        Self {
            input: make_zero_buffer(),
            level_mod_input: make_zero_buffer(),
            output: make_zero_buffer(),
        }
    }
}

impl Default for AmplifierVoice {
    fn default() -> Self {
        Self::new()
    }
}

pub struct AmplifierModule {
    module_id: ModuleId,
    level: f32,
    voices: [AmplifierVoice; MAX_VOICES],
}

impl AmplifierModule {
    pub fn new(module_id: ModuleId) -> Self {
        Self {
            module_id,
            level: 0.8,
            voices: Default::default(),
        }
    }

    fn process_voice(&mut self, params: &ProcessParams, router: &dyn Router, voice_idx: usize) {
        let voice = &mut self.voices[voice_idx];
        let input = router
            .get_input(
                ModuleInput::AmplifierInput(self.module_id),
                voice_idx,
                &mut voice.input,
            )
            .unwrap_or(&ZEROES_BUFFER);
        let level_mod = router
            .get_input(
                ModuleInput::AmplifierLevel(self.module_id),
                voice_idx,
                &mut voice.level_mod_input,
            )
            .unwrap_or(&ONES_BUFFER);

        for (out, input, modulation, _) in
            izip!(&mut voice.output, input, level_mod, 0..params.samples)
        {
            *out = input * self.level * modulation;
        }
    }
}

impl SynthModule for AmplifierModule {
    fn get_id(&self) -> ModuleId {
        self.module_id
    }

    fn get_output(&self, voice_idx: usize) -> &Buffer {
        &self.voices[voice_idx].output
    }

    fn note_on(&mut self, _: &super::synth_module::NoteOnParams) {}
    fn note_off(&mut self, _: &super::synth_module::NoteOffParams) {}

    fn process(&mut self, params: &ProcessParams, router: &dyn Router) {
        for voice_idx in params.active_voices {
            self.process_voice(params, router, *voice_idx);
        }
    }
}
