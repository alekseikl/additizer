use std::sync::Arc;

use nih_plug::params::FloatParam;

mod config;
mod link;
mod ui_bridge;

pub use config::ExternalParamConfig;
use link::{AudioEnd, UiEnd, UiEvent, create_link_pair};
pub use ui_bridge::ExternalParamUiBridge;

use crate::synth_engine::{
    Buffer, ModuleId, Sample,
    buffer::{VoicesLayout, new_voices_layout, zero_buffer},
    routing::{
        ControlRouterType, DataType, InputMeta, ProcessContext, RouterFactory, SamplesOutput,
        VoiceTarget,
    },
    smooth::Smoother,
    synth_module::SynthModule,
};

pub const NUM_FLOAT_PARAMS: usize = 4;

pub struct ExternalParamsBlock {
    pub float_params: [Arc<FloatParam>; NUM_FLOAT_PARAMS],
}

struct Params {
    selected_param_index: usize,
    smooth: Sample,
    sample_on_trigger: bool,
    make_bipolar: bool,
}

impl Params {
    fn from_config(c: &config::ExternalParamConfig) -> Self {
        Self {
            selected_param_index: c.selected_param_index.min(NUM_FLOAT_PARAMS - 1),
            smooth: c.smooth,
            sample_on_trigger: c.sample_on_trigger,
            make_bipolar: c.make_bipolar,
        }
    }
}

struct VoiceState {
    value_at_trigger: Sample,
    smoother: Smoother,
}

impl Default for VoiceState {
    fn default() -> Self {
        Self {
            value_at_trigger: 0.0,
            smoother: Smoother::default(),
        }
    }
}

pub struct ExternalParam {
    id: ModuleId,
    params_block: Arc<ExternalParamsBlock>,
    params: Params,
    audio_end: AudioEnd,
    ui_end: Option<UiEnd>,
    output_slot: usize,
    mono_buff: Buffer,
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
            output_slot: usize::MAX,
            mono_buff: zero_buffer(),
            voices: new_voices_layout(),
        }
    }

    pub fn get_config(&self) -> ExternalParamConfig {
        ExternalParamConfig {
            id: self.id,
            selected_param_index: self.params.selected_param_index,
            smooth: self.params.smooth,
            sample_on_trigger: self.params.sample_on_trigger,
            make_bipolar: self.params.make_bipolar,
        }
    }

    set_mono_param!(
        select_param,
        selected_param_index,
        usize,
        selected_param_index.min(NUM_FLOAT_PARAMS - 1)
    );
    set_mono_param!(set_smooth, smooth, Sample);
    set_mono_param!(set_sample_on_trigger, sample_on_trigger, bool);
    set_mono_param!(set_make_bipolar, make_bipolar, bool);

    fn process_voice(
        &mut self,
        target: &VoiceTarget,
        outputs: &mut VoicesLayout<SamplesOutput>,
        rf: &mut RouterFactory<ControlRouterType>,
    ) {
        let block_samples = rf.params().samples;
        let (router, mut voice_output) = rf.for_voice(target, outputs);
        let voice = &mut self.voices[target.channel_idx][target.voice_idx];
        let sample_rate = router.sample_rate();
        let mono = &self.mono_buff[..block_samples];

        if router.triggered() {
            let idx = router.offset().min(block_samples.saturating_sub(1));
            let value = mono[idx];

            if self.params.sample_on_trigger {
                voice.value_at_trigger = value;
            }

            voice.smoother.reset(value);
        }

        if self.params.sample_on_trigger {
            voice_output.fill_with_ext_control_value(voice.value_at_trigger);
        } else {
            voice_output.fill_with_ext_control(mono);
        }

        voice.smoother.apply_if_needed2(
            sample_rate,
            self.params.smooth,
            voice_output.audio_output(),
        );
    }
}

impl SynthModule for ExternalParam {
    fn id(&self) -> ModuleId {
        self.id
    }

    fn inputs(&self) -> &'static [InputMeta] {
        &[]
    }

    fn output_type(&self) -> DataType {
        DataType::Control
    }

    fn output_slot(&self) -> usize {
        self.output_slot
    }

    fn set_output_slot(&mut self, slot: usize) {
        self.output_slot = slot;
    }

    fn process_ui_events(&mut self) {
        while let Some(event) = self.audio_end.pop_event() {
            match event {
                UiEvent::SelectedParamIndex(index) => self.select_param(index),
                UiEvent::Smooth(value) => self.set_smooth(value),
                UiEvent::SampleOnTrigger(value) => self.set_sample_on_trigger(value),
                UiEvent::MakeBipolar(value) => self.set_make_bipolar(value),
            }
        }
    }

    fn process(&mut self, ctx: &mut ProcessContext) {
        let samples = ctx.params.samples;
        let param = &self.params_block.float_params[self.params.selected_param_index];

        param.smoothed.next_block(&mut self.mono_buff, samples);

        if ctx.params.needs_update_ui {
            self.audio_end.update_value(self.mono_buff[0]);
        }

        if self.params.make_bipolar {
            for sample in &mut self.mono_buff[..samples] {
                *sample = *sample * 2.0 - 1.0;
            }
        }

        ctx.for_control(self.id, self.output_slot, |rf, target, outputs| {
            self.process_voice(target, outputs, rf);
        });
    }
}
