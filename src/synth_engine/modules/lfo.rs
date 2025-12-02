use serde::{Deserialize, Serialize};
use std::{any::Any, f32};

use crate::synth_engine::{
    InputType, ModuleId, ModuleInput, ModuleType, Sample, StereoSample, SynthModule,
    phase::Phase,
    routing::{MAX_VOICES, NUM_CHANNELS, OutputType, Router},
    synth_module::{ModuleConfigBox, NoteOnParams, ProcessParams, VoiceRouter},
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
}

impl Default for ChannelParams {
    fn default() -> Self {
        Self {
            frequency: 1.0,
            phase_shift: 0.0,
            skew: 0.5,
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
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct LfoConfig {
    label: Option<String>,
    params: Params,
    channels: [ChannelParams; NUM_CHANNELS],
}

#[derive(Default)]
struct Voice {
    phase: Phase,
    triggered: bool,
    output: ScalarOutput,
}

#[derive(Default)]
struct Channel {
    params: ChannelParams,
    voices: [Voice; MAX_VOICES],
}

pub struct Lfo {
    id: ModuleId,
    label: String,
    params: Params,
    config: ModuleConfigBox<LfoConfig>,
    channels: [Channel; NUM_CHANNELS],
}

macro_rules! set_param_method {
    ($fn_name:ident, $param:ident, $transform:expr) => {
        pub fn $fn_name(&mut self, $param: StereoSample) {
            for (channel, $param) in self.channels.iter_mut().zip($param.iter()) {
                channel.params.$param = $transform;
            }

            let mut cfg = self.config.lock();

            for (cfg_channel, channel) in cfg.channels.iter_mut().zip(self.channels.iter()) {
                cfg_channel.$param = channel.params.$param;
            }
        }
    };
}

macro_rules! extract_param {
    ($self:ident, $param:ident) => {
        StereoSample::from_iter($self.channels.iter().map(|channel| channel.params.$param))
    };
}

impl Lfo {
    pub fn new(id: ModuleId, config: ModuleConfigBox<LfoConfig>) -> Self {
        let mut lfo = Self {
            id,
            label: format!("LFO {id}"),
            config,
            params: Params {
                shape: LfoShape::Triangle,
                bipolar: false,
                reset_phase: false,
            },
            channels: Default::default(),
        };

        {
            let cfg = lfo.config.lock();

            if let Some(label) = cfg.label.as_ref() {
                lfo.label = label.clone();
            }

            for (channel, cfg_channel) in lfo.channels.iter_mut().zip(cfg.channels.iter()) {
                channel.params = cfg_channel.clone();
            }

            lfo.params = cfg.params.clone();
        }

        lfo
    }

    gen_downcast_methods!(Lfo);

    pub fn get_ui(&self) -> LfoUiData {
        LfoUiData {
            label: self.label.clone(),
            shape: self.params.shape,
            bipolar: self.params.bipolar,
            reset_phase: self.params.reset_phase,
            frequency: extract_param!(self, frequency),
            phase_shift: extract_param!(self, phase_shift),
            skew: extract_param!(self, skew),
        }
    }

    set_param_method!(set_frequency, frequency, frequency.clamp(-50.0, 50.0));
    set_param_method!(set_phase_shift, phase_shift, phase_shift.clamp(-1.0, 1.0));
    set_param_method!(set_skew, skew, skew.clamp(0.0, 1.0));

    pub fn set_shape(&mut self, shape: LfoShape) {
        self.params.shape = shape;
        self.config.lock().params.shape = shape;
    }

    pub fn set_bipolar(&mut self, bipolar: bool) {
        self.params.bipolar = bipolar;
        self.config.lock().params.bipolar = bipolar;
    }

    pub fn set_reset_phase(&mut self, reset: bool) {
        self.params.reset_phase = reset;
        self.config.lock().params.reset_phase = reset;
    }

    fn triangle(x: Sample) -> Sample {
        if x < 0.5 { 2.0 * x } else { 2.0 - 2.0 * x }
    }

    fn square(x: Sample) -> Sample {
        if x < 0.5 { 1.0 } else { 0.0 }
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

    fn process_voice(
        id: ModuleId,
        params: &Params,
        channel_params: &ChannelParams,
        voice: &mut Voice,
        current: bool,
        t_step: Sample,
        router: &VoiceRouter,
    ) {
        let frequency = (channel_params.frequency
            + router.get_scalar_input(ModuleInput::low_frequency(id), current))
        .clamp(-50.0, 50.0);

        let phase_shift = (channel_params.phase_shift
            + router.get_scalar_input(ModuleInput::phase_shift_scalar(id), current))
        .clamp(-1.0, 1.0);

        let skew = (channel_params.skew + router.get_scalar_input(ModuleInput::skew(id), current))
            .clamp(0.0, 1.0);

        let arg = voice.phase.add_normalized(phase_shift).normalized();

        let skewed_arg = if skew == 0.0 {
            0.5 + 0.5 * arg
        } else if skew == 1.0 {
            0.5 * arg
        } else if arg < skew {
            arg * 0.5 / skew
        } else {
            0.5 + (arg - skew) * 0.5 / (1.0 - skew)
        };

        let mut value = Self::shape_function(params.shape)(skewed_arg);

        if params.bipolar {
            value = value * 2.0 - 1.0;
        }

        voice.output.advance(value);
        voice.phase.advance_normalized(t_step * frequency);
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

    fn inputs(&self) -> &'static [InputType] {
        &[
            InputType::LowFrequency,
            InputType::PhaseShiftScalar,
            InputType::Skew,
        ]
    }

    fn output_type(&self) -> OutputType {
        OutputType::Scalar
    }

    fn note_on(&mut self, params: &NoteOnParams) {
        for channel in &mut self.channels {
            let voice = &mut channel.voices[params.voice_idx];

            voice.triggered = true;

            if params.reset || self.params.reset_phase {
                voice.phase = Phase::ZERO;
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
                    samples: params.samples,
                    voice_idx: *voice_idx,
                    channel_idx,
                };

                if voice.triggered {
                    Self::process_voice(
                        self.id,
                        &self.params,
                        &channel.params,
                        voice,
                        false,
                        0.0,
                        &router,
                    );
                    voice.triggered = false;
                }
                Self::process_voice(
                    self.id,
                    &self.params,
                    &channel.params,
                    voice,
                    true,
                    t_step,
                    &router,
                );
            }
        }
    }

    fn get_scalar_output(&self, current: bool, voice_idx: usize, channel: usize) -> Sample {
        self.channels[channel].voices[voice_idx].output.get(current)
    }
}
