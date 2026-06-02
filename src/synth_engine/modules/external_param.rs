use std::sync::Arc;

use nih_plug::params::FloatParam;
use serde::{Deserialize, Serialize};

mod link;
mod ui_bridge;

use link::{AudioEnd, UiEnd, UiEvent, create_link_pair};
pub use ui_bridge::{ControlsState, UiBridge};

use crate::{
    synth_engine::{
        ModuleId, ModuleType, Sample, SynthModule,
        buffer::{Buffer, zero_buffer},
        routing::{DataType, MAX_VOICES, NUM_CHANNELS, Router, VoiceEvent},
        smooth::Smoother,
        synth_module::{ModInput, ModuleConfigBox, ProcessParams},
        types::ScalarOutput,
    },
    utils::from_ms,
};

pub const NUM_FLOAT_PARAMS: usize = 4;

pub struct ExternalParamsBlock {
    pub float_params: [Arc<FloatParam>; NUM_FLOAT_PARAMS],
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Params {
    selected_param_index: usize,
    smooth: Sample,
    sample_and_hold: bool,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            selected_param_index: 0,
            smooth: from_ms(2.0),
            sample_and_hold: false,
        }
    }
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct ExternalParamConfig {
    label: Option<String>,
    params: Params,
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

#[derive(Default)]
struct Channel {
    voices: [Voice; MAX_VOICES],
}

pub struct ExternalParam {
    id: ModuleId,
    label: String,
    config: ModuleConfigBox<ExternalParamConfig>,
    params_block: Arc<ExternalParamsBlock>,
    params: Params,
    audio_end: AudioEnd,
    ui_end: Option<UiEnd>,
    channels: [Channel; NUM_CHANNELS],
}

impl ExternalParam {
    pub fn new(
        id: ModuleId,
        config: ModuleConfigBox<ExternalParamConfig>,
        params_block: Arc<ExternalParamsBlock>,
    ) -> Self {
        let (audio_end, ui_end) = create_link_pair();

        let mut ext = Self {
            id,
            label: format!("External Param {id}"),
            config,
            params_block,
            params: Params::default(),
            audio_end,
            ui_end: Some(ui_end),
            channels: Default::default(),
        };

        {
            let cfg = ext.config.lock();

            if let Some(label) = cfg.label.as_ref() {
                ext.label = label.clone();
            }
            ext.params = cfg.params.clone();
        }

        ext.params.selected_param_index = ext.params.selected_param_index.min(NUM_FLOAT_PARAMS - 1);

        ext
    }

    pub fn take_ui_end(&mut self) -> Option<UiEnd> {
        self.ui_end.take()
    }

    pub fn return_ui_end(&mut self, ui_end: UiEnd) {
        assert!(self.ui_end.is_none(), "ui_end not taken");
        self.ui_end = Some(ui_end);
    }

    pub fn get_controls_state(&self) -> ControlsState {
        ControlsState {
            selected_param_index: self.params.selected_param_index,
            num_of_params: NUM_FLOAT_PARAMS,
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

    fn label(&self) -> String {
        self.label.clone()
    }

    fn set_label(&mut self, label: String) {
        self.label = label.clone();
        self.config.lock().label = Some(label);
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
        for channel in &mut self.channels {
            for event in events {
                if let VoiceEvent::Trigger { voice_idx, .. } = event {
                    let param_value =
                        self.params_block.float_params[self.params.selected_param_index].value();
                    let voice = &mut channel.voices[*voice_idx];

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

        for channel in self.channels.iter_mut() {
            for voice_idx in params.active_voices {
                let voice = &mut channel.voices[*voice_idx];
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
        &self.channels[channel_idx].voices[voice_idx].audio_output
    }

    fn get_scalar_output(&self, current: bool, voice_idx: usize, channel: usize) -> Sample {
        self.channels[channel].voices[voice_idx].output.get(current)
    }
}
