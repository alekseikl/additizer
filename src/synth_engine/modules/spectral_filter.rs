use itertools::izip;
use serde::{Deserialize, Serialize};
use std::any::Any;

use crate::synth_engine::{
    StereoSample,
    biquad_filter::BiquadFilter,
    buffer::{SpectralBuffer, zero_spectral_buffer},
    routing::{DataType, Input, MAX_VOICES, ModuleId, ModuleType, NUM_CHANNELS, Router},
    synth_module::{
        InputInfo, ModuleConfigBox, NoteOnParams, ProcessParams, SynthModule, VoiceRouter,
    },
    types::{ComplexSample, Sample, SpectralOutput},
};

#[derive(Default, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum SpectralFilterType {
    #[default]
    LowPass,
    HighPass,
    BandPass,
    BandStop,
    Peaking,
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct Params {
    filter_type: SpectralFilterType,
    fourth_order: bool,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ChannelParams {
    cutoff: Sample, //Cutoff octave
    q: Sample,
    gain: Sample,
}

impl Default for ChannelParams {
    fn default() -> Self {
        Self {
            cutoff: 1.0,
            q: 0.7,
            gain: 1.0,
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
    pub filter_type: SpectralFilterType,
    pub cutoff: StereoSample,
    pub q: StereoSample,
    pub gain: StereoSample,
    pub fourth_order: bool,
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
    input_buffer: SpectralBuffer,
    channels: [Channel; NUM_CHANNELS],
}

impl SpectralFilter {
    pub fn new(id: ModuleId, config: ModuleConfigBox<SpectralFilterConfig>) -> Self {
        let mut filter = Self {
            id,
            label: format!("Filter {id}"),
            config,
            params: Params::default(),
            input_buffer: zero_spectral_buffer(),
            channels: Default::default(),
        };

        load_module_config!(filter);
        filter
    }

    gen_downcast_methods!();

    pub fn get_ui(&self) -> SpectralFilterUIData {
        SpectralFilterUIData {
            label: self.label.clone(),
            filter_type: self.params.filter_type,
            cutoff: get_stereo_param!(self, cutoff),
            q: get_stereo_param!(self, q),
            gain: get_stereo_param!(self, gain),
            fourth_order: self.params.fourth_order,
        }
    }

    set_mono_param!(set_filter_type, filter_type, SpectralFilterType);
    set_mono_param!(set_fourth_order, fourth_order, bool);

    set_stereo_param!(set_cutoff, cutoff, cutoff.clamp(-4.0, 10.0));
    set_stereo_param!(set_q, q, q.clamp(0.1, 10.0));
    set_stereo_param!(set_gain, gain, *gain);

    fn apply_response(
        output: &mut SpectralBuffer,
        input: &SpectralBuffer,
        response: impl Iterator<Item = ComplexSample>,
        fourth_order: bool,
    ) {
        if fourth_order {
            for (out, input, response) in izip!(output, input, response) {
                *out = input * response * response;
            }
        } else {
            for (out, input, response) in izip!(output, input, response) {
                *out = input * response;
            }
        }
    }

    fn apply_biquad(
        output: &mut SpectralBuffer,
        input: &SpectralBuffer,
        filter_type: SpectralFilterType,
        biquad: &BiquadFilter,
        fourth_order: bool,
    ) {
        match filter_type {
            SpectralFilterType::LowPass => {
                Self::apply_response(output, input, biquad.low_pass(), fourth_order)
            }
            SpectralFilterType::HighPass => {
                Self::apply_response(output, input, biquad.high_pass(), fourth_order)
            }
            SpectralFilterType::BandPass => {
                Self::apply_response(output, input, biquad.band_pass(), fourth_order)
            }
            SpectralFilterType::BandStop => {
                Self::apply_response(output, input, biquad.band_stop(), fourth_order)
            }
            SpectralFilterType::Peaking => {
                Self::apply_response(output, input, biquad.peaking(), fourth_order)
            }
        }
    }

    fn process_voice(
        current: bool,
        params: &Params,
        channel: &ChannelParams,
        input_buffer: &mut SpectralBuffer,
        voice: &mut Voice,
        router: &VoiceRouter,
    ) {
        let input = router.spectral(Input::Spectrum, current, input_buffer);
        let cutoff = (channel.cutoff + router.scalar(Input::Cutoff, current)).clamp(-4.0, 10.0);
        let q = (channel.q + router.scalar(Input::Q, current)).clamp(0.1, 10.0);
        let gain = (channel.gain + router.scalar(Input::Level, current)).min(24.0);

        let biquad = BiquadFilter::new(gain, cutoff.exp2(), q);

        Self::apply_biquad(
            voice.output.advance(),
            input,
            params.filter_type,
            &biquad,
            params.fourth_order,
        );
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
                    Self::process_voice(
                        false,
                        &self.params,
                        &channel.params,
                        &mut self.input_buffer,
                        voice,
                        &router,
                    );
                    voice.triggered = false;
                }
                Self::process_voice(
                    true,
                    &self.params,
                    &channel.params,
                    &mut self.input_buffer,
                    voice,
                    &router,
                );
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
