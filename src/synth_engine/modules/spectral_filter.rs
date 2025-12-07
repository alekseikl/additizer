use std::any::Any;

use serde::{Deserialize, Serialize};

use crate::synth_engine::{
    StereoSample,
    buffer::{SPECTRAL_BUFFER_SIZE, SpectralBuffer},
    routing::{DataType, Input, MAX_VOICES, ModuleId, ModuleType, NUM_CHANNELS, Router},
    synth_module::{
        InputInfo, ModuleConfigBox, NoteOnParams, ProcessParams, SynthModule, VoiceRouter,
    },
    types::{ComplexSample, Sample, SpectralOutput},
};

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct Params {
    four_pole: bool,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ChannelParams {
    cutoff: Sample, //Cutoff octave
    q: Sample,
}

impl Default for ChannelParams {
    fn default() -> Self {
        Self {
            cutoff: 1.0,
            q: 0.7,
        }
    }
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct SpectralFilterConfig {
    label: Option<String>,
    params: Params,
    channels: [ChannelParams; NUM_CHANNELS],
}

pub struct SpectralFilterUIData {
    pub label: String,
    pub cutoff: StereoSample,
    pub q: StereoSample,
    pub four_pole: bool,
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

pub struct SpectralFilter {
    id: ModuleId,
    label: String,
    config: ModuleConfigBox<SpectralFilterConfig>,
    params: Params,
    channels: [Channel; NUM_CHANNELS],
}

impl SpectralFilter {
    pub fn new(id: ModuleId, config: ModuleConfigBox<SpectralFilterConfig>) -> Self {
        let mut filter = Self {
            id,
            label: format!("Filter {id}"),
            config,
            params: Params::default(),
            channels: Default::default(),
        };

        load_module_config!(filter);
        filter
    }

    gen_downcast_methods!();

    pub fn get_ui(&self) -> SpectralFilterUIData {
        SpectralFilterUIData {
            label: self.label.clone(),
            cutoff: get_stereo_param!(self, cutoff),
            q: get_stereo_param!(self, q),
            four_pole: self.params.four_pole,
        }
    }

    set_mono_param!(set_four_pole, four_pole, bool);

    set_stereo_param!(set_cutoff, cutoff, cutoff.clamp(-4.0, 10.0));
    set_stereo_param!(set_q, q, q.clamp(0.1, 10.0));

    fn process_voice(
        four_pole: bool,
        current: bool,
        params: &ChannelParams,
        voice: &mut Voice,
        router: &VoiceRouter,
    ) {
        let spectrum = router.spectral(Input::Spectrum, current);
        let cutoff_mod = router.scalar(Input::Cutoff, current);
        let q_mod = router.scalar(Input::Q, current);

        let range = 1..SPECTRAL_BUFFER_SIZE - 1;
        let input_buff = &spectrum[range.clone()];
        let output_buff = &mut voice.output.advance()[range];
        let cutoff_freq = (params.cutoff + cutoff_mod).exp2();
        let cutoff_squared = cutoff_freq * cutoff_freq;
        let numerator = ComplexSample::new(cutoff_squared, 0.0);
        let q_mult = (params.q + q_mod).clamp(0.1, 10.0).recip();

        for (idx, (out_freq, in_freq)) in output_buff.iter_mut().zip(input_buff).enumerate() {
            let freq = (idx + 1) as Sample;
            let mut filter_response = numerator
                / ComplexSample::new(cutoff_squared - (freq * freq), cutoff_freq * freq * q_mult);

            if four_pole {
                filter_response *= filter_response;
            }

            *out_freq = filter_response * in_freq;
        }
    }
}

impl SynthModule for SpectralFilter {
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
        ModuleType::SpectralFilter
    }

    fn inputs(&self) -> &'static [InputInfo] {
        static INPUTS: &[InputInfo] = &[
            InputInfo::spectral(Input::Spectrum),
            InputInfo::scalar(Input::Cutoff),
            InputInfo::scalar(Input::Q),
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
                    Self::process_voice(self.params.four_pole, false, params, voice, &router);
                    voice.triggered = false;
                }
                Self::process_voice(self.params.four_pole, true, params, voice, &router);
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
