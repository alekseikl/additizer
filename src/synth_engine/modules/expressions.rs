mod config;
mod link;
mod ui_bridge;

pub use config::ExpressionsConfig;
use link::{AudioEnd, UiEnd, UiEvent, create_link_pair};
pub use ui_bridge::UiBridge;

use crate::{
    synth_engine::{
        Expression, ModuleId, ModuleType, Sample, SynthModule,
        buffer::{Buffer, zero_buffer},
        routing::{DataType, MAX_VOICES, NUM_CHANNELS, Router, VoiceEvent},
        smooth::Smoother,
        synth_module::{ModInput, ProcessParams},
    },
    utils::st_to_octave,
};

struct Params {
    expression: Expression,
    use_release_velocity: bool,
    smooth: Sample,
}

impl Params {
    fn from_config(c: &config::ExpressionsConfig) -> Self {
        Self {
            expression: c.expression,
            use_release_velocity: c.use_release_velocity,
            smooth: c.smooth,
        }
    }
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

type ChannelVoices = [Voice; MAX_VOICES];

pub struct Expressions {
    id: ModuleId,
    params: Params,
    audio_end: AudioEnd,
    ui_end: Option<UiEnd>,
    voices: [ChannelVoices; NUM_CHANNELS],
}

impl Expressions {
    pub fn new(id: ModuleId) -> Self {
        Self::from_config(&ExpressionsConfig {
            id,
            ..ExpressionsConfig::default()
        })
    }

    pub fn from_config(config: &config::ExpressionsConfig) -> Self {
        let (audio_end, ui_end) = create_link_pair();

        Self {
            id: config.id,
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

    pub fn get_config(&self) -> ExpressionsConfig {
        ExpressionsConfig {
            id: self.id,
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
        "Expr".into()
    }

    fn set_label(&mut self, _label: String) {}

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
        for (channel_idx, channel) in self.voices.iter_mut().enumerate() {
            for event in events {
                match event {
                    VoiceEvent::Trigger {
                        voice_idx,
                        velocity,
                        ..
                    } => {
                        Self::handle_trigger(
                            channel_idx,
                            &mut channel[*voice_idx],
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
                            &mut channel[*voice_idx],
                            &self.params,
                            *velocity,
                        );
                    }
                    VoiceEvent::Release {
                        voice_idx,
                        velocity,
                    } => {
                        Self::handle_release(&mut channel[*voice_idx], &self.params, *velocity);
                    }
                    VoiceEvent::Expression {
                        voice_idx,
                        expression,
                        value,
                    } if *expression == self.params.expression => {
                        Self::handle_expression(
                            channel_idx,
                            &mut channel[*voice_idx],
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
        for channel in &mut self.voices {
            for voice_idx in params.active_voices {
                let voice = &mut channel[*voice_idx];

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
        &self.voices[channel_idx][voice_idx].audio_output
    }

    fn get_scalar_output(&self, _current: bool, voice_idx: usize, channel: usize) -> Sample {
        self.voices[channel][voice_idx].output
    }
}
