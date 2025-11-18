use std::any::Any;

use serde::{Deserialize, Serialize};

use crate::synth_engine::{
    buffer::{
        SPECTRAL_BUFFER_SIZE, SpectralBuffer, ZEROES_SPECTRAL_BUFFER, make_zero_spectral_buffer,
    },
    routing::{
        InputType, MAX_VOICES, ModuleId, ModuleInput, ModuleType, NUM_CHANNELS, OutputType, Router,
    },
    synth_module::{ModuleConfigBox, ProcessParams, SynthModule},
    types::{ComplexSample, Sample, StereoSample},
};

#[derive(Clone, Serialize, Deserialize)]
pub struct SpectralFilterConfigChannel {
    cutoff: Sample,
    q: Sample,
}

impl Default for SpectralFilterConfigChannel {
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
    channels: [SpectralFilterConfigChannel; NUM_CHANNELS],
    four_pole: bool,
}

pub struct SpectralFilterUIData {
    pub label: String,
    pub cutoff: StereoSample,
    pub q: StereoSample,
    pub four_pole: bool,
}

struct Voice {
    output: SpectralBuffer,
}

impl Default for Voice {
    fn default() -> Self {
        Self {
            output: make_zero_spectral_buffer(),
        }
    }
}

struct ChannelParams {
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

#[derive(Default)]
struct Channel {
    params: ChannelParams,
    voices: [Voice; MAX_VOICES],
}

pub struct SpectralFilter {
    id: ModuleId,
    label: String,
    config: ModuleConfigBox<SpectralFilterConfig>,
    four_pole: bool,
    channels: [Channel; NUM_CHANNELS],
}

macro_rules! set_param_method {
    ($fn_name:ident, $param:ident, $transform:expr) => {
        pub fn $fn_name(&mut self, $param: StereoSample) {
            for (channel, $param) in self.channels.iter_mut().zip($param.iter()) {
                channel.params.$param = $transform;
            }

            {
                let mut cfg = self.config.lock();
                for (config_channel, channel) in cfg.channels.iter_mut().zip(self.channels.iter()) {
                    config_channel.$param = channel.params.$param;
                }
            }
        }
    };
}

macro_rules! extract_param {
    ($self:ident, $param:ident) => {
        StereoSample::from_iter($self.channels.iter().map(|channel| channel.params.$param))
    };
}

impl SpectralFilter {
    pub fn new(id: ModuleId, config: ModuleConfigBox<SpectralFilterConfig>) -> Self {
        let mut filter = Self {
            id,
            label: format!("Filter {id}"),
            config,
            four_pole: false,
            channels: Default::default(),
        };

        {
            let cfg = filter.config.lock();

            if let Some(label) = cfg.label.as_ref() {
                filter.label = label.clone();
            }

            for (channel, cfg_channel) in filter.channels.iter_mut().zip(cfg.channels.iter()) {
                channel.params.cutoff = cfg_channel.cutoff;
                channel.params.q = cfg_channel.q;
            }

            filter.four_pole = cfg.four_pole;
        }

        filter
    }

    gen_downcast_methods!(SpectralFilter);

    pub fn get_ui(&self) -> SpectralFilterUIData {
        SpectralFilterUIData {
            label: self.label.clone(),
            cutoff: extract_param!(self, cutoff),
            q: extract_param!(self, q),
            four_pole: self.four_pole,
        }
    }

    pub fn set_four_pole(&mut self, four_pole: bool) {
        self.four_pole = four_pole;
        self.config.lock().four_pole = four_pole;
    }

    set_param_method!(set_cutoff, cutoff, cutoff.clamp(-4.0, 10.0));
    set_param_method!(set_q, q, q.clamp(0.1, 10.0));

    fn process_channel_voice(
        module_id: ModuleId,
        four_pole: bool,
        channel: &mut Channel,
        router: &dyn Router,
        voice_idx: usize,
        channel_idx: usize,
    ) {
        let voice = &mut channel.voices[voice_idx];
        let spectrum = router
            .get_spectral_input(ModuleInput::spectrum(module_id), voice_idx, channel_idx)
            .unwrap_or(&ZEROES_SPECTRAL_BUFFER);
        let cutoff_mod = router
            .get_scalar_input(ModuleInput::cutoff(module_id), voice_idx, channel_idx)
            .unwrap_or(0.0);
        let q_mod = router
            .get_scalar_input(ModuleInput::q(module_id), voice_idx, channel_idx)
            .unwrap_or(0.0);

        let range = 1..SPECTRAL_BUFFER_SIZE - 1;
        let input_buff = &spectrum[range.clone()];
        let output_buff = &mut voice.output[range];
        let cutoff_freq = (channel.params.cutoff + cutoff_mod).exp2();
        let cutoff_squared = cutoff_freq * cutoff_freq;
        let numerator = ComplexSample::new(cutoff_squared, 0.0);
        let q_mult = (channel.params.q + q_mod).clamp(0.1, 10.0).recip();

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

    fn is_spectral_rate(&self) -> bool {
        true
    }

    fn inputs(&self) -> &'static [InputType] {
        &[InputType::Spectrum, InputType::Cutoff, InputType::Q]
    }

    fn output_type(&self) -> OutputType {
        OutputType::Spectrum
    }

    fn process(&mut self, params: &ProcessParams, router: &dyn Router) {
        for (channel_idx, channel) in self.channels.iter_mut().enumerate() {
            for voice_idx in params.active_voices {
                Self::process_channel_voice(
                    self.id,
                    self.four_pole,
                    channel,
                    router,
                    *voice_idx,
                    channel_idx,
                );
            }
        }
    }

    fn get_spectral_output(&self, voice_idx: usize, channel: usize) -> &SpectralBuffer {
        let voice = &self.channels[channel].voices[voice_idx];

        &voice.output
    }
}
