use std::sync::Arc;

use nih_plug::params::FloatParam;

mod config;
mod link;
mod ui_bridge;

pub use config::ExternalParamConfig;
use link::{AudioEnd, UiEnd, UiEvent, create_link_pair};
pub use ui_bridge::ExternalParamUiBridge;

use crate::synth_engine::{
    ModuleId, ModuleType, Sample, StereoSample,
    buffer::{VoicesLayout, new_voices_layout},
    routing::{
        ControlRouterType, DataType, Input, InputSlots, NUM_CHANNELS, ProcessContext,
        SamplesOutput, SpectralInputSlot, VoiceEvent, VoiceRouter,
    },
    smooth::Smoother,
    synth_module::{ModInput, SynthModule},
};

pub const NUM_FLOAT_PARAMS: usize = 4;

pub struct ExternalParamsBlock {
    pub float_params: [Arc<FloatParam>; NUM_FLOAT_PARAMS],
}

struct Params {
    selected_param_index: usize,
    smooth: Sample,
    sample_and_hold: bool,
}

impl Params {
    fn from_config(c: &config::ExternalParamConfig) -> Self {
        Self {
            selected_param_index: c.selected_param_index.min(NUM_FLOAT_PARAMS - 1),
            smooth: c.smooth,
            sample_and_hold: c.sample_and_hold,
        }
    }
}

struct VoiceState {
    triggered: bool,
    value_at_trigger: Sample,
    smoother: Smoother,
}

impl Default for VoiceState {
    fn default() -> Self {
        Self {
            triggered: false,
            value_at_trigger: 0.0,
            smoother: Smoother::default(),
        }
    }
}

type Router<'v, 'f, 'c> = VoiceRouter<'v, 'f, 'c, ControlRouterType>;

pub struct ExternalParam {
    id: ModuleId,
    params_block: Arc<ExternalParamsBlock>,
    params: Params,
    audio_end: AudioEnd,
    ui_end: Option<UiEnd>,
    output_slot: usize,
    voices: VoicesLayout<VoiceState>,
}

impl ExternalParam {
    pub fn new(id: ModuleId, params_block: Arc<ExternalParamsBlock>) -> Self {
        Self::from_config(
            &ExternalParamConfig {
                id,
                ..ExternalParamConfig::default()
            },
            params_block,
        )
    }

    pub fn from_config(
        config: &config::ExternalParamConfig,
        params_block: Arc<ExternalParamsBlock>,
    ) -> Self {
        let (audio_end, ui_end) = create_link_pair();

        Self {
            id: config.id,
            params_block,
            params: Params::from_config(config),
            audio_end,
            ui_end: Some(ui_end),
            output_slot: 0,
            voices: new_voices_layout(),
        }
    }

    pub fn get_config(&self) -> ExternalParamConfig {
        ExternalParamConfig {
            id: self.id,
            selected_param_index: self.params.selected_param_index,
            smooth: self.params.smooth,
            sample_and_hold: self.params.sample_and_hold,
        }
    }

    set_mono_param!(
        select_param,
        selected_param_index,
        usize,
        selected_param_index.min(NUM_FLOAT_PARAMS - 1)
    );
    set_mono_param!(set_smooth, smooth, Sample);
    set_mono_param!(set_sample_and_hold, sample_and_hold, bool);

    fn process_voice(
        &mut self,
        output_slot: &mut VoicesLayout<SamplesOutput>,
        router: Router<'_, '_, '_>,
    ) {
        let channel_idx = router.channel_idx();
        let voice_idx = router.voice_idx();
        let voice = &mut self.voices[channel_idx][voice_idx];
        let samples = router.samples();
        let voice_output = &mut output_slot[channel_idx][voice_idx];

        let param_value = if self.params.sample_and_hold {
            voice.value_at_trigger
        } else {
            self.params_block.float_params[self.params.selected_param_index].value()
        };

        let mut control_output = voice_output.control_output(samples, voice.triggered);
        control_output.output().fill(param_value);
        drop(control_output);

        voice.triggered = false;

        voice.smoother.apply_if_needed(
            samples,
            router.sample_rate(),
            self.params.smooth,
            voice_output.output(samples),
        );
    }
}

impl SynthModule for ExternalParam {
    fn id(&self) -> ModuleId {
        self.id
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::ExternalParam
    }

    fn inputs(&self) -> &'static [ModInput] {
        &[]
    }

    fn output(&self) -> DataType {
        DataType::Control
    }

    fn output_slot(&self) -> usize {
        self.output_slot
    }

    fn set_slots(
        &mut self,
        _inputs: &[InputSlots],
        _spectral_inputs: &[SpectralInputSlot],
        output_slot: usize,
    ) {
        self.output_slot = output_slot;
    }

    fn update_input_amount(&mut self, _input_type: Input, _src_slot: usize, _amount: StereoSample) {
    }

    fn process_events(&mut self, events: &[VoiceEvent]) {
        for channel in self.voices.iter_mut() {
            for event in events {
                if let VoiceEvent::Trigger { voice_idx, .. } = event {
                    let param_value =
                        self.params_block.float_params[self.params.selected_param_index].value();
                    let voice = &mut channel[*voice_idx];

                    voice.triggered = true;
                    voice.value_at_trigger = param_value;
                    voice.smoother.reset(param_value);
                }
            }
        }
    }

    fn process_ui_events(&mut self) {
        while let Some(event) = self.audio_end.pop_event() {
            match event {
                UiEvent::SelectedParamIndex(index) => self.select_param(index),
                UiEvent::Smooth(value) => self.set_smooth(value),
                UiEvent::SampleAndHold(value) => self.set_sample_and_hold(value),
            }
        }
    }

    fn process(&mut self, ctx: &mut ProcessContext) {
        ctx.for_control(self.id, self.output_slot, |router, output| {
            let num_active_voices = router.params().active_voices.len();

            for channel_idx in 0..NUM_CHANNELS {
                for seq_idx in 0..num_active_voices {
                    let voice_idx = router.params().active_voices[seq_idx];

                    self.process_voice(output, router.for_voice(channel_idx, voice_idx, seq_idx));
                }
            }
        });
    }
}
