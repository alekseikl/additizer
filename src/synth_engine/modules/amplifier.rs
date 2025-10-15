use crate::synth_engine::{
    buffer::{Buffer, ONES_BUFFER, ZEROES_BUFFER, make_zero_buffer},
    routing::{MAX_VOICES, ModuleId, ModuleInput, NUM_CHANNELS, Router},
    synth_module::{BufferOutputModule, NoteOffParams, NoteOnParams, ProcessParams, SynthModule},
    types::StereoValue,
};
use itertools::izip;

struct Voice {
    output: Buffer,
}

impl Voice {
    pub fn new() -> Self {
        Self {
            output: make_zero_buffer(),
        }
    }
}

impl Default for Voice {
    fn default() -> Self {
        Self::new()
    }
}

struct Channel {
    level: f32,
    voices: [Voice; MAX_VOICES],
}

impl Default for Channel {
    fn default() -> Self {
        Self {
            level: 1.0,
            voices: Default::default(),
        }
    }
}

struct Common {
    module_id: ModuleId,
    input: Buffer,
    level_mod_input: Buffer,
}

impl Default for Common {
    fn default() -> Self {
        Self {
            module_id: 0,
            input: make_zero_buffer(),
            level_mod_input: make_zero_buffer(),
        }
    }
}

pub struct Amplifier {
    common: Common,
    channels: [Channel; NUM_CHANNELS],
}

impl Amplifier {
    pub fn new() -> Self {
        Self {
            common: Default::default(),
            channels: Default::default(),
        }
    }

    pub fn set_id(&mut self, module_id: ModuleId) {
        self.common.module_id = module_id;
    }

    pub fn set_level(&mut self, level: StereoValue) {
        for (chan, value) in self.channels.iter_mut().zip(level.iter()) {
            chan.level = value;
        }
    }

    fn process_channel_voice(
        common: &mut Common,
        channel: &mut Channel,
        params: &ProcessParams,
        router: &dyn Router,
        voice_idx: usize,
        channel_idx: usize,
    ) {
        let voice = &mut channel.voices[voice_idx];
        let input = router
            .get_input(
                ModuleInput::AmplifierInput(common.module_id),
                voice_idx,
                channel_idx,
                &mut common.input,
            )
            .unwrap_or(&ZEROES_BUFFER);
        let level_mod = router
            .get_input(
                ModuleInput::AmplifierLevel(common.module_id),
                voice_idx,
                channel_idx,
                &mut common.level_mod_input,
            )
            .unwrap_or(&ONES_BUFFER);

        for (out, input, modulation, _) in
            izip!(&mut voice.output, input, level_mod, 0..params.samples)
        {
            *out = input * channel.level * modulation;
        }
    }
}

impl SynthModule for Amplifier {
    fn get_id(&self) -> ModuleId {
        self.common.module_id
    }

    fn note_on(&mut self, _: &NoteOnParams) {}
    fn note_off(&mut self, _: &NoteOffParams) {}

    fn process(&mut self, params: &ProcessParams, router: &dyn Router) {
        for (channel_idx, channel) in self.channels.iter_mut().enumerate() {
            for voice_idx in params.active_voices {
                Self::process_channel_voice(
                    &mut self.common,
                    channel,
                    params,
                    router,
                    *voice_idx,
                    channel_idx,
                );
            }
        }
    }
}

impl BufferOutputModule for Amplifier {
    fn get_output(&self, voice_idx: usize, channel: usize) -> &Buffer {
        &self.channels[channel].voices[voice_idx].output
    }
}
