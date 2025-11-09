use std::any::Any;

use crate::synth_engine::{
    buffer::{Buffer, ONES_BUFFER, ZEROES_BUFFER, make_zero_buffer},
    routing::{
        InputType, MAX_VOICES, ModuleId, ModuleInput, ModuleType, NUM_CHANNELS, OutputType, Router,
    },
    synth_module::{ModuleConfigBox, ProcessParams, SynthModule},
    types::{Sample, StereoSample},
};
use itertools::izip;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct AmplifierConfigChannel {
    level: Sample,
}

impl Default for AmplifierConfigChannel {
    fn default() -> Self {
        Self { level: 1.0 }
    }
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct AmplifierConfig {
    channels: [AmplifierConfigChannel; NUM_CHANNELS],
}

pub struct AmplifierUI {
    pub level: StereoSample,
}

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
    id: ModuleId,
    config: ModuleConfigBox<AmplifierConfig>,
    input: Buffer,
    level_mod_input: Buffer,
}

pub struct Amplifier {
    common: Common,
    channels: [Channel; NUM_CHANNELS],
}

impl Amplifier {
    pub fn new(id: ModuleId, config: ModuleConfigBox<AmplifierConfig>) -> Self {
        let mut amp = Self {
            common: Common {
                id,
                config,
                input: make_zero_buffer(),
                level_mod_input: make_zero_buffer(),
            },
            channels: Default::default(),
        };

        {
            let cfg = amp.common.config.lock();

            for (channel, cfg) in amp.channels.iter_mut().zip(cfg.channels.iter()) {
                channel.level = cfg.level;
            }
        }

        amp
    }

    gen_downcast_methods!(Amplifier);

    pub fn get_ui(&self) -> AmplifierUI {
        AmplifierUI {
            level: StereoSample::from_iter(self.channels.iter().map(|channel| channel.level)),
        }
    }

    pub fn set_level(&mut self, level: StereoSample) {
        for (chan, value) in self.channels.iter_mut().zip(level.iter()) {
            chan.level = *value;
        }

        {
            let mut cfg = self.common.config.lock();

            for (chan, value) in cfg.channels.iter_mut().zip(level.iter()) {
                chan.level = *value;
            }
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
        let id = common.id;
        let voice = &mut channel.voices[voice_idx];
        let input = router
            .get_input(
                ModuleInput::input(id),
                params.samples,
                voice_idx,
                channel_idx,
                &mut common.input,
            )
            .unwrap_or(&ZEROES_BUFFER);
        let level_mod = router
            .get_input(
                ModuleInput::level(id),
                params.samples,
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
    fn id(&self) -> ModuleId {
        self.common.id
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::Amplifier
    }

    fn is_spectral_rate(&self) -> bool {
        false
    }

    fn inputs(&self) -> &'static [InputType] {
        &[InputType::Input, InputType::Level]
    }

    fn output_type(&self) -> OutputType {
        OutputType::Output
    }

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

    fn get_buffer_output(&self, voice_idx: usize, channel: usize) -> &Buffer {
        &self.channels[channel].voices[voice_idx].output
    }
}
