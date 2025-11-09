use crate::synth_engine::{
    InputType, ModuleId, ModuleInput, ModuleType, Sample, SynthModule,
    buffer::{Buffer, make_zero_buffer},
    routing::{MAX_VOICES, NUM_CHANNELS, OutputType, Router},
    synth_module::{NoteOnParams, ProcessParams},
};

struct Voice {
    from_value: Sample,
    output: Buffer,
}

impl Voice {
    pub fn new() -> Self {
        Self {
            from_value: 0.0,
            output: make_zero_buffer(),
        }
    }
}

impl Default for Voice {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Default)]
struct Channel {
    voices: [Voice; MAX_VOICES],
}

pub struct ScalarAdapter {
    id: ModuleId,
    channels: [Channel; NUM_CHANNELS],
}

impl ScalarAdapter {
    pub fn new(id: ModuleId) -> Self {
        Self {
            id,
            channels: Default::default(),
        }
    }
}

impl SynthModule for ScalarAdapter {
    fn id(&self) -> ModuleId {
        self.id
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::ScalarAdapter
    }

    fn is_spectral_rate(&self) -> bool {
        false
    }

    fn inputs(&self) -> &'static [InputType] {
        &[InputType::ScalarInput]
    }

    fn outputs(&self) -> &'static [OutputType] {
        &[OutputType::Output]
    }

    fn note_on(&mut self, params: &NoteOnParams, router: &dyn Router) {
        for (channel_idx, channel) in self.channels.iter_mut().enumerate() {
            channel.voices[params.voice_idx].from_value = router
                .get_scalar_input(
                    ModuleInput::scalar_input(self.id),
                    params.voice_idx,
                    channel_idx,
                )
                .unwrap_or(0.0);
        }
    }

    fn process(&mut self, params: &ProcessParams, router: &dyn Router) {
        let steps_mult = (params.samples as Sample).recip();

        for (channel_idx, channel) in self.channels.iter_mut().enumerate() {
            for voice_idx in params.active_voices {
                let voice = &mut channel.voices[*voice_idx];
                let to_value = router
                    .get_scalar_input(ModuleInput::scalar_input(self.id), *voice_idx, channel_idx)
                    .unwrap_or(0.0);
                let step = (to_value - voice.from_value) * steps_mult;

                for (out, idx) in voice.output.iter_mut().zip(0..params.samples) {
                    *out = voice.from_value + step * idx as Sample;
                }

                voice.from_value = to_value;
            }
        }
    }

    fn get_buffer_output(&self, voice_idx: usize, channel_idx: usize) -> &Buffer {
        &self.channels[channel_idx].voices[voice_idx].output
    }
}
