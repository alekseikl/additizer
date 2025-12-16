use itertools::izip;
use serde::{Deserialize, Serialize};
use std::any::Any;

use crate::synth_engine::{
    Input, ModuleId, ModuleType, Sample, StereoSample, SynthModule,
    buffer::{SpectralBuffer, zero_spectral_buffer},
    routing::{DataType, MAX_VOICES, NUM_CHANNELS, Router},
    synth_module::{InputInfo, ModuleConfigBox, NoteOnParams, ProcessParams, VoiceRouter},
    types::SpectralOutput,
};

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct ChannelParams {
    blend: Sample,
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct SpectralBlendConfig {
    label: Option<String>,
    channels: [ChannelParams; NUM_CHANNELS],
}

pub struct SpectralBlendUIData {
    pub label: String,
    pub blend: StereoSample,
}

#[derive(Default)]
struct Voice {
    triggered: bool,
    output: SpectralOutput,
}

#[derive(Default)]
struct Channel {
    params: ChannelParams,
    voices: [Voice; MAX_VOICES],
}

struct Buffers {
    input: SpectralBuffer,
    input_to: SpectralBuffer,
}

impl Default for Buffers {
    fn default() -> Self {
        Self {
            input: zero_spectral_buffer(),
            input_to: zero_spectral_buffer(),
        }
    }
}

pub struct SpectralBlend {
    id: ModuleId,
    label: String,
    config: ModuleConfigBox<SpectralBlendConfig>,
    buffers: Buffers,
    channels: [Channel; NUM_CHANNELS],
}

impl SpectralBlend {
    pub fn new(id: ModuleId, config: ModuleConfigBox<SpectralBlendConfig>) -> Self {
        let mut blend = Self {
            id,
            label: format!("Spectral Blend {id}"),
            config,
            buffers: Buffers::default(),
            channels: Default::default(),
        };

        load_module_config_no_params!(blend);
        blend
    }

    gen_downcast_methods!();

    pub fn get_ui(&self) -> SpectralBlendUIData {
        SpectralBlendUIData {
            label: self.label.clone(),
            blend: get_stereo_param!(self, blend),
        }
    }

    set_stereo_param!(set_blend, blend, blend.clamp(0.0, 1.0));

    fn process_voice(
        current: bool,
        params: &ChannelParams,
        buffers: &mut Buffers,
        voice: &mut Voice,
        router: &VoiceRouter,
    ) {
        let spectrum_from = router.spectral(Input::Spectrum, current, &mut buffers.input);
        let spectrum_to = router.spectral(Input::SpectrumTo, current, &mut buffers.input_to);
        let blend = (params.blend + router.scalar(Input::Blend, current)).clamp(0.0, 1.0);
        let output = voice.output.advance();

        for (out, from, to) in izip!(output, spectrum_from, spectrum_to) {
            *out = from + (to - from) * blend;
        }
    }
}

impl SynthModule for SpectralBlend {
    fn id(&self) -> ModuleId {
        self.id
    }

    fn label(&self) -> String {
        self.label.clone()
    }

    fn set_label(&mut self, label: String) {
        self.label = label.clone();
        self.config.lock().label = Some(label);
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::SpectralBlend
    }

    fn inputs(&self) -> &'static [InputInfo] {
        static INPUTS: &[InputInfo] = &[
            InputInfo::spectral(Input::Spectrum),
            InputInfo::spectral(Input::SpectrumTo),
            InputInfo::scalar(Input::Blend),
        ];

        INPUTS
    }

    fn output(&self) -> DataType {
        DataType::Spectral
    }

    fn note_on(&mut self, params: &NoteOnParams) {
        for channel in &mut self.channels {
            channel.voices[params.voice_idx].triggered = true;
        }
    }

    fn process(&mut self, process_params: &ProcessParams, router: &dyn Router) {
        for (channel_idx, channel) in self.channels.iter_mut().enumerate() {
            let params = &channel.params;

            for voice_idx in process_params.active_voices {
                let voice = &mut channel.voices[*voice_idx];
                let router = VoiceRouter {
                    router,
                    module_id: self.id,
                    samples: process_params.samples,
                    voice_idx: *voice_idx,
                    channel_idx,
                };

                if voice.triggered {
                    Self::process_voice(false, params, &mut self.buffers, voice, &router);
                    voice.triggered = false;
                }
                Self::process_voice(true, params, &mut self.buffers, voice, &router);
            }
        }
    }

    fn get_spectral_output(
        &self,
        current: bool,
        voice_idx: usize,
        channel_idx: usize,
    ) -> &SpectralBuffer {
        self.channels[channel_idx].voices[voice_idx]
            .output
            .get(current)
    }
}
