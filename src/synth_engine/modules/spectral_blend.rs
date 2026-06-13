use std::array;

use itertools::izip;

mod config;
mod link;
mod ui_bridge;

pub use config::SpectralBlendConfig;
use link::{AudioEnd, UiEnd, UiEvent, create_link_pair};
pub use ui_bridge::SpectralBlendUiBridge;

use crate::synth_engine::{
    Input, ModuleId, ModuleType, Sample, StereoSample, SynthModule,
    buffer::{SpectralBuffer, new_voices_layout},
    routing::{DataType, MAX_VOICES, NUM_CHANNELS, Router, VoiceEvent},
    synth_module::{ModInput, ProcessParams, VoiceRouter, VoiceRouterFactory},
    types::SpectralOutput,
};

struct ChannelParams {
    blend: Sample,
}

impl ChannelParams {
    fn from_config(c: &config::SpectralBlendConfig, channel_idx: usize) -> Self {
        Self {
            blend: c.blend[channel_idx],
        }
    }
}

#[derive(Default)]
struct Voice {
    triggered: bool,
    output: SpectralOutput,
}

type ChannelVoices = [Voice; MAX_VOICES];

pub struct SpectralBlend {
    id: ModuleId,
    channel_params: [ChannelParams; NUM_CHANNELS],
    audio_end: AudioEnd,
    ui_end: Option<UiEnd>,
    voices: Box<[ChannelVoices; NUM_CHANNELS]>,
}

impl SpectralBlend {
    pub fn new(id: ModuleId) -> Self {
        Self::from_config(&SpectralBlendConfig {
            id,
            ..SpectralBlendConfig::default()
        })
    }

    pub fn from_config(config: &config::SpectralBlendConfig) -> Self {
        let (audio_end, ui_end) = create_link_pair();

        Self {
            id: config.id,
            channel_params: array::from_fn(|channel_idx| {
                ChannelParams::from_config(config, channel_idx)
            }),
            audio_end,
            ui_end: Some(ui_end),
            voices: new_voices_layout(),
        }
    }

    pub fn get_config(&self) -> SpectralBlendConfig {
        SpectralBlendConfig {
            id: self.id,
            blend: get_stereo_param!(self, blend),
        }
    }

    set_stereo_param!(set_blend, blend, blend.clamp(0.0, 1.0));

    fn process_voice(&mut self, router: &mut VoiceRouter<'_, '_>) {
        let channel = &self.channel_params[router.channel_idx()];
        let voice = &mut self.voices[router.channel_idx()][router.voice_idx()];
        let current = !voice.triggered;

        let blend = router
            .scalar(Input::Blend, channel.blend, current)
            .clamp(0.0, 1.0);
        let spectrum_from = router.spectral(Input::Spectrum, current);
        let spectrum_to = router.spectral(Input::SpectrumTo, current);
        let output = voice.output.advance();

        for (out, from, to) in izip!(output, spectrum_from, spectrum_to) {
            *out = from + (to - from) * blend;
        }

        if voice.triggered {
            voice.triggered = false;

            self.process_voice(router);
        }
    }
}

impl SynthModule for SpectralBlend {
    fn id(&self) -> ModuleId {
        self.id
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::SpectralBlend
    }

    fn inputs(&self) -> &'static [ModInput] {
        static INPUTS: &[ModInput] = &[
            ModInput::spectral(Input::Spectrum),
            ModInput::spectral(Input::SpectrumTo),
            ModInput::control(Input::Blend),
        ];

        INPUTS
    }

    fn output(&self) -> DataType {
        DataType::Spectral
    }

    fn handle_events(&mut self, events: &[VoiceEvent]) {
        for channel in self.voices.iter_mut() {
            for event in events {
                if let VoiceEvent::Trigger { voice_idx, .. } = event {
                    channel[*voice_idx].triggered = true;
                }
            }
        }
    }

    fn handle_ui_events(&mut self) {
        while let Some(event) = self.audio_end.pop_event() {
            if let UiEvent::InputParam {
                input: Input::Blend,
                value,
            } = event
            {
                self.set_blend(value);
            }
        }
    }

    fn process(&mut self, process_params: &ProcessParams, router: &mut dyn Router) {
        let mut rf = VoiceRouterFactory::new(self.id, router, process_params);

        for channel_idx in (0..NUM_CHANNELS).take(process_params.spectrum_channels) {
            for (seq_idx, voice_idx) in process_params.active_voices.iter().enumerate() {
                let mut voice_router = rf.for_voice(*voice_idx, channel_idx, seq_idx);

                self.process_voice(&mut voice_router);
            }
        }
    }

    fn get_spectral_output(
        &self,
        current: bool,
        voice_idx: usize,
        channel_idx: usize,
    ) -> &SpectralBuffer {
        self.voices[channel_idx][voice_idx].output.get(current)
    }
}
