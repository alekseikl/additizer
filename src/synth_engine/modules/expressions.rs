mod config;
mod link;
mod ui_bridge;

pub use config::ExpressionsConfig;
use link::{AudioEnd, UiEnd, UiEvent, create_link_pair};
pub use ui_bridge::ExpressionsUiBridge;

use crate::{
    synth_engine::{
        Buffer, Expression, ModuleId, Sample,
        buffer::{MonoVoicesLayout, VoicesLayout, new_mono_voices_layout, zero_buffer},
        routing::{DataType, InputMeta, ProcessContext, SamplesOutput, VoiceEvent},
        smooth::Smoother,
        synth_module::SynthModule,
    },
    utils::from_st,
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
    change_at: usize,
    buffer: Buffer,
    smoother: Smoother,
}

impl Voice {
    fn set_value_at(&mut self, value: Sample, at: usize) {
        if at > self.change_at {
            let prev = self.buffer[self.change_at];
            self.buffer[self.change_at + 1..at].fill(prev);
        }

        self.buffer[at] = value;
        self.change_at = at;
    }
}

impl Default for Voice {
    fn default() -> Self {
        Self {
            triggered: false,
            change_at: 0,
            buffer: zero_buffer(),
            smoother: Smoother::default(),
        }
    }
}

pub struct Expressions {
    id: ModuleId,
    params: Params,
    audio_end: AudioEnd,
    ui_end: Option<UiEnd>,
    output_slot: usize,
    mono_voices: MonoVoicesLayout<Voice>,
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
            output_slot: usize::MAX,
            mono_voices: new_mono_voices_layout(),
        }
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

    fn transform_value(expression: Expression, value: Sample) -> Sample {
        match expression {
            Expression::Pitch => from_st(value),
            _ => value,
        }
    }

    fn default_value(expression: Expression) -> Sample {
        match expression {
            Expression::Gain => 1.0,
            _ => 0.0,
        }
    }

    fn normalize_display_value(expression: Expression, value: Sample) -> Sample {
        match expression {
            Expression::Pitch => (value.abs() / from_st(12.0)).clamp(0.0, 1.0),
            Expression::Pan => ((value + 1.0) * 0.5).clamp(0.0, 1.0),
            _ => value.abs().clamp(0.0, 1.0),
        }
    }

    fn handle_trigger(voice: &mut Voice, params: &Params, velocity: Sample) {
        let value = if matches!(params.expression, Expression::Velocity) {
            velocity
        } else {
            Self::default_value(params.expression)
        };

        voice.triggered = true;
        voice.set_value_at(value, 0);
        voice.smoother.reset(value);
    }

    fn handle_update(voice: &mut Voice, params: &Params, velocity: Sample) {
        let value = if matches!(params.expression, Expression::Velocity) {
            velocity
        } else {
            Self::default_value(params.expression)
        };

        voice.set_value_at(value, 0);
    }

    fn handle_release(voice: &mut Voice, params: &Params, velocity: Sample) {
        if matches!(params.expression, Expression::Velocity) && params.use_release_velocity {
            voice.set_value_at(velocity, 0);
        }
    }

    fn handle_expression(voice: &mut Voice, expression: Expression, timing: usize, value: Sample) {
        voice.set_value_at(Self::transform_value(expression, value), timing);
    }

    fn process_voice(
        &mut self,
        output_slot: &mut VoicesLayout<SamplesOutput>,
        voice_idx: usize,
        samples: usize,
        sample_rate: Sample,
    ) {
        let voice = &mut self.mono_voices[voice_idx];
        let last_value = voice.buffer[voice.change_at];
        let mono_buff = &mut voice.buffer[..samples];

        if voice.change_at + 1 < samples {
            mono_buff[voice.change_at + 1..samples].fill(last_value);
        }

        voice
            .smoother
            .apply_if_needed(samples, sample_rate, self.params.smooth, mono_buff);

        output_slot[0][voice_idx].fill_with_ext_control(mono_buff);
        output_slot[1][voice_idx].fill_with_ext_control(mono_buff);

        voice.set_value_at(last_value, 0);
        voice.triggered = false;
    }
}

impl SynthModule for Expressions {
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

    fn process_events(&mut self, events: &[VoiceEvent]) {
        let voices = &mut self.mono_voices;

        for event in events {
            match event {
                VoiceEvent::Trigger {
                    voice_idx,
                    velocity,
                    ..
                } => {
                    Self::handle_trigger(&mut voices[*voice_idx], &self.params, *velocity);
                }
                VoiceEvent::Update {
                    voice_idx,
                    velocity,
                    ..
                } => {
                    Self::handle_update(&mut voices[*voice_idx], &self.params, *velocity);
                }
                VoiceEvent::Release {
                    voice_idx,
                    velocity,
                } => {
                    Self::handle_release(&mut voices[*voice_idx], &self.params, *velocity);
                }
                VoiceEvent::Expression {
                    voice_idx,
                    expression,
                    timing,
                    value,
                } if *expression == self.params.expression => {
                    Self::handle_expression(&mut voices[*voice_idx], *expression, *timing, *value);
                }
                _ => (),
            }
        }
    }

    fn process_ui_events(&mut self) {
        while let Some(event) = self.audio_end.pop_event() {
            match event {
                UiEvent::Expression(expression) => self.set_expression(expression),
                UiEvent::UseReleaseVelocity(value) => self.set_use_release_velocity(value),
                UiEvent::Smooth(value) => self.set_smooth(value),
            }
        }
    }

    fn process(&mut self, ctx: &mut ProcessContext) {
        ctx.for_control(self.id, self.output_slot, |router, output| {
            for playing_voice in router.params().active_voices.iter() {
                self.process_voice(
                    output,
                    playing_voice.voice_idx(),
                    router.params().samples,
                    router.params().sample_rate,
                );
            }

            if router.params().needs_update_ui {
                let display_value = router
                    .params()
                    .active_voices
                    .first()
                    .map(|v| self.mono_voices[v.voice_idx()].buffer[0])
                    .unwrap_or(0.0);
                self.audio_end.update_value(Self::normalize_display_value(
                    self.params.expression,
                    display_value,
                ));
            }
        });
    }
}
