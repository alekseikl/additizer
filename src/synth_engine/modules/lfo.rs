use itertools::izip;
use serde::{Deserialize, Serialize};
use std::{any::Any, f32};

use crate::synth_engine::{
    Input, ModuleId, ModuleType, Sample, StereoSample, SynthModule,
    buffer::{Buffer, zero_buffer},
    phase::Phase,
    routing::{DataType, MAX_VOICES, NUM_CHANNELS, Router},
    smoother::Smoother,
    synth_module::{ModInput, ModuleConfigBox, NoteOnParams, ProcessParams, VoiceRouter},
    types::ScalarOutput,
};

#[derive(Default, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum LfoShape {
    #[default]
    Triangle,
    Square,
    Sine,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ChannelParams {
    frequency: Sample,
    phase_shift: Sample,
    skew: Sample,
    smooth_time: Sample,
}

impl Default for ChannelParams {
    fn default() -> Self {
        Self {
            frequency: 1.0,
            phase_shift: 0.0,
            skew: 0.5,
            smooth_time: 0.0,
        }
    }
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct Params {
    shape: LfoShape,
    bipolar: bool,
    reset_phase: bool,
}

pub struct LfoUiData {
    pub label: String,
    pub shape: LfoShape,
    pub bipolar: bool,
    pub reset_phase: bool,
    pub frequency: StereoSample,
    pub phase_shift: StereoSample,
    pub skew: StereoSample,
    pub smooth_time: StereoSample,
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct LfoConfig {
    label: Option<String>,
    params: Params,
    channels: [ChannelParams; NUM_CHANNELS],
}

struct Voice {
    phase: Phase,
    triggered: bool,
    output: ScalarOutput,
    audio_phase: Phase,
    audio_smoother: Smoother,
    audio_output: Buffer,
}

impl Default for Voice {
    fn default() -> Self {
        Self {
            phase: Phase::ZERO,
            triggered: false,
            output: ScalarOutput::default(),
            audio_phase: Phase::ZERO,
            audio_smoother: Smoother::default(),
            audio_output: zero_buffer(),
        }
    }
}

#[derive(Default)]
struct Channel {
    params: ChannelParams,
    voices: [Voice; MAX_VOICES],
}

struct InputBuffers {
    frequency: Buffer,
    phase_shift: Buffer,
    skew: Buffer,
}

pub struct Lfo {
    id: ModuleId,
    label: String,
    params: Params,
    config: ModuleConfigBox<LfoConfig>,
    inputs: InputBuffers,
    channels: [Channel; NUM_CHANNELS],
}

impl Lfo {
    pub fn new(id: ModuleId, config: ModuleConfigBox<LfoConfig>) -> Self {
        let mut lfo = Self {
            id,
            label: format!("LFO {id}"),
            config,
            params: Params::default(),
            inputs: InputBuffers {
                frequency: zero_buffer(),
                phase_shift: zero_buffer(),
                skew: zero_buffer(),
            },
            channels: Default::default(),
        };

        load_module_config!(lfo);
        lfo
    }

    gen_downcast_methods!();

    pub fn get_ui(&self) -> LfoUiData {
        LfoUiData {
            label: self.label.clone(),
            shape: self.params.shape,
            bipolar: self.params.bipolar,
            reset_phase: self.params.reset_phase,
            frequency: get_stereo_param!(self, frequency),
            phase_shift: get_stereo_param!(self, phase_shift),
            skew: get_stereo_param!(self, skew),
            smooth_time: get_stereo_param!(self, smooth_time),
        }
    }

    set_mono_param!(set_shape, shape, LfoShape);
    set_mono_param!(set_bipolar, bipolar, bool);
    set_mono_param!(set_reset_phase, reset_phase, bool);

    set_stereo_param!(set_frequency, frequency);
    set_stereo_param!(set_phase_shift, phase_shift, phase_shift.clamp(-1.0, 1.0));
    set_stereo_param!(set_skew, skew, skew.clamp(0.0, 1.0));
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
        params: &Params,
        channel_params: &ChannelParams,
        voice: &mut Voice,
        current: bool,
        t_step: Sample,
        router: &VoiceRouter,
    ) {
        let frequency = channel_params.frequency + router.scalar(Input::LowFrequency, current);

        let phase_shift = (channel_params.phase_shift + router.scalar(Input::PhaseShift, current))
            .clamp(-1.0, 1.0);

        let skew = (channel_params.skew + router.scalar(Input::Skew, current)).clamp(0.0, 1.0);

        let arg = voice.phase.add_normalized(phase_shift).normalized();

        voice.output.advance(Self::apply_bipolar(
            Self::shape_function(params.shape)(Self::skew_arg(arg, skew)),
            params.bipolar,
        ));
        voice.phase.advance_normalized(t_step * frequency);
    }

    fn process_voice_buffer(
        params: &Params,
        channel_params: &ChannelParams,
        process_params: &ProcessParams,
        inputs: &mut InputBuffers,
        voice: &mut Voice,
        router: &VoiceRouter,
    ) {
        let frequency_mod = router.buffer(Input::LowFrequency, &mut inputs.frequency);
        let phase_shift_mod = router.buffer(Input::PhaseShift, &mut inputs.phase_shift);
        let skew_mod = router.buffer(Input::Skew, &mut inputs.skew);
        let out = voice.audio_output.iter_mut().take(process_params.samples);

        let shape_func = Self::shape_function(params.shape);
        let freq_phase_mult = Phase::freq_phase_mult(process_params.sample_rate);

        voice
            .audio_smoother
            .update(process_params.sample_rate, channel_params.smooth_time);

        for (out, frequency_mod, phase_shift_mod, skew_mod) in
            izip!(out, frequency_mod, phase_shift_mod, skew_mod)
        {
            let arg = voice
                .audio_phase
                .add_normalized(channel_params.phase_shift + phase_shift_mod)
                .normalized();

            let sample = Self::apply_bipolar(
                shape_func(Self::skew_arg(
                    arg,
                    (channel_params.skew + skew_mod).clamp(0.0, 1.0),
                )),
                params.bipolar,
            );

            *out = voice.audio_smoother.tick(sample);
            voice.audio_phase += (channel_params.frequency + frequency_mod) * freq_phase_mult;
        }
    }
}

impl SynthModule for Lfo {
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
        ModuleType::Lfo
    }

    fn inputs(&self) -> &'static [ModInput] {
        static INPUTS: &[ModInput] = &[
            ModInput::scalar(Input::LowFrequency),
            ModInput::scalar(Input::PhaseShift),
            ModInput::scalar(Input::Skew),
        ];

        INPUTS
    }

    fn output(&self) -> DataType {
        DataType::Scalar
    }

    fn note_on(&mut self, params: &NoteOnParams) {
        for channel in &mut self.channels {
            let voice = &mut channel.voices[params.voice_idx];

            voice.triggered = true;

            if params.reset || self.params.reset_phase {
                voice.phase = Phase::ZERO;
                voice.audio_phase = Phase::ZERO;
                voice.audio_smoother.reset(0.0);
            }
        }
    }

    fn process(&mut self, params: &ProcessParams, router: &dyn Router) {
        let t_step = params.buffer_t_step;

        for (channel_idx, channel) in self.channels.iter_mut().enumerate() {
            for voice_idx in params.active_voices {
                let voice = &mut channel.voices[*voice_idx];
                let router = VoiceRouter {
                    router,
                    module_id: self.id,
                    samples: params.samples,
                    sample_rate: params.sample_rate,
                    voice_idx: *voice_idx,
                    channel_idx,
                };

                if voice.triggered {
                    Self::process_voice(&self.params, &channel.params, voice, false, 0.0, &router);
                    voice.triggered = false;
                }
                Self::process_voice(&self.params, &channel.params, voice, true, t_step, &router);

                if params.needs_audio_rate {
                    Self::process_voice_buffer(
                        &self.params,
                        &channel.params,
                        params,
                        &mut self.inputs,
                        voice,
                        &router,
                    );
                }
            }
        }
    }

    fn get_buffer_output(&self, voice_idx: usize, channel_idx: usize) -> &Buffer {
        &self.channels[channel_idx].voices[voice_idx].audio_output
    }

    fn get_scalar_output(&self, current: bool, voice_idx: usize, channel_idx: usize) -> Sample {
        self.channels[channel_idx].voices[voice_idx]
            .output
            .get(current)
    }
}
