mod config;
mod link;
mod ui_bridge;

pub use config::ExpressionsConfig;
use link::{AudioEnd, UiEnd, UiEvent, create_link_pair};
pub use ui_bridge::ExpressionsUiBridge;

use crate::{
    synth_engine::{
        Expression, Input, ModuleId, ModuleType, Sample, StereoSample,
        buffer::{VoicesLayout, new_voices_layout},
        routing::{
            ControlRouterType, DataType, InputSlots, NUM_CHANNELS, ProcessContext, SpectralInputSlot,
            VoiceEvent, VoiceRouter,
        },
        smooth::Smoother,
        synth_module::{ModInput, SynthModule},
        types::SamplesOutput,
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

struct VoiceState {
    triggered: bool,
    value: Sample,
    smoother: Smoother,
}

impl Default for VoiceState {
    fn default() -> Self {
        Self {
            triggered: false,
            value: 0.0,
            smoother: Smoother::default(),
        }
    }
}

type Router<'v, 'f, 'c> = VoiceRouter<'v, 'f, 'c, ControlRouterType>;

pub struct Expressions {
    id: ModuleId,
    params: Params,
    audio_end: AudioEnd,
    ui_end: Option<UiEnd>,
    output_slot: usize,
    voices: VoicesLayout<VoiceState>,
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
            output_slot: 0,
            voices: new_voices_layout(),
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

    fn handle_trigger(
        channel_idx: usize,
        voice: &mut VoiceState,
        params: &Params,
        velocity: Sample,
    ) {
        if matches!(params.expression, Expression::Velocity) {
            voice.value = velocity;
            voice.smoother.reset(velocity);
            voice.triggered = false;
        } else {
            let default_value = Self::default_value(params.expression);
            let value = Self::transform_value(params.expression, channel_idx, default_value);

            voice.value = value;
            voice.smoother.reset(value);
            voice.triggered = true;
        }
    }

    fn handle_update(
        channel_idx: usize,
        voice: &mut VoiceState,
        params: &Params,
        velocity: Sample,
    ) {
        if matches!(params.expression, Expression::Velocity) {
            voice.value = velocity;
        } else {
            let default_value = Self::default_value(params.expression);
            let value = Self::transform_value(params.expression, channel_idx, default_value);

            voice.value = value;
        }
    }

    fn handle_release(voice: &mut VoiceState, params: &Params, velocity: Sample) {
        if matches!(params.expression, Expression::Velocity) && params.use_release_velocity {
            voice.value = velocity;
        }
    }

    fn handle_expression(
        channel_idx: usize,
        voice: &mut VoiceState,
        expression: Expression,
        value: Sample,
    ) {
        let value = Self::transform_value(expression, channel_idx, value);

        if voice.triggered {
            voice.value = value;
            voice.smoother.reset(value);
            voice.triggered = false;
        } else {
            voice.value = value;
        }
    }

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
        let triggered = voice.triggered;

        let mut control_output = voice_output.control_output(samples, triggered);
        control_output.output().fill(voice.value);
        drop(control_output);

        if triggered {
            voice.triggered = false;
        }

        voice.smoother.apply_if_needed(
            samples,
            router.sample_rate(),
            self.params.smooth,
            voice_output.output(samples),
        );
    }
}

impl SynthModule for Expressions {
    fn id(&self) -> ModuleId {
        self.id
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::Expressions
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

    fn update_input_amount(&mut self, _input_type: Input, _src_slot: usize, _amount: StereoSample) {}

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
