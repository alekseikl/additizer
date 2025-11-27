use std::any::Any;

use serde::{Deserialize, Serialize};

use crate::{
    synth_engine::{
        ModuleInput,
        routing::{InputType, MAX_VOICES, ModuleId, ModuleType, NUM_CHANNELS, OutputType, Router},
        synth_module::{
            ModuleConfigBox, NoteOffParams, NoteOnParams, ProcessParams, SynthModule, VoiceRouter,
        },
        types::{Sample, ScalarOutput, StereoSample},
    },
    utils::from_ms,
};

#[derive(Debug)]
pub struct EnvelopeActivityState {
    pub voice_idx: usize,
    pub active: bool,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct EnvelopeConfig {
    label: Option<String>,
    keep_voice_alive: bool,
    channels: [ChannelParams; NUM_CHANNELS],
}

impl Default for EnvelopeConfig {
    fn default() -> Self {
        Self {
            label: None,
            keep_voice_alive: true,
            channels: Default::default(),
        }
    }
}

trait CurveIterator {
    fn next(&mut self, arg_step: Sample) -> Option<Sample>;
}

struct PowerIn {
    power: Sample,
    arg: Sample,
    arg_to: Sample,
}

impl PowerIn {
    pub fn new(curvature: Sample, (value_from, value_to): (Sample, Sample)) -> Self {
        let value_from = value_from.max(0.0);

        assert!(value_from <= value_to);

        let power = 1.0 + curvature.clamp(0.0, 1.0) * 10.0;
        let inverse_power = power.recip();

        Self {
            power,
            arg: value_from.powf(inverse_power),
            arg_to: value_to.powf(inverse_power),
        }
    }
}

impl CurveIterator for PowerIn {
    fn next(&mut self, arg_step: Sample) -> Option<Sample> {
        self.arg += arg_step;

        if self.arg < self.arg_to {
            Some(self.arg.powf(self.power))
        } else {
            None
        }
    }
}

struct PowerOut {
    power: Sample,
    arg: Sample,
    arg_to: Sample,
}

impl PowerOut {
    pub fn new(curvature: Sample, (value_from, value_to): (Sample, Sample)) -> Self {
        let value_to = value_to.min(1.0);

        assert!(value_from <= value_to);

        let power = 1.0 + curvature.clamp(0.0, 1.0) * 10.0;
        let inverse_power = power.recip();

        Self {
            power,
            arg: Self::calc(value_from, inverse_power),
            arg_to: Self::calc(value_to, inverse_power),
        }
    }

    #[inline]
    fn calc(x: Sample, power: Sample) -> Sample {
        1.0 - (1.0 - x).powf(power)
    }
}

impl CurveIterator for PowerOut {
    fn next(&mut self, arg_step: Sample) -> Option<Sample> {
        self.arg += arg_step;

        if self.arg < self.arg_to {
            Some(Self::calc(self.arg, self.power))
        } else {
            None
        }
    }
}

struct ExponentialIn {
    linear_rate: Sample,
    arg: Sample,
    arg_to: Sample,
}

impl ExponentialIn {
    const RATE: Sample = 5.0;
    const LINEAR_THRESHOLD_ARG: Sample = 0.05;

    pub fn new((value_from, value_to): (Sample, Sample)) -> Self {
        let value_from = value_from.max(0.0);

        assert!(value_from <= value_to);

        let exp_from_value = Self::calc_exp(Self::LINEAR_THRESHOLD_ARG);
        let linear_rate = exp_from_value / Self::LINEAR_THRESHOLD_ARG;
        let arg = Self::calc_arg(linear_rate, exp_from_value, value_from);
        let arg_to = Self::calc_arg(linear_rate, exp_from_value, value_to);

        Self {
            linear_rate,
            arg,
            arg_to,
        }
    }

    fn calc_exp(arg: Sample) -> Sample {
        (Self::RATE * (arg - 1.0)).exp()
    }

    fn calc_arg(linear_rate: Sample, exp_from_value: Sample, value: Sample) -> Sample {
        if value < exp_from_value {
            value / linear_rate
        } else {
            value.ln() / Self::RATE + 1.0
        }
    }
}

impl CurveIterator for ExponentialIn {
    fn next(&mut self, arg_step: Sample) -> Option<Sample> {
        self.arg += arg_step;

        if self.arg < self.arg_to {
            Some(if self.arg < Self::LINEAR_THRESHOLD_ARG {
                self.linear_rate * self.arg
            } else {
                Self::calc_exp(self.arg)
            })
        } else {
            None
        }
    }
}

struct ExponentialOut {
    linear_from_value: Sample,
    linear_rate: Sample,
    arg: Sample,
    arg_to: Sample,
}

impl ExponentialOut {
    const RATE: Sample = 5.0;
    const LINEAR_THRESHOLD_ARG: Sample = 0.95;

    pub fn new((value_from, value_to): (Sample, Sample)) -> Self {
        let value_to = value_to.min(1.0);

        assert!(value_from <= value_to);

        let linear_from_value = Self::calc_exp(Self::LINEAR_THRESHOLD_ARG);
        let linear_rate = (1.0 - linear_from_value) / (1.0 - Self::LINEAR_THRESHOLD_ARG);
        let arg = Self::calc_arg(linear_rate, linear_from_value, value_from);
        let arg_to = Self::calc_arg(linear_rate, linear_from_value, value_to);

        Self {
            linear_from_value,
            linear_rate,
            arg,
            arg_to,
        }
    }

    fn calc_exp(arg: Sample) -> Sample {
        1.0 - (-Self::RATE * arg).exp()
    }

    fn calc_arg(linear_rate: Sample, linear_from_value: Sample, value: Sample) -> Sample {
        if value < linear_from_value {
            -(1.0 - value).ln() / Self::RATE
        } else {
            Self::LINEAR_THRESHOLD_ARG + (value - linear_from_value) / linear_rate
        }
    }
}

impl CurveIterator for ExponentialOut {
    fn next(&mut self, arg_step: Sample) -> Option<Sample> {
        self.arg += arg_step;

        if self.arg < self.arg_to {
            Some(if self.arg < Self::LINEAR_THRESHOLD_ARG {
                Self::calc_exp(self.arg)
            } else {
                self.linear_from_value + (self.arg - Self::LINEAR_THRESHOLD_ARG) * self.linear_rate
            })
        } else {
            None
        }
    }
}

struct ExponentialTail {
    arg: Sample,
}

impl ExponentialTail {
    const RATE: Sample = 5.0;
    const END_AT: Sample = 1.0 - 0.00001; //-100dB

    pub fn new(value_from: Sample) -> Self {
        let value_from = value_from.min(1.0);

        Self {
            arg: -(1.0 - value_from).ln() / Self::RATE,
        }
    }

    fn calc_exp(arg: Sample) -> Sample {
        1.0 - (-Self::RATE * arg).exp()
    }
}

impl CurveIterator for ExponentialTail {
    fn next(&mut self, arg_step: Sample) -> Option<Sample> {
        self.arg += arg_step;

        let result = Self::calc_exp(self.arg);

        if result < Self::END_AT {
            Some(result)
        } else {
            None
        }
    }
}

type CurveBox = Box<dyn CurveIterator + Send>;

struct CurveTransform<T: CurveIterator + Send> {
    curve: T,
    value_from: Sample,
    interval: Sample,
}

impl<T: CurveIterator + Send + 'static> CurveTransform<T> {
    fn wrap(curve: T, from: Sample, to: Sample, full_range: bool) -> CurveBox {
        if full_range {
            Box::new(Self {
                curve,
                value_from: from,
                interval: to - from,
            })
        } else {
            Box::new(Self {
                curve,
                value_from: if from > to { 1.0 } else { 0.0 },
                interval: (to - from).signum(),
            })
        }
    }
}

impl<T: CurveIterator + Send> CurveIterator for CurveTransform<T> {
    fn next(&mut self, arg_step: Sample) -> Option<Sample> {
        self.curve
            .next(arg_step)
            .map(|v| self.value_from + self.interval * v)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum EnvelopeCurve {
    Linear { full_range: bool },
    PowerIn { full_range: bool, curvature: Sample },
    PowerOut { full_range: bool, curvature: Sample },
    ExponentialIn { full_range: bool },
    ExponentialOut { full_range: bool },
    ExponentialTail { full_range: bool },
}

impl EnvelopeCurve {
    fn adjust_curve_range(from: Sample, to: Sample, full_range: bool) -> (Sample, Sample) {
        if full_range {
            (0.0, 1.0)
        } else if from > to {
            (1.0 - from, 1.0 - to)
        } else {
            (from, to)
        }
    }

    fn curve_iter(&self, from: Sample, to: Sample) -> CurveBox {
        let from = from.clamp(0.0, 1.0);
        let to = to.clamp(0.0, 1.0);

        match *self {
            Self::Linear { full_range } => CurveTransform::wrap(
                PowerIn::new(0.0, Self::adjust_curve_range(from, to, full_range)),
                from,
                to,
                full_range,
            ),
            Self::PowerIn {
                curvature,
                full_range,
            } => CurveTransform::wrap(
                PowerIn::new(curvature, Self::adjust_curve_range(from, to, full_range)),
                from,
                to,
                full_range,
            ),
            Self::PowerOut {
                full_range,
                curvature,
            } => CurveTransform::wrap(
                PowerOut::new(curvature, Self::adjust_curve_range(from, to, full_range)),
                from,
                to,
                full_range,
            ),
            Self::ExponentialIn { full_range } => CurveTransform::wrap(
                ExponentialIn::new(Self::adjust_curve_range(from, to, full_range)),
                from,
                to,
                full_range,
            ),
            Self::ExponentialOut { full_range } => CurveTransform::wrap(
                ExponentialOut::new(Self::adjust_curve_range(from, to, full_range)),
                from,
                to,
                full_range,
            ),
            Self::ExponentialTail { full_range } => CurveTransform::wrap(
                ExponentialTail::new(Self::adjust_curve_range(from, to, full_range).0),
                from,
                to,
                full_range,
            ),
        }
    }
}

pub struct EnvelopeUIData {
    pub label: String,
    pub attack: StereoSample,
    pub attack_curve: EnvelopeCurve,
    pub hold: StereoSample,
    pub decay: StereoSample,
    pub decay_curve: EnvelopeCurve,
    pub sustain: StereoSample,
    pub release: StereoSample,
    pub release_curve: EnvelopeCurve,
    pub keep_voice_alive: bool,
}

enum Stage {
    Attack(CurveBox),
    Hold { t: Sample },
    Decay(CurveBox),
    Sustain,
    Release(CurveBox),
    Done,
}

struct Voice {
    stage: Stage,
    triggered: bool,
    output: ScalarOutput,
}

impl Default for Voice {
    fn default() -> Self {
        Self {
            stage: Stage::Done,
            triggered: false,
            output: ScalarOutput::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelParams {
    attack: Sample,
    attack_curve: EnvelopeCurve,
    hold: Sample,
    decay: Sample,
    decay_curve: EnvelopeCurve,
    sustain: Sample,
    release: Sample,
    release_curve: EnvelopeCurve,
}

impl Default for ChannelParams {
    fn default() -> Self {
        Self {
            attack: from_ms(10.0),
            attack_curve: EnvelopeCurve::PowerIn {
                full_range: true,
                curvature: 0.3,
            },
            hold: 0.0,
            decay: from_ms(200.0),
            decay_curve: EnvelopeCurve::PowerOut {
                full_range: true,
                curvature: 0.2,
            },
            sustain: 1.0,
            release: from_ms(300.0),
            release_curve: EnvelopeCurve::PowerOut {
                full_range: true,
                curvature: 0.2,
            },
        }
    }
}

#[derive(Default)]
struct Channel {
    params: ChannelParams,
    voices: [Voice; MAX_VOICES],
}

pub struct Envelope {
    id: ModuleId,
    label: String,
    config: ModuleConfigBox<EnvelopeConfig>,
    keep_voice_alive: bool,
    channels: [Channel; NUM_CHANNELS],
}

macro_rules! set_param_method {
    ($fn_name:ident, $param:ident, $transform:expr) => {
        pub fn $fn_name(&mut self, $param: StereoSample) {
            for (channel, $param) in self.channels.iter_mut().zip($param.iter()) {
                channel.params.$param = $transform;
            }

            {
                let mut cfg = self.config.lock();

                for (cfg_channel, channel) in cfg.channels.iter_mut().zip(self.channels.iter()) {
                    cfg_channel.$param = channel.params.$param;
                }
            }
        }
    };
}

macro_rules! set_curve_method {
    ($fn_name:ident, $param:ident) => {
        pub fn $fn_name(&mut self, $param: EnvelopeCurve) {
            for channel in &mut self.channels {
                channel.params.$param = $param;
            }

            {
                let mut cfg = self.config.lock();

                for channel in &mut cfg.channels {
                    channel.$param = $param;
                }
            }
        }
    };
}

macro_rules! extract_param {
    ($self:ident, $param:ident) => {
        StereoSample::from_iter($self.channels.iter().map(|channel| channel.params.$param))
    };
}

impl Envelope {
    pub fn new(id: ModuleId, config: ModuleConfigBox<EnvelopeConfig>) -> Self {
        let mut env = Self {
            id,
            label: format!("Envelope {id}"),
            config,
            keep_voice_alive: true,
            channels: Default::default(),
        };

        {
            let cfg = env.config.lock();

            if let Some(label) = cfg.label.as_ref() {
                env.label = label.clone();
            }

            for (channel, cfg_channel) in env.channels.iter_mut().zip(cfg.channels.iter()) {
                channel.params = cfg_channel.clone();
            }

            env.keep_voice_alive = cfg.keep_voice_alive;
        }

        env
    }

    gen_downcast_methods!(Envelope);

    pub fn get_ui(&self) -> EnvelopeUIData {
        EnvelopeUIData {
            label: self.label.clone(),
            attack: extract_param!(self, attack),
            attack_curve: self.channels[0].params.attack_curve,
            hold: extract_param!(self, hold),
            decay: extract_param!(self, decay),
            decay_curve: self.channels[0].params.decay_curve,
            sustain: extract_param!(self, sustain),
            release: extract_param!(self, release),
            release_curve: self.channels[0].params.release_curve,
            keep_voice_alive: self.keep_voice_alive,
        }
    }

    pub fn set_keep_voice_alive(&mut self, keep_alive: bool) {
        self.keep_voice_alive = keep_alive;

        {
            let mut cfg = self.config.lock();
            cfg.keep_voice_alive = keep_alive;
        }
    }

    set_curve_method!(set_attack_curve, attack_curve);
    set_curve_method!(set_decay_curve, decay_curve);
    set_curve_method!(set_release_curve, release_curve);

    set_param_method!(set_attack, attack, *attack);
    set_param_method!(set_hold, hold, *hold);
    set_param_method!(set_decay, decay, *decay);
    set_param_method!(set_sustain, sustain, *sustain);
    set_param_method!(set_release, release, *release);

    pub fn check_activity(&self, activity: &mut [EnvelopeActivityState]) {
        if self.keep_voice_alive {
            for channel in &self.channels {
                for voice_activity in activity.iter_mut() {
                    let voice = &channel.voices[voice_activity.voice_idx];

                    voice_activity.active =
                        voice_activity.active || !matches!(voice.stage, Stage::Done);
                }
            }
        }
    }

    fn process_voice(
        id: ModuleId,
        env: &ChannelParams,
        voice: &mut Voice,
        current: bool,
        t_step: Sample,
        router: &VoiceRouter,
    ) {
        voice.output.advance(loop {
            voice.stage = match &mut voice.stage {
                Stage::Attack(curve) => {
                    let attack = env.attack
                        + router
                            .get_scalar_input(ModuleInput::attack(id), current)
                            .unwrap_or(0.0);

                    if attack > 0.0
                        && let Some(value) = curve.next(t_step / attack)
                    {
                        break value;
                    }

                    Stage::Hold { t: 0.0 }
                }
                Stage::Hold { t } => {
                    let hold = env.hold
                        + router
                            .get_scalar_input(ModuleInput::hold(id), current)
                            .unwrap_or(0.0);

                    *t += t_step;

                    if *t < hold {
                        break 1.0;
                    }

                    Stage::Decay(env.decay_curve.curve_iter(1.0, env.sustain))
                }
                Stage::Decay(curve) => {
                    let decay = env.decay
                        + router
                            .get_scalar_input(ModuleInput::decay(id), current)
                            .unwrap_or(0.0);

                    if decay > 0.0
                        && let Some(value) = curve.next(t_step / decay)
                    {
                        break value;
                    }

                    Stage::Sustain
                }
                Stage::Sustain => {
                    break env.sustain
                        + router
                            .get_scalar_input(ModuleInput::sustain(id), current)
                            .unwrap_or(0.0);
                }
                Stage::Release(curve) => {
                    let release = env.release
                        + router
                            .get_scalar_input(ModuleInput::release(id), current)
                            .unwrap_or(0.0);

                    if release > 0.0
                        && let Some(value) = curve.next(t_step / release)
                    {
                        break value;
                    }

                    Stage::Done
                }
                Stage::Done => {
                    break 0.0;
                }
            };
        });
    }

    fn trigger_voice(channel: &ChannelParams, voice: &mut Voice, reset: bool) {
        if reset {
            voice.stage = Stage::Attack(channel.attack_curve.curve_iter(0.0, 1.0));
            voice.output = ScalarOutput::default();
        } else {
            voice.stage =
                Stage::Attack(channel.attack_curve.curve_iter(voice.output.current(), 1.0));
        }

        voice.triggered = true;
    }

    fn release_voice(params: &ChannelParams, voice: &mut Voice) {
        voice.stage = Stage::Release(params.release_curve.curve_iter(voice.output.current(), 0.0));
        voice.triggered = true;
    }
}

impl SynthModule for Envelope {
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
        ModuleType::Envelope
    }

    fn is_spectral_rate(&self) -> bool {
        true
    }

    fn inputs(&self) -> &'static [InputType] {
        &[
            InputType::Attack,
            InputType::Hold,
            InputType::Decay,
            InputType::Sustain,
            InputType::Release,
        ]
    }

    fn output_type(&self) -> OutputType {
        OutputType::Scalar
    }

    fn note_on(&mut self, params: &NoteOnParams) {
        for channel in &mut self.channels {
            Self::trigger_voice(
                &channel.params,
                &mut channel.voices[params.voice_idx],
                params.reset,
            );
        }
    }

    fn note_off(&mut self, params: &NoteOffParams) {
        for channel in &mut self.channels {
            Self::release_voice(&channel.params, &mut channel.voices[params.voice_idx]);
        }
    }

    fn process(&mut self, params: &ProcessParams, router: &dyn Router) {
        let t_step = params.buffer_t_step;

        for (channel_idx, channel) in self.channels.iter_mut().enumerate() {
            let env = &channel.params;

            for voice_idx in params.active_voices {
                let voice = &mut channel.voices[*voice_idx];
                let router = VoiceRouter {
                    router,
                    samples: params.samples,
                    voice_idx: *voice_idx,
                    channel_idx,
                };

                if voice.triggered {
                    Self::process_voice(self.id, env, voice, false, 0.0, &router);
                    voice.triggered = false;
                }
                Self::process_voice(self.id, env, voice, true, t_step, &router);
            }
        }
    }

    fn get_scalar_output(&self, current: bool, voice_idx: usize, channel: usize) -> Sample {
        self.channels[channel].voices[voice_idx].output.get(current)
    }
}
