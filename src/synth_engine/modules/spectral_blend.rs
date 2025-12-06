use itertools::izip;
use serde::{Deserialize, Serialize};
use std::any::Any;

use crate::synth_engine::{
    Input, ModuleId, ModuleType, Sample, StereoSample, SynthModule,
    buffer::{SPECTRAL_BUFFER_SIZE, SpectralBuffer},
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

pub struct SpectralBlend {
    id: ModuleId,
    label: String,
    config: ModuleConfigBox<SpectralBlendConfig>,
    channels: [Channel; NUM_CHANNELS],
}

impl SpectralBlend {
    pub fn new(id: ModuleId, config: ModuleConfigBox<SpectralBlendConfig>) -> Self {
        let mut blend = Self {
            id,
            label: format!("Spectral Blend {id}"),
            config,
            channels: Default::default(),
        };

        {
            let cfg = blend.config.lock();

            if let Some(label) = cfg.label.as_ref() {
                blend.label = label.clone();
            }

            for (channel, cfg_channel) in blend.channels.iter_mut().zip(cfg.channels.iter()) {
                channel.params = cfg_channel.clone();
            }
        }

        blend
    }

    gen_downcast_methods!();

    pub fn get_ui(&self) -> SpectralBlendUIData {
        SpectralBlendUIData {
            label: self.label.clone(),
            blend: extract_module_param!(self, blend),
        }
    }

    set_module_param_method!(set_blend, blend, blend.clamp(0.0, 1.0));

    fn process_voice(
        current: bool,
        params: &ChannelParams,
        voice: &mut Voice,
        router: &VoiceRouter,
    ) {
        let spectrum_from = router.spectral(Input::Spectrum, current);
        let spectrum_to = router.spectral(Input::SpectrumTo, current);
        let blend = (params.blend + router.scalar(Input::Blend, current)).clamp(0.0, 1.0);
        let output = &mut voice.output.advance()[..SPECTRAL_BUFFER_SIZE - 1];

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

    fn outputs(&self) -> &'static [DataType] {
        &[DataType::Spectral]
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
                    Self::process_voice(false, params, voice, &router);
                    voice.triggered = false;
                }
                Self::process_voice(true, params, voice, &router);
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
