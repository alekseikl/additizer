use std::array;

mod config;
mod link;
mod ui_bridge;

pub use config::SpectralFilterConfig;
use link::{AudioEnd, UiEnd, UiEvent, create_link_pair};
pub use ui_bridge::SpectralFilterUiBridge;

use crate::synth_engine::{
    StereoSample,
    buffer::{VoicesLayout, new_voices_layout},
    filters::spectral_filter::{
        FilterParams, FilterType, MAX_RESONANCE, MIN_RESONANCE,
        SpectralFilter as SpectralFilterEngine,
    },
    routing::{
        DataType, Input, InputMeta, InputSlots, ModuleId, NUM_CHANNELS, ProcessContext,
        SpectralInputSlot, SpectralOutput, SpectralRouterType, VoiceEvent, VoiceRouter,
    },
    synth_module::SynthModule,
    types::{ComplexSample, Sample},
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
}

impl ChannelParams {
    fn from_config(c: &SpectralFilterConfig, channel_idx: usize) -> Self {
        Self {
            cutoff: c.cutoff[channel_idx],
            resonance: c.resonance[channel_idx],
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
    resonance: InputSlots,
    drive: InputSlots,
}

impl Default for Inputs {
    fn default() -> Self {
        Self {
            spectrum: None,
            cutoff: InputSlots::empty(Input::Cutoff),
            resonance: InputSlots::empty(Input::Resonance),
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

type Router<'v, 'f, 'c> = VoiceRouter<'v, 'f, 'c, SpectralRouterType>;

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
            output_slot: usize::MAX,
            voices: new_voices_layout(),
        }
    }

    pub fn get_config(&self) -> SpectralFilterConfig {
        SpectralFilterConfig {
            id: self.id,
            filter_type: self.params.filter_type,
            linear_phase: self.params.linear_phase,
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

    fn apply_filter(
        filter_type: FilterType,
        params: &Params,
        cutoff: Sample,
        resonance: Sample,
        drive: Sample,
        input: &[ComplexSample],
        output: &mut [ComplexSample],
    ) {
        let filter = SpectralFilterEngine::new(
            filter_type,
            FilterParams {
                drive,
                cutoff,
                resonance,
                linear_phase: params.linear_phase,
            },
        );

        filter.apply_response(input, output);
    }

    fn process_voice(
        &mut self,
        output: &mut VoicesLayout<SpectralOutput>,
        mut router: Router<'_, '_, '_>,
    ) {
        let channel_idx = router.channel_idx();
        let voice_idx = router.voice_idx();
        let inputs = &self.inputs;
        let channel = &self.channel_params[channel_idx];
        let voice = &mut self.voices[channel_idx][voice_idx];
        let voice_output = output[channel_idx][voice_idx].advance();

        let cutoff = router
            .scalar_param(&inputs.cutoff, channel.cutoff, voice.triggered)
            .clamp(-4.0, 10.0);
        let resonance = router
            .scalar_param(&inputs.resonance, channel.resonance, voice.triggered)
            .clamp(MIN_RESONANCE, MAX_RESONANCE);
        let drive = router
            .scalar_param(&inputs.drive, channel.drive, voice.triggered)
            .min(24.0);
        let input = router.spectral(inputs.spectrum, voice.triggered);

        Self::apply_filter(
            self.params.filter_type,
            &self.params,
            cutoff,
            resonance,
            drive,
            input,
            voice_output,
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

    fn process_events(&mut self, events: &[VoiceEvent]) {
        for channel in self.voices.iter_mut() {
            for event in events {
                if let VoiceEvent::Trigger { voice_idx, .. } = event {
                    channel[*voice_idx].triggered = true;
                }
            }
        }
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
            }
        }
    }

    fn process(&mut self, ctx: &mut ProcessContext) {
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
