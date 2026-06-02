use itertools::izip;
use nih_plug::util::db_to_gain_fast;
use serde::{Deserialize, Serialize};

use crate::synth_engine::{
    StereoSample,
    biquad_filter::BiquadFilter,
    buffer::SpectralBuffer,
    routing::{
        DataType, Input, MAX_VOICES, ModuleId, ModuleType, NUM_CHANNELS, Router, VoiceEvent,
    },
    synth_module::{
        ModInput, ModuleConfigBox, ProcessParams, SynthModule, VoiceRouter, VoiceRouterFactory,
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

#[derive(Clone, Serialize, Deserialize)]
pub struct Params {
    filter_type: SpectralFilterType,
    fourth_order: bool,
    linear_phase: bool,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            filter_type: SpectralFilterType::default(),
            fourth_order: false,
            linear_phase: true,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ChannelParams {
    cutoff: Sample, //Cutoff octave
    q: Sample,
    drive: Sample,
}

impl Default for ChannelParams {
    fn default() -> Self {
        Self {
            cutoff: 1.0,
            q: 0.7,
            drive: 0.0,
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
    pub filter_type: SpectralFilterType,
    pub cutoff: StereoSample,
    pub q: StereoSample,
    pub drive: StereoSample,
    pub fourth_order: bool,
    pub linear_phase: bool,
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

    pub fn get_ui(&self) -> SpectralFilterUIData {
        SpectralFilterUIData {
            filter_type: self.params.filter_type,
            cutoff: get_stereo_param!(self, cutoff),
            q: get_stereo_param!(self, q),
            drive: get_stereo_param!(self, drive),
            fourth_order: self.params.fourth_order,
            linear_phase: self.params.linear_phase,
        }
    }

    set_mono_param!(set_filter_type, filter_type, SpectralFilterType);
    set_mono_param!(set_fourth_order, fourth_order, bool);
    set_mono_param!(set_linear_phase, linear_phase, bool);

    set_stereo_param!(set_cutoff, cutoff, cutoff.clamp(-4.0, 10.0));
    set_stereo_param!(set_q, q, q.clamp(0.1, 10.0));
    set_stereo_param!(set_drive, drive);

    fn apply_response(
        output: &mut SpectralBuffer,
        input: &SpectralBuffer,
        response: impl Iterator<Item = ComplexSample>,
        fourth_order: bool,
        linear_phase: bool,
    ) {
        fn apply(
            output: &mut SpectralBuffer,
            input: &SpectralBuffer,
            response: impl Iterator<Item = ComplexSample>,
            transform: impl Fn(ComplexSample, ComplexSample) -> ComplexSample,
        ) {
            for (out, input, response) in izip!(output, input, response) {
                *out = transform(*input, response);
            }
        }

        if linear_phase {
            if fourth_order {
                apply(output, input, response, |input, response| {
                    let magnitude = response.norm();

                    input * (magnitude * magnitude)
                });
            } else {
                apply(output, input, response, |i, r| i * r.norm());
            }
        } else if fourth_order {
            apply(output, input, response, |i, r| i * r * r);
        } else {
            apply(output, input, response, |i, r| i * r);
        }
    }

    fn apply_biquad(
        output: &mut SpectralBuffer,
        input: &SpectralBuffer,
        filter_type: SpectralFilterType,
        biquad: &BiquadFilter,
        fourth_order: bool,
        linear_phase: bool,
    ) {
        match filter_type {
            SpectralFilterType::LowPass => {
                Self::apply_response(output, input, biquad.low_pass(), fourth_order, linear_phase)
            }
            SpectralFilterType::HighPass => Self::apply_response(
                output,
                input,
                biquad.high_pass(),
                fourth_order,
                linear_phase,
            ),
            SpectralFilterType::BandPass => Self::apply_response(
                output,
                input,
                biquad.band_pass(),
                fourth_order,
                linear_phase,
            ),
            SpectralFilterType::BandStop => Self::apply_response(
                output,
                input,
                biquad.band_stop(),
                fourth_order,
                linear_phase,
            ),
            SpectralFilterType::Peaking => {
                Self::apply_response(output, input, biquad.peaking(), fourth_order, linear_phase)
            }
        }
    }

    fn process_voice(
        current: bool,
        params: &Params,
        channel: &ChannelParams,
        voice: &mut Voice,
        router: &mut VoiceRouter<'_, '_>,
    ) {
        let cutoff = router
            .scalar(Input::Cutoff, channel.cutoff, current)
            .clamp(-4.0, 10.0);
        let q = router.scalar(Input::Q, channel.q, current).clamp(0.1, 10.0);
        let drive = router
            .scalar(Input::Drive, channel.drive, current)
            .min(24.0);
        let input = router.spectral(Input::Spectrum, current);

        let biquad = BiquadFilter::new(db_to_gain_fast(drive), cutoff.exp2(), q);

        Self::apply_biquad(
            voice.output.advance(),
            input,
            params.filter_type,
            &biquad,
            params.fourth_order,
            params.linear_phase,
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

    fn inputs(&self) -> &'static [ModInput] {
        static INPUTS: &[ModInput] = &[
            ModInput::spectral(Input::Spectrum),
            ModInput::scalar(Input::Cutoff),
            ModInput::scalar(Input::Q),
            ModInput::scalar(Input::Drive),
        ];

        INPUTS
    }

    fn output(&self) -> DataType {
        DataType::Spectral
    }

    fn handle_events(&mut self, events: &[VoiceEvent]) {
        for channel in &mut self.channels {
            for event in events {
                if let VoiceEvent::Trigger { voice_idx, .. } = event {
                    channel.voices[*voice_idx].triggered = true;
                }
            }
        }
    }

    fn process(&mut self, process_params: &ProcessParams, router: &mut dyn Router) {
        let mut rf = VoiceRouterFactory::new(self.id, router, process_params);

        for (channel_idx, channel) in self
            .channels
            .iter_mut()
            .enumerate()
            .take(process_params.spectrum_channels)
        {
            for (seq_idx, voice_idx) in process_params.active_voices.iter().enumerate() {
                let voice = &mut channel.voices[*voice_idx];
                let mut voice_router = rf.for_voice(*voice_idx, channel_idx, seq_idx);

                if voice.triggered {
                    Self::process_voice(
                        false,
                        &self.params,
                        &channel.params,
                        voice,
                        &mut voice_router,
                    );
                    voice.triggered = false;
                }
                Self::process_voice(
                    true,
                    &self.params,
                    &channel.params,
                    voice,
                    &mut voice_router,
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
