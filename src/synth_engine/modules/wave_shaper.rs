use std::array;

use itertools::izip;
use nih_plug::util::db_to_gain_fast;

mod config;
mod link;
mod ui_bridge;

pub use config::{WaveShaperConfig, ShaperType};
use link::{AudioEnd, UiEnd, UiEvent, create_link_pair};
pub use ui_bridge::WaveShaperUiBridge;

use crate::synth_engine::{
    Input, ModuleId, ModuleType, Sample, StereoSample, SynthModule,
    buffer::{Buffer, zero_buffer},
    routing::{DataType, MAX_VOICES, NUM_CHANNELS, Router},
    synth_module::{ModInput, ProcessParams, VoiceRouter, VoiceRouterFactory},
};

struct Params {
    shaper_type: ShaperType,
}

impl Params {
    fn from_config(c: &config::WaveShaperConfig) -> Self {
        Self {
            shaper_type: c.shaper_type,
        }
    }
}

struct ChannelParams {
    distortion: Sample,
    clipping_level: Sample,
}

impl ChannelParams {
    fn from_config(c: &WaveShaperConfig, channel_idx: usize) -> Self {
        Self {
            distortion: c.distortion[channel_idx],
            clipping_level: c.clipping_level[channel_idx],
        }
    }
}

struct Voice {
    output: Buffer,
}

impl Default for Voice {
    fn default() -> Self {
        Self {
            output: zero_buffer(),
        }
    }
}

struct Buffers {
    input: Buffer,
    distortion_mod_input: Buffer,
    clipping_level_mod_input: Buffer,
}

type ChannelVoices = [Voice; MAX_VOICES];

pub struct WaveShaper {
    id: ModuleId,
    params: Params,
    channel_params: [ChannelParams; NUM_CHANNELS],
    buffers: Buffers,
    audio_end: AudioEnd,
    ui_end: Option<UiEnd>,
    voices: [ChannelVoices; NUM_CHANNELS],
}

impl WaveShaper {
    pub fn new(id: ModuleId) -> Self {
        Self::from_config(&WaveShaperConfig {
            id,
            ..WaveShaperConfig::default()
        })
    }

    pub fn from_config(config: &config::WaveShaperConfig) -> Self {
        let (audio_end, ui_end) = create_link_pair();

        Self {
            id: config.id,
            params: Params::from_config(config),
            channel_params: array::from_fn(|channel_idx| {
                ChannelParams::from_config(config, channel_idx)
            }),
            buffers: Buffers {
                input: zero_buffer(),
                distortion_mod_input: zero_buffer(),
                clipping_level_mod_input: zero_buffer(),
            },
            audio_end,
            ui_end: Some(ui_end),
            voices: Default::default(),
        }
    }

    pub fn get_config(&self) -> WaveShaperConfig {
        WaveShaperConfig {
            id: self.id,
            shaper_type: self.params.shaper_type,
            distortion: get_stereo_param!(self, distortion),
            clipping_level: get_stereo_param!(self, clipping_level),
        }
    }

    set_mono_param!(set_shaper_type, shaper_type, ShaperType);

    set_stereo_param!(set_distortion, distortion);
    set_stereo_param!(set_clipping_level, clipping_level);

    fn process_channel_voice(&mut self, router: VoiceRouter<'_, '_>) {
        let channel = &self.channel_params[router.channel_idx()];
        let voice = &mut self.voices[router.channel_idx()][router.voice_idx()];
        let input = router.buffer(Input::Audio, &mut self.buffers.input);
        let clipping_level_mod = router.buffer(
            Input::ClippingLevel,
            &mut self.buffers.clipping_level_mod_input,
        );
        let distortion_mod =
            router.buffer(Input::Distortion, &mut self.buffers.distortion_mod_input);

        for (out, input, clipping_level_mod, distortion_mod) in izip!(
            voice.output.iter_mut().take(router.samples()),
            input,
            clipping_level_mod,
            distortion_mod
        ) {
            let clipping_gain =
                db_to_gain_fast((channel.clipping_level + clipping_level_mod).min(24.0));
            let gain = db_to_gain_fast((channel.distortion + distortion_mod).clamp(0.0, 48.0));

            match self.params.shaper_type {
                ShaperType::HardClip => {
                    *out = (input * gain).clamp(-clipping_gain, clipping_gain);
                }
                ShaperType::Sigmoid => *out = clipping_gain * (input * gain / clipping_gain).tanh(),
            }
        }
    }
}

impl SynthModule for WaveShaper {
    fn id(&self) -> ModuleId {
        self.id
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::WaveShaper
    }

    fn inputs(&self) -> &'static [ModInput] {
        static INPUTS: &[ModInput] = &[
            ModInput::buffer(Input::Audio),
            ModInput::buffer(Input::ClippingLevel),
            ModInput::buffer(Input::Distortion),
        ];

        INPUTS
    }

    fn output(&self) -> DataType {
        DataType::Buffer
    }

    fn handle_ui_events(&mut self) {
        while let Some(event) = self.audio_end.pop_event() {
            match event {
                UiEvent::InputParam { input, value } => match input {
                    Input::Distortion => self.set_distortion(value),
                    Input::ClippingLevel => self.set_clipping_level(value),
                    _ => (),
                },
                UiEvent::ShaperType(shaper_type) => self.set_shaper_type(shaper_type),
            }
        }
    }

    fn process(&mut self, process_params: &ProcessParams, router: &mut dyn Router) {
        let mut rf = VoiceRouterFactory::new(self.id, router, process_params);

        for channel_idx in 0..NUM_CHANNELS {
            for (seq_idx, voice_idx) in process_params.active_voices.iter().enumerate() {
                self.process_channel_voice(rf.for_voice(*voice_idx, channel_idx, seq_idx));
            }
        }
    }

    fn get_buffer_output(&self, voice_idx: usize, channel: usize) -> &Buffer {
        &self.voices[channel][voice_idx].output
    }
}
