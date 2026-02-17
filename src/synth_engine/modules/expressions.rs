use serde::{Deserialize, Serialize};
use std::any::Any;

use crate::{
    synth_engine::{
        Expression, ModuleId, ModuleType, Sample, SynthModule,
        buffer::{Buffer, zero_buffer},
        routing::{DataType, MAX_VOICES, NUM_CHANNELS, Router},
        smoother::Smoother,
        synth_module::{
            ExpressionParams, InputInfo, ModuleConfigBox, NoteOffParams, NoteOnParams,
            ProcessParams,
        },
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

pub struct ExpressionsUi {
    pub label: String,
    pub expression: Expression,
    pub use_release_velocity: bool,
    pub smooth: Sample,
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
    channels: [Channel; NUM_CHANNELS],
}

impl Expressions {
    pub fn new(id: ModuleId, config: ModuleConfigBox<ExpressionsConfig>) -> Self {
        let mut exp = Self {
            id,
            label: format!("Expressions {id}"),
            config,
            params: Params::default(),
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

    gen_downcast_methods!();

    pub fn get_ui(&self) -> ExpressionsUi {
        ExpressionsUi {
            label: self.label.clone(),
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

    fn inputs(&self) -> &'static [InputInfo] {
        &[]
    }

    fn output(&self) -> DataType {
        DataType::Scalar
    }

    fn note_on(&mut self, params: &NoteOnParams) {
        for (channel_idx, channel) in self.channels.iter_mut().enumerate() {
            let voice = &mut channel.voices[params.voice_idx];

            if matches!(self.params.expression, Expression::Velocity) {
                voice.output = params.velocity;
                voice.audio_smoother.reset(params.velocity);
                voice.triggered = false;
            } else {
                let default_value = Self::default_value(self.params.expression);
                let value =
                    Self::transform_value(self.params.expression, channel_idx, default_value);

                voice.output = value;
                voice.audio_smoother.reset(value);
                voice.triggered = true;
            }
        }
    }

    fn note_off(&mut self, params: &NoteOffParams) {
        if matches!(self.params.expression, Expression::Velocity)
            && self.params.use_release_velocity
        {
            for channel in &mut self.channels {
                let voice = &mut channel.voices[params.voice_idx];

                voice.output = params.velocity;
            }
        }
    }

    fn expression(&mut self, params: &ExpressionParams) {
        if params.expression != self.params.expression {
            return;
        }

        for (channel_idx, channel) in self.channels.iter_mut().enumerate() {
            let voice = &mut channel.voices[params.voice_idx];
            let value = Self::transform_value(self.params.expression, channel_idx, params.value);

            if voice.triggered {
                voice.output = value;
                voice.audio_smoother.reset(value);
                voice.triggered = false;
            } else {
                voice.output = value;
            }
        }
    }

    fn process(&mut self, params: &ProcessParams, _router: &dyn Router) {
        if params.needs_audio_rate {
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
    }

    fn get_buffer_output(&self, voice_idx: usize, channel_idx: usize) -> &Buffer {
        &self.channels[channel_idx].voices[voice_idx].audio_output
    }

    fn get_scalar_output(&self, _current: bool, voice_idx: usize, channel: usize) -> Sample {
        self.channels[channel].voices[voice_idx].output
    }
}
