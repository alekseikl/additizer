use serde::{Deserialize, Serialize};

mod link;
mod ui_bridge;

use link::{AudioEnd, UiEnd, UiEvent, create_link_pair};
pub use ui_bridge::{ControlsState, UiBridge};

use crate::{
    synth_engine::{
        Expression, ModuleId, ModuleType, Sample, SynthModule,
        buffer::{Buffer, zero_buffer},
        routing::{DataType, MAX_VOICES, NUM_CHANNELS, Router, VoiceEvent},
        smooth::Smoother,
        synth_module::{ModInput, ModuleConfigBox, ProcessParams},
    },
    utils::{from_ms, st_to_octave},
};

#[derive(Clone, Serialize, Deserialize)]
pub struct Params {
    expression: Expression,
    use_release_velocity: bool,
    smooth: Sample,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            expression: Expression::Velocity,
            use_release_velocity: false,
            smooth: from_ms(4.0),
        }
    }
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct ExpressionsConfig {
    label: Option<String>,
    params: Params,
}

struct Voice {
    triggered: bool,
    output: Sample,
    audio_smoother: Smoother,
    audio_output: Buffer,
}

impl Default for Voice {
    fn default() -> Self {
        Self {
            triggered: false,
            output: 0.0,
            audio_smoother: Smoother::default(),
            audio_output: zero_buffer(),
        }
    }
}

#[derive(Default)]
struct Channel {
    voices: [Voice; MAX_VOICES],
}

pub struct Expressions {
    id: ModuleId,
    label: String,
    config: ModuleConfigBox<ExpressionsConfig>,
    params: Params,
    audio_end: AudioEnd,
    ui_end: Option<UiEnd>,
    channels: [Channel; NUM_CHANNELS],
}

impl Expressions {
    pub fn new(id: ModuleId, config: ModuleConfigBox<ExpressionsConfig>) -> Self {
        let (audio_end, ui_end) = create_link_pair();

        let mut exp = Self {
            id,
            label: format!("Expressions {id}"),
            config,
            params: Params::default(),
            audio_end,
            ui_end: Some(ui_end),
            channels: Default::default(),
        };

        {
            let cfg = exp.config.lock();

            if let Some(label) = cfg.label.as_ref() {
                exp.label = label.clone();
            }
            exp.params = cfg.params.clone();
        }

        exp
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
            expression: self.params.expression,
            use_release_velocity: self.params.use_release_velocity,
            smooth: self.params.smooth,
        }
    }

    set_mono_param!(set_expression, expression, Expression);
    set_mono_param!(set_use_release_velocity, use_release_velocity, bool);
    set_mono_param!(set_smooth, smooth, Sample);

    fn transform_value(expression: Expression, channel_idx: usize, value: Sample) -> Sample {
        match expression {
            Expression::Pitch => st_to_octave(value),
            Expression::Pan => {
                if channel_idx == 0 {
                    if value > 0.0 { 1.0 - value } else { 1.0 }
                } else if value < 0.0 {
                    1.0 + value
                } else {
                    1.0
                }
            }
            _ => value,
        }
    }

    fn default_value(expression: Expression) -> Sample {
        match expression {
            Expression::Gain => 1.0,
            _ => 0.0,
        }
    }

    fn handle_trigger(channel_idx: usize, voice: &mut Voice, params: &Params, velocity: Sample) {
        if matches!(params.expression, Expression::Velocity) {
            voice.output = velocity;
            voice.audio_smoother.reset(velocity);
            voice.triggered = false;
        } else {
            let default_value = Self::default_value(params.expression);
            let value = Self::transform_value(params.expression, channel_idx, default_value);

            voice.output = value;
            voice.audio_smoother.reset(value);
            voice.triggered = true;
        }
    }

    fn handle_update(channel_idx: usize, voice: &mut Voice, params: &Params, velocity: Sample) {
        if matches!(params.expression, Expression::Velocity) {
            voice.output = velocity;
        } else {
            let default_value = Self::default_value(params.expression);
            let value = Self::transform_value(params.expression, channel_idx, default_value);

            voice.output = value;
        }
    }

    fn handle_release(voice: &mut Voice, params: &Params, velocity: Sample) {
        if matches!(params.expression, Expression::Velocity) && params.use_release_velocity {
            voice.output = velocity;
        }
    }

    fn handle_expression(
        channel_idx: usize,
        voice: &mut Voice,
        expression: Expression,
        value: Sample,
    ) {
        let value = Self::transform_value(expression, channel_idx, value);

        if voice.triggered {
            voice.output = value;
            voice.audio_smoother.reset(value);
            voice.triggered = false;
        } else {
            voice.output = value;
        }
    }
}

impl SynthModule for Expressions {
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
        ModuleType::Expressions
    }

    fn inputs(&self) -> &'static [ModInput] {
        &[]
    }

    fn output(&self) -> DataType {
        DataType::Scalar
    }

    fn handle_events(&mut self, events: &[VoiceEvent]) {
        for (channel_idx, channel) in self.channels.iter_mut().enumerate() {
            for event in events {
                match event {
                    VoiceEvent::Trigger {
                        voice_idx,
                        velocity,
                        ..
                    } => {
                        Self::handle_trigger(
                            channel_idx,
                            &mut channel.voices[*voice_idx],
                            &self.params,
                            *velocity,
                        );
                    }
                    VoiceEvent::Update {
                        voice_idx,
                        velocity,
                        ..
                    } => {
                        Self::handle_update(
                            channel_idx,
                            &mut channel.voices[*voice_idx],
                            &self.params,
                            *velocity,
                        );
                    }
                    VoiceEvent::Release {
                        voice_idx,
                        velocity,
                    } => {
                        Self::handle_release(
                            &mut channel.voices[*voice_idx],
                            &self.params,
                            *velocity,
                        );
                    }
                    VoiceEvent::Expression {
                        voice_idx,
                        expression,
                        value,
                    } if *expression == self.params.expression => {
                        Self::handle_expression(
                            channel_idx,
                            &mut channel.voices[*voice_idx],
                            *expression,
                            *value,
                        );
                    }
                    _ => (),
                }
            }
        }
    }

    fn handle_ui_events(&mut self) {
        while let Some(event) = self.audio_end.pop_event() {
            match event {
                UiEvent::Expression(expression) => self.set_expression(expression),
                UiEvent::UseReleaseVelocity(value) => self.set_use_release_velocity(value),
                UiEvent::Smooth(value) => self.set_smooth(value),
            }
        }
    }

    fn process(&mut self, params: &ProcessParams, _router: &mut dyn Router) {
        for channel in &mut self.channels {
            for voice_idx in params.active_voices {
                let voice = &mut channel.voices[*voice_idx];

                voice
                    .audio_smoother
                    .update(params.sample_rate, self.params.smooth);

                for out in voice.audio_output.iter_mut().take(params.samples) {
                    *out = voice.audio_smoother.tick(voice.output);
                }
            }
        }
    }

    fn get_buffer_output(&self, voice_idx: usize, channel_idx: usize) -> &Buffer {
        &self.channels[channel_idx].voices[voice_idx].audio_output
    }

    fn get_scalar_output(&self, _current: bool, voice_idx: usize, channel: usize) -> Sample {
        self.channels[channel].voices[voice_idx].output
    }
}
