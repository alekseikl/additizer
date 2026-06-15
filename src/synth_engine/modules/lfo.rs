use std::{array, f32};

use itertools::izip;

mod config;
mod link;
mod ui_bridge;

pub use config::{LfoConfig, LfoShape};
use link::{AudioEnd, UiEnd, UiEvent, create_link_pair};
pub use ui_bridge::LfoUiBridge;

use crate::synth_engine::{
    Input, ModuleId, ModuleType, Sample, StereoSample,
    buffer::{Buffer, VoicesLayout, new_voices_layout, zero_buffer},
    phase::Phase,
    routing::{
        ControlRouterType, DataType, InputSlots, NUM_CHANNELS, ProcessContext, SamplesOutput,
        SpectralInputSlot, VoiceEvent, VoiceRouter,
    },
    smooth::{SmoothedSample, Smoother},
    synth_module::{ModInput, SynthModule},
};

struct ChannelParams {
    frequency: SmoothedSample,
    phase_shift: SmoothedSample,
    skew: SmoothedSample,
    smooth_time: Sample,
}

impl ChannelParams {
    fn from_config(c: &config::LfoConfig, channel_idx: usize) -> Self {
        Self {
            frequency: c.frequency[channel_idx].into(),
            phase_shift: c.phase_shift[channel_idx].into(),
            skew: c.skew[channel_idx].into(),
            smooth_time: c.smooth_time[channel_idx],
        }
    }
}

struct Params {
    shape: LfoShape,
    bipolar: bool,
    steal_phase: bool,
}

impl Params {
    fn from_config(c: &config::LfoConfig) -> Self {
        Self {
            shape: c.shape,
            bipolar: c.bipolar,
            steal_phase: c.steal_phase,
        }
    }
}

struct VoiceState {
    phase: Phase,
    triggered: bool,
    smoother: Smoother,
}

impl Default for VoiceState {
    fn default() -> Self {
        Self {
            phase: Phase::ZERO,
            triggered: false,
            smoother: Smoother::default(),
        }
    }
}

pub struct Inputs {
    frequency: InputSlots,
    phase_shift: InputSlots,
    skew: InputSlots,
}

impl Default for Inputs {
    fn default() -> Self {
        Self {
            frequency: InputSlots::empty(Input::LowFrequency),
            phase_shift: InputSlots::empty(Input::PhaseShift),
            skew: InputSlots::empty(Input::Skew),
        }
    }
}

impl Inputs {
    fn from_slots(inputs: &[InputSlots], _spectral_inputs: &[SpectralInputSlot]) -> Self {
        let mut result = Self::default();

        for input in inputs {
            match input.input_type {
                Input::LowFrequency => result.frequency = input.clone(),
                Input::PhaseShift => result.phase_shift = input.clone(),
                Input::Skew => result.skew = input.clone(),
                _ => (),
            }
        }

        result
    }

    fn update_amount(&mut self, input_type: Input, src_slot: usize, amount: StereoSample) {
        match input_type {
            Input::LowFrequency => self.frequency.update_amount(src_slot, amount),
            Input::PhaseShift => self.phase_shift.update_amount(src_slot, amount),
            Input::Skew => self.skew.update_amount(src_slot, amount),
            _ => (),
        }
    }
}

type Router<'v, 'f, 'c> = VoiceRouter<'v, 'f, 'c, ControlRouterType>;

struct Buffers {
    frequency: Buffer,
    phase_shift: Buffer,
    skew: Buffer,
}

pub struct Lfo {
    id: ModuleId,
    params: Params,
    channel_params: [ChannelParams; NUM_CHANNELS],
    buffers: Buffers,
    audio_end: AudioEnd,
    ui_end: Option<UiEnd>,
    inputs: Inputs,
    output_slot: usize,
    voices: VoicesLayout<VoiceState>,
}

impl Lfo {
    pub fn new(id: ModuleId) -> Self {
        Self::from_config(&LfoConfig {
            id,
            ..LfoConfig::default()
        })
    }

    pub fn from_config(config: &config::LfoConfig) -> Self {
        let (audio_end, ui_end) = create_link_pair();

        Self {
            id: config.id,
            params: Params::from_config(config),
            channel_params: array::from_fn(|channel_idx| {
                ChannelParams::from_config(config, channel_idx)
            }),
            buffers: Buffers {
                frequency: zero_buffer(),
                phase_shift: zero_buffer(),
                skew: zero_buffer(),
            },
            audio_end,
            ui_end: Some(ui_end),
            inputs: Inputs::default(),
            output_slot: 0,
            voices: new_voices_layout(),
        }
    }

    pub fn get_config(&self) -> LfoConfig {
        LfoConfig {
            id: self.id,
            shape: self.params.shape,
            bipolar: self.params.bipolar,
            steal_phase: self.params.steal_phase,
            frequency: get_smoothed_param!(self, frequency),
            phase_shift: get_smoothed_param!(self, phase_shift),
            skew: get_smoothed_param!(self, skew),
            smooth_time: get_stereo_param!(self, smooth_time),
        }
    }

    set_mono_param!(set_shape, shape, LfoShape);
    set_mono_param!(set_bipolar, bipolar, bool);
    set_mono_param!(set_steal_phase, steal_phase, bool);

    set_smoothed_param!(set_frequency, frequency);
    set_smoothed_param!(set_phase_shift, phase_shift, phase_shift.clamp(-1.0, 1.0));
    set_smoothed_param!(set_skew, skew, skew.clamp(0.0, 1.0));
    set_stereo_param!(set_smooth_time, smooth_time, smooth_time.max(0.0));

    fn triangle(x: Sample) -> Sample {
        2.0 * x.min(1.0 - x)
    }

    fn square(x: Sample) -> Sample {
        Sample::from(x < 0.5)
    }

    fn sine(x: Sample) -> Sample {
        let sine = (f32::consts::PI * x).sin();

        sine * sine
    }

    fn shape_function(shape: LfoShape) -> fn(Sample) -> Sample {
        match shape {
            LfoShape::Triangle => Self::triangle,
            LfoShape::Square => Self::square,
            LfoShape::Sine => Self::sine,
        }
    }

    #[inline]
    fn skew_arg(arg: Sample, skew: Sample) -> Sample {
        let arg_less = arg < skew;

        Sample::from(arg_less) * (0.5 * arg / skew.max(Sample::EPSILON))
            + Sample::from(!arg_less)
                * (0.5 + (arg - skew) * 0.5 / (1.0 - skew).max(Sample::EPSILON))
    }

    #[inline]
    fn apply_bipolar(value: Sample, bipolar: bool) -> Sample {
        Sample::from(bipolar) * value.mul_add(2.0, -1.0) + Sample::from(!bipolar) * value
    }

    fn process_voice(
        &mut self,
        output_slot: &mut VoicesLayout<SamplesOutput>,
        mut router: Router<'_, '_, '_>,
    ) {
        let channel_idx = router.channel_idx();
        let voice_idx = router.voice_idx();
        let inputs = &self.inputs;
        let params = &self.params;
        let channel = &mut self.channel_params[channel_idx];
        let voice = &mut self.voices[channel_idx][voice_idx];
        let samples = router.samples();
        let sample_rate = router.sample_rate();
        let voice_output = &mut output_slot[channel_idx][voice_idx];

        router.buff_param(
            &inputs.frequency,
            &mut channel.frequency,
            &mut self.buffers.frequency,
            voice.triggered,
        );
        router.buff_param(
            &inputs.phase_shift,
            &mut channel.phase_shift,
            &mut self.buffers.phase_shift,
            voice.triggered,
        );
        router.buff_param(
            &inputs.skew,
            &mut channel.skew,
            &mut self.buffers.skew,
            voice.triggered,
        );

        let mut control_output = voice_output.control_output(samples, voice.triggered);
        let shape_func = Self::shape_function(params.shape);
        let freq_phase_mult = Phase::freq_phase_mult(sample_rate);

        voice.smoother.update(sample_rate, channel.smooth_time);

        for (out, frequency, phase_shift, skew) in izip!(
            control_output.output().iter_mut(),
            &self.buffers.frequency,
            &self.buffers.phase_shift,
            &self.buffers.skew,
        ) {
            let arg = voice
                .phase
                .add_normalized(phase_shift.clamp(-1.0, 1.0))
                .normalized();

            *out = Self::apply_bipolar(
                shape_func(Self::skew_arg(arg, skew.clamp(0.0, 1.0))),
                params.bipolar,
            );

            voice.phase += *frequency * freq_phase_mult;
        }

        drop(control_output);

        if voice.triggered {
            voice.smoother.reset(0.0);
            voice.triggered = false;
        }

        voice.smoother.apply_if_needed(
            samples,
            sample_rate,
            channel.smooth_time,
            voice_output.output(samples),
        );
    }
}

impl SynthModule for Lfo {
    fn id(&self) -> ModuleId {
        self.id
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::Lfo
    }

    fn inputs(&self) -> &'static [ModInput] {
        static INPUTS: &[ModInput] = &[
            ModInput::control(Input::LowFrequency),
            ModInput::control(Input::PhaseShift),
            ModInput::control(Input::Skew),
        ];

        INPUTS
    }

    fn output(&self) -> DataType {
        DataType::Control
    }

    fn output_slot(&self) -> usize {
        self.output_slot
    }

    fn set_slots(
        &mut self,
        inputs: &[InputSlots],
        spectral_inputs: &[SpectralInputSlot],
        output_slot: usize,
    ) {
        self.inputs = Inputs::from_slots(inputs, spectral_inputs);
        self.output_slot = output_slot;
    }

    fn update_input_amount(&mut self, input_type: Input, src_slot: usize, amount: StereoSample) {
        self.inputs.update_amount(input_type, src_slot, amount);
    }

    fn handle_events(&mut self, events: &[VoiceEvent]) {
        for channel in self.voices.iter_mut() {
            for event in events {
                if let VoiceEvent::Trigger {
                    voice_idx,
                    prev_voice_idx,
                    ..
                } = event
                {
                    let phase = if let Some(prev_voice_idx) = prev_voice_idx
                        && self.params.steal_phase
                    {
                        channel[*prev_voice_idx].phase
                    } else {
                        Phase::ZERO
                    };

                    let voice = &mut channel[*voice_idx];

                    voice.triggered = true;
                    voice.phase = phase;
                }
            }
        }
    }

    fn handle_ui_events(&mut self) {
        while let Some(event) = self.audio_end.pop_event() {
            match event {
                UiEvent::InputParam { input, value } => match input {
                    Input::LowFrequency => self.set_frequency(value),
                    Input::PhaseShift => self.set_phase_shift(value),
                    Input::Skew => self.set_skew(value),
                    _ => (),
                },
                UiEvent::Shape(shape) => self.set_shape(shape),
                UiEvent::Bipolar(value) => self.set_bipolar(value),
                UiEvent::StealPhase(value) => self.set_steal_phase(value),
                UiEvent::SmoothTime(value) => self.set_smooth_time(value),
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
