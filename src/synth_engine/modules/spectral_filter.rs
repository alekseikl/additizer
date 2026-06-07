use std::array;

use itertools::izip;
use nih_plug::util::db_to_gain_fast;

mod config;
mod link;
mod ui_bridge;

pub use config::{SpectralFilterConfig, SpectralFilterType};
use link::{AudioEnd, UiEnd, UiEvent, create_link_pair};
pub use ui_bridge::UiBridge;

use crate::synth_engine::{
    StereoSample,
    biquad_filter::BiquadFilter,
    buffer::SpectralBuffer,
    routing::{
        DataType, Input, MAX_VOICES, ModuleId, ModuleType, NUM_CHANNELS, Router, VoiceEvent,
    },
    synth_module::{ModInput, ProcessParams, SynthModule, VoiceRouter, VoiceRouterFactory},
    types::{ComplexSample, Sample, SpectralOutput},
};

struct Params {
    filter_type: SpectralFilterType,
    fourth_order: bool,
    linear_phase: bool,
}

impl Params {
    fn from_config(c: &config::SpectralFilterConfig) -> Self {
        Self {
            filter_type: c.filter_type,
            fourth_order: c.fourth_order,
            linear_phase: c.linear_phase,
        }
    }
}

struct ChannelParams {
    cutoff: Sample,
    q: Sample,
    drive: Sample,
}

impl ChannelParams {
    fn from_config(c: &SpectralFilterConfig, channel_idx: usize) -> Self {
        Self {
            cutoff: c.cutoff[channel_idx],
            q: c.q[channel_idx],
            drive: c.drive[channel_idx],
        }
    }
}

#[derive(Default)]
struct Voice {
    triggered: bool,
    output: SpectralOutput,
}

type ChannelVoices = [Voice; MAX_VOICES];

pub struct SpectralFilter {
    id: ModuleId,
    params: Params,
    channel_params: [ChannelParams; NUM_CHANNELS],
    audio_end: AudioEnd,
    ui_end: Option<UiEnd>,
    voices: [ChannelVoices; NUM_CHANNELS],
}

impl SpectralFilter {
    pub fn new(id: ModuleId) -> Self {
        Self::from_config(&SpectralFilterConfig {
            id,
            ..SpectralFilterConfig::default()
        })
    }

    pub fn from_config(config: &config::SpectralFilterConfig) -> Self {
        let (audio_end, ui_end) = create_link_pair();

        Self {
            id: config.id,
            params: Params::from_config(config),
            channel_params: array::from_fn(|channel_idx| {
                ChannelParams::from_config(config, channel_idx)
            }),
            audio_end,
            ui_end: Some(ui_end),
            voices: Default::default(),
        }
    }

    pub fn take_ui_end(&mut self) -> Option<UiEnd> {
        self.ui_end.take()
    }

    pub fn return_ui_end(&mut self, ui_end: UiEnd) {
        assert!(self.ui_end.is_none(), "ui_end not taken");
        self.ui_end = Some(ui_end);
    }

    pub fn get_config(&self) -> SpectralFilterConfig {
        SpectralFilterConfig {
            id: self.id,
            filter_type: self.params.filter_type,
            fourth_order: self.params.fourth_order,
            linear_phase: self.params.linear_phase,
            cutoff: get_stereo_param!(self, cutoff),
            q: get_stereo_param!(self, q),
            drive: get_stereo_param!(self, drive),
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

    fn process_voice(&mut self, router: &mut VoiceRouter<'_, '_>) {
        let channel = &self.channel_params[router.channel_idx()];
        let voice = &mut self.voices[router.channel_idx()][router.voice_idx()];
        let current = !voice.triggered;

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
            self.params.filter_type,
            &biquad,
            self.params.fourth_order,
            self.params.linear_phase,
        );

        if voice.triggered {
            voice.triggered = false;

            self.process_voice(router);
        }
    }
}

impl SynthModule for SpectralFilter {
    fn id(&self) -> ModuleId {
        self.id
    }

    fn label(&self) -> String {
        "Filter".into()
    }

    fn set_label(&mut self, _label: String) {}

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
        for channel in &mut self.voices {
            for event in events {
                if let VoiceEvent::Trigger { voice_idx, .. } = event {
                    channel[*voice_idx].triggered = true;
                }
            }
        }
    }

    fn handle_ui_events(&mut self) {
        while let Some(event) = self.audio_end.pop_event() {
            match event {
                UiEvent::InputParam { input, value } => match input {
                    Input::Cutoff => self.set_cutoff(value),
                    Input::Q => self.set_q(value),
                    Input::Drive => self.set_drive(value),
                    _ => (),
                },
                UiEvent::FilterType(filter_type) => self.set_filter_type(filter_type),
                UiEvent::FourthOrder(value) => self.set_fourth_order(value),
                UiEvent::LinearPhase(value) => self.set_linear_phase(value),
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
