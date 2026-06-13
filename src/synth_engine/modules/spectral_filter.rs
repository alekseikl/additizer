use std::array;

use itertools::izip;
use nih_plug::util::db_to_gain_fast;

mod config;
mod link;
mod ui_bridge;

pub use config::{SpectralFilterConfig, SpectralFilterType};
use link::{AudioEnd, UiEnd, UiEvent, create_link_pair};
pub use ui_bridge::SpectralFilterUiBridge;

use crate::synth_engine::{
    StereoSample,
    biquad_filter::BiquadFilter,
    buffer::{SpectralBuffer, VoicesLayout, new_voices_layout},
    outputs_arena::{self, InputSlots, ProcessContext, SpectralInputSlot, SpectralOutputSlot},
    routing::{DataType, Input, ModuleId, ModuleType, NUM_CHANNELS, VoiceEvent},
    synth_module::{ModInput, SynthModule},
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
struct VoiceState {
    triggered: bool,
}

pub struct Inputs {
    spectrum: Option<usize>,
    cutoff: InputSlots,
    q: InputSlots,
    drive: InputSlots,
}

impl Default for Inputs {
    fn default() -> Self {
        Self {
            spectrum: None,
            cutoff: InputSlots::empty(Input::Cutoff),
            q: InputSlots::empty(Input::Q),
            drive: InputSlots::empty(Input::Drive),
        }
    }
}

impl Inputs {
    fn from_slots(inputs: &[InputSlots], spectral_inputs: &[SpectralInputSlot]) -> Self {
        let mut result = Self::default();

        for input in inputs {
            match input.input_type {
                Input::Cutoff => result.cutoff = input.clone(),
                Input::Q => result.q = input.clone(),
                Input::Drive => result.drive = input.clone(),
                _ => (),
            }
        }

        for input in spectral_inputs {
            if matches!(input.input_type, Input::Spectrum) {
                result.spectrum = Some(input.slot);
            }
        }

        result
    }
}

type VoiceRouter<'v, 'f, 'c> = outputs_arena::VoiceRouter<'v, 'f, 'c, SpectralOutputSlot>;

pub struct SpectralFilter {
    id: ModuleId,
    params: Params,
    channel_params: [ChannelParams; NUM_CHANNELS],
    audio_end: AudioEnd,
    ui_end: Option<UiEnd>,
    inputs: Inputs,
    output_slot: usize,
    voices: VoicesLayout<VoiceState>,
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
            inputs: Inputs::default(),
            output_slot: 0,
            voices: new_voices_layout(),
        }
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

    fn process_voice(
        &mut self,
        output: &mut VoicesLayout<SpectralOutput>,
        mut router: VoiceRouter<'_, '_, '_>,
    ) {
        let channel_idx = router.channel_idx();
        let voice_idx = router.voice_idx();
        let inputs = &self.inputs;
        let channel = &self.channel_params[channel_idx];
        let voice = &mut self.voices[channel_idx][voice_idx];
        let voice_output = output[channel_idx][voice_idx].advance();
        let current = !voice.triggered;

        let cutoff = router
            .scalar_param(&inputs.cutoff, channel.cutoff, current)
            .clamp(-4.0, 10.0);
        let q = router
            .scalar_param(&inputs.q, channel.q, current)
            .clamp(0.1, 10.0);
        let drive = router
            .scalar_param(&inputs.drive, channel.drive, current)
            .min(24.0);
        let input = router.spectral(inputs.spectrum, current);

        let biquad = BiquadFilter::new(db_to_gain_fast(drive), cutoff.exp2(), q);

        Self::apply_biquad(
            voice_output,
            input,
            self.params.filter_type,
            &biquad,
            self.params.fourth_order,
            self.params.linear_phase,
        );

        if voice.triggered {
            voice.triggered = false;

            self.process_voice(output, router);
        }
    }
}

impl SynthModule for SpectralFilter {
    fn id(&self) -> ModuleId {
        self.id
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::SpectralFilter
    }

    fn inputs(&self) -> &'static [ModInput] {
        static INPUTS: &[ModInput] = &[
            ModInput::spectral(Input::Spectrum),
            ModInput::control(Input::Cutoff),
            ModInput::control(Input::Q),
            ModInput::control(Input::Drive),
        ];

        INPUTS
    }

    fn output(&self) -> DataType {
        DataType::Spectral
    }

    fn set_slots(
        &mut self,
        inputs: &[InputSlots],
        spectral_inputs: &[SpectralInputSlot],
        output_slot: usize,
    ) {
        self.inputs = Inputs::from_slots(inputs, spectral_inputs);
        self.output_slot = output_slot;
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

    fn process2(&mut self, ctx: &mut ProcessContext) {
        ctx.for_spectral(self.id, self.output_slot, |router, output| {
            let num_active_voices = router.params().active_voices.len();
            let spectrum_channels = router.params().spectrum_channels;

            for channel_idx in 0..spectrum_channels {
                for seq_idx in 0..num_active_voices {
                    let voice_idx = router.params().active_voices[seq_idx];

                    self.process_voice(output, router.for_voice(channel_idx, voice_idx, seq_idx));
                }
            }
        });
    }
}
