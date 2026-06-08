use std::sync::Arc;

use nih_plug::params::FloatParam;

mod config;
mod link;
mod ui_bridge;

pub use config::ExternalParamConfig;
use link::{AudioEnd, UiEnd, UiEvent, create_link_pair};
pub use ui_bridge::UiBridge;

use crate::synth_engine::{
    ModuleId, ModuleType, Sample, SynthModule,
    buffer::{Buffer, zero_buffer},
    routing::{DataType, MAX_VOICES, NUM_CHANNELS, Router, VoiceEvent},
    smooth::Smoother,
    synth_module::{ModInput, ProcessParams},
    types::ScalarOutput,
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

struct Voice {
    triggered: bool,
    value_at_trigger: Sample,
    output: ScalarOutput,
    audio_smoother: Smoother,
    audio_output: Buffer,
}

impl Default for Voice {
    fn default() -> Self {
        Self {
            triggered: false,
            value_at_trigger: 0.0,
            output: ScalarOutput::default(),
            audio_smoother: Smoother::default(),
            audio_output: zero_buffer(),
        }
    }
}

type ChannelVoices = [Voice; MAX_VOICES];

pub struct ExternalParam {
    id: ModuleId,
    params_block: Arc<ExternalParamsBlock>,
    params: Params,
    audio_end: AudioEnd,
    ui_end: Option<UiEnd>,
    voices: [ChannelVoices; NUM_CHANNELS],
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
        DataType::Scalar
    }

    fn handle_events(&mut self, events: &[VoiceEvent]) {
        for channel in &mut self.voices {
            for event in events {
                if let VoiceEvent::Trigger { voice_idx, .. } = event {
                    let param_value =
                        self.params_block.float_params[self.params.selected_param_index].value();
                    let voice = &mut channel[*voice_idx];

                    voice.triggered = true;
                    voice.value_at_trigger = param_value;
                    voice.audio_smoother.reset(param_value);
                }
            }
        }
    }

    fn handle_ui_events(&mut self) {
        while let Some(event) = self.audio_end.pop_event() {
            match event {
                UiEvent::SelectedParamIndex(index) => self.select_param(index),
                UiEvent::Smooth(value) => self.set_smooth(value),
                UiEvent::SampleAndHold(value) => self.set_sample_and_hold(value),
            }
        }
    }

    fn process(&mut self, params: &ProcessParams, _router: &mut dyn Router) {
        let param_value = self.params_block.float_params[self.params.selected_param_index].value();

        for channel in self.voices.iter_mut() {
            for voice_idx in params.active_voices {
                let voice = &mut channel[*voice_idx];
                let param_value = if self.params.sample_and_hold {
                    voice.value_at_trigger
                } else {
                    param_value
                };

                if voice.triggered {
                    voice.output.advance(param_value);
                    voice.triggered = false;
                }

                voice.output.advance(param_value);

                voice
                    .audio_smoother
                    .update(params.sample_rate, self.params.smooth);
                voice.audio_smoother.segment(
                    &voice.output,
                    params.samples,
                    &mut voice.audio_output,
                );
            }
        }
    }

    fn get_buffer_output(&self, voice_idx: usize, channel_idx: usize) -> &Buffer {
        &self.voices[channel_idx][voice_idx].audio_output
    }

    fn get_scalar_output(&self, current: bool, voice_idx: usize, channel: usize) -> Sample {
        self.voices[channel][voice_idx].output.get(current)
    }
}
