use std::array;

mod config;
mod link;
mod ui_bridge;

pub use config::SpectralFilterConfig;
use link::{AudioEnd, UiEnd, UiEvent, create_link_pair};
pub use ui_bridge::SpectralFilterUiBridge;

use crate::synth_engine::{
    StereoSample,
    buffer::VoicesLayout,
    filters::spectral_filter::{
        FilterParams, FilterType, MAX_RESONANCE, MIN_RESONANCE,
        SpectralFilter as SpectralFilterEngine,
    },
    routing::{
        DataType, Input, InputMeta, InputSlots, ModuleId, NUM_CHANNELS, ProcessContext,
        RouterFactory, SpectralInputSlot, SpectralOutput, SpectralRouterType, VoiceTarget,
    },
    synth_module::SynthModule,
    types::Sample,
};

struct Params {
    filter_type: FilterType,
    linear_phase: bool,
}

impl Params {
    fn from_config(c: &config::SpectralFilterConfig) -> Self {
        Self {
            filter_type: c.filter_type,
            linear_phase: c.linear_phase,
        }
    }
}

struct ChannelParams {
    cutoff: Sample,
    resonance: Sample,
    drive: Sample,
    q_limit_to: Sample,
    q_limit_curve: Sample,
}

impl ChannelParams {
    fn from_config(c: &SpectralFilterConfig, channel_idx: usize) -> Self {
        Self {
            cutoff: c.cutoff[channel_idx],
            resonance: c.resonance[channel_idx],
            drive: c.drive[channel_idx],
            q_limit_to: c.q_limit_to[channel_idx],
            q_limit_curve: c.q_limit_curve[channel_idx],
        }
    }
}

pub struct Inputs {
    spectrum: Option<usize>,
    cutoff: InputSlots,
    resonance: InputSlots,
    drive: InputSlots,
}

impl Default for Inputs {
    fn default() -> Self {
        Self {
            spectrum: None,
            cutoff: InputSlots::new(Input::Cutoff),
            resonance: InputSlots::new(Input::Resonance),
            drive: InputSlots::new(Input::Drive),
        }
    }
}

impl Inputs {
    fn from_slots(inputs: &[InputSlots], spectral_inputs: &[SpectralInputSlot]) -> Self {
        let mut result = Self::default();

        for input in inputs {
            match input.input_type {
                Input::Cutoff => result.cutoff = input.clone(),
                Input::Resonance => result.resonance = input.clone(),
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

    fn update_amount(&mut self, input_type: Input, src_slot: usize, amount: StereoSample) {
        match input_type {
            Input::Cutoff => self.cutoff.update_amount(src_slot, amount),
            Input::Resonance => self.resonance.update_amount(src_slot, amount),
            Input::Drive => self.drive.update_amount(src_slot, amount),
            _ => (),
        }
    }
}

pub struct SpectralFilter {
    id: ModuleId,
    params: Params,
    channel_params: [ChannelParams; NUM_CHANNELS],
    audio_end: AudioEnd,
    ui_end: Option<UiEnd>,
    inputs: Inputs,
    output_slot: usize,
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
            output_slot: usize::MAX,
        }
    }

    pub fn get_config(&self) -> SpectralFilterConfig {
        SpectralFilterConfig {
            id: self.id,
            filter_type: self.params.filter_type,
            linear_phase: self.params.linear_phase,
            q_limit_to: get_stereo_param!(self, q_limit_to),
            q_limit_curve: get_stereo_param!(self, q_limit_curve),
            cutoff: get_stereo_param!(self, cutoff),
            resonance: get_stereo_param!(self, resonance),
            drive: get_stereo_param!(self, drive),
        }
    }

    set_mono_param!(set_filter_type, filter_type, FilterType);
    set_mono_param!(set_linear_phase, linear_phase, bool);

    set_stereo_param!(set_cutoff, cutoff, cutoff.clamp(-4.0, 10.0));
    set_stereo_param!(
        set_resonance,
        resonance,
        resonance.clamp(MIN_RESONANCE, MAX_RESONANCE)
    );
    set_stereo_param!(set_drive, drive);
    set_stereo_param!(set_q_limit_to, q_limit_to, q_limit_to.clamp(0.0, 10.0));
    set_stereo_param!(
        set_q_limit_curve,
        q_limit_curve,
        q_limit_curve.clamp(0.0, 1.0)
    );

    fn process_voice(
        &mut self,
        target: &VoiceTarget,
        outputs: &mut VoicesLayout<SpectralOutput>,
        rf: &mut RouterFactory<SpectralRouterType>,
    ) {
        let (mut router, mut voice_output) = rf.for_voice(target, outputs);
        let inputs = &self.inputs;
        let channel = &self.channel_params[target.channel_idx];

        let cutoff = router
            .scalar(&inputs.cutoff, channel.cutoff)
            .clamp(-4.0, 10.0);
        let resonance = router
            .scalar(&inputs.resonance, channel.resonance)
            .clamp(MIN_RESONANCE, MAX_RESONANCE);
        let drive = router.scalar(&inputs.drive, channel.drive).min(24.0);
        let input = router.spectral(inputs.spectrum);

        let filter = SpectralFilterEngine::new(
            self.params.filter_type,
            FilterParams {
                drive,
                cutoff,
                resonance,
                q_limit_to: channel.q_limit_to,
                q_limit_curve: channel.q_limit_curve,
                linear_phase: self.params.linear_phase,
            },
        );

        filter.apply_response(input, voice_output.output());
    }
}

impl SynthModule for SpectralFilter {
    fn id(&self) -> ModuleId {
        self.id
    }

    fn inputs(&self) -> &'static [InputMeta] {
        static INPUTS: &[InputMeta] = &[
            InputMeta::spectral(Input::Spectrum),
            InputMeta::control(Input::Cutoff),
            InputMeta::control(Input::Resonance),
            InputMeta::control(Input::Drive),
        ];

        INPUTS
    }

    fn output_type(&self) -> DataType {
        DataType::Spectral
    }

    fn output_slot(&self) -> usize {
        self.output_slot
    }

    fn set_output_slot(&mut self, slot: usize) {
        self.output_slot = slot;
    }

    fn set_input_slots(&mut self, inputs: &[InputSlots], spectral_inputs: &[SpectralInputSlot]) {
        self.inputs = Inputs::from_slots(inputs, spectral_inputs);
    }

    fn update_input_amount(&mut self, input_type: Input, src_slot: usize, amount: StereoSample) {
        self.inputs.update_amount(input_type, src_slot, amount);
    }

    fn process_ui_events(&mut self) {
        while let Some(event) = self.audio_end.pop_event() {
            match event {
                UiEvent::InputParam { input, value } => match input {
                    Input::Cutoff => self.set_cutoff(value),
                    Input::Resonance => self.set_resonance(value),
                    Input::Drive => self.set_drive(value),
                    _ => (),
                },
                UiEvent::FilterType(filter_type) => self.set_filter_type(filter_type),
                UiEvent::LinearPhase(value) => self.set_linear_phase(value),
                UiEvent::QLimitTo(value) => self.set_q_limit_to(value),
                UiEvent::QLimitCurve(value) => self.set_q_limit_curve(value),
            }
        }
    }

    fn process(&mut self, ctx: &mut ProcessContext) {
        ctx.spectral(self.id, self.output_slot)
            .for_voices(|rf, target, outputs| {
                self.process_voice(target, outputs, rf);
            });
    }
}
