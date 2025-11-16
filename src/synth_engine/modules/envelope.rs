use std::any::Any;

use serde::{Deserialize, Serialize};

use crate::{
    synth_engine::{
        ModuleInput,
        routing::{InputType, MAX_VOICES, ModuleId, ModuleType, NUM_CHANNELS, OutputType, Router},
        synth_module::{ModuleConfigBox, NoteOffParams, NoteOnParams, ProcessParams, SynthModule},
        types::{Sample, StereoSample},
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
    keep_voice_alive: bool,
    channels: [ChannelParams; NUM_CHANNELS],
}

impl Default for EnvelopeConfig {
    fn default() -> Self {
        Self {
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
    pub fn new(curvature: Sample, value_from: Sample, value_to: Sample) -> Self {
        let value_from = value_from.max(0.0);

        assert!(value_from <= value_to);

        let power = 1.0 + curvature.clamp(0.0, 1.0) * 9.0;
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
        if self.arg < self.arg_to {
            let result = Some(self.arg.powf(self.power));

            self.arg += arg_step;
            result
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
    pub fn new(curvature: Sample, value_from: Sample, value_to: Sample) -> Self {
        let value_to = value_to.min(1.0);

        assert!(value_from <= value_to);

        let power = 1.0 + curvature.clamp(0.0, 1.0) * 9.0;
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
        if self.arg < self.arg_to {
            let result = Some(Self::calc(self.arg, self.power));

            self.arg += arg_step;
            result
        } else {
            None
        }
    }
}

struct ExponentialIn {
    power: Sample,
    arg: Sample,
    exp_from_arg: Sample,
    arg_from: Sample,
    arg_to: Sample,
}

impl ExponentialIn {
    const LINEAR_THRESHOLD: Sample = 0.005;

    pub fn new(curvature: Sample, value_from: Sample, value_to: Sample) -> Self {
        let value_from = value_from.max(0.0);

        assert!(value_from <= value_to);

        let power = 1.0 + curvature.clamp(0.0, 1.0) * 9.0;
        let inverse_power = power.recip();
        let exp_from_arg = Self::inverse(Self::LINEAR_THRESHOLD, inverse_power);
        let arg = Self::calc_arg(value_from, exp_from_arg, inverse_power);
        let arg_to = Self::calc_arg(value_to, exp_from_arg, inverse_power);

        Self {
            power,
            arg,
            exp_from_arg,
            arg_from: arg,
            arg_to,
        }
    }

    fn inverse(value: Sample, inverse_power: Sample) -> Sample {
        inverse_power * value.ln()
    }

    fn calc_arg(value: Sample, exp_from_arg: Sample, inverse_power: Sample) -> Sample {
        if value < Self::LINEAR_THRESHOLD {
            exp_from_arg - (Self::LINEAR_THRESHOLD - value)
        } else {
            Self::inverse(value, inverse_power)
        }
    }
}

impl CurveIterator for ExponentialIn {
    fn next(&mut self, arg_step: Sample) -> Option<Sample> {
        if self.arg < self.arg_to {
            let result = if self.arg < self.exp_from_arg {
                self.arg - self.arg_from
            } else {
                (self.power * self.arg).exp()
            };

            self.arg += arg_step;
            Some(result)
        } else {
            None
        }
    }
}

struct ExponentialOut {
    power: Sample,
    arg: Sample,
    linear_from_arg: Sample,
    arg_to: Sample,
}

impl ExponentialOut {
    const LINEAR_THRESHOLD: Sample = 0.995;

    pub fn new(curvature: Sample, value_from: Sample, value_to: Sample) -> Self {
        let value_to = value_to.min(1.0);

        assert!(value_from <= value_to);

        let power = 1.0 + curvature.clamp(0.0, 1.0) * 9.0;
        let inverse_power = power.recip();
        let linear_from_arg = Self::inverse(Self::LINEAR_THRESHOLD, inverse_power);
        let arg = Self::calc_arg(value_from, linear_from_arg, inverse_power);
        let arg_to = Self::calc_arg(value_to, linear_from_arg, inverse_power);

        Self {
            power,
            arg,
            linear_from_arg,
            arg_to,
        }
    }

    fn inverse(value: Sample, inverse_power: Sample) -> Sample {
        -inverse_power * (1.0 - value).ln()
    }

    fn calc_arg(value: Sample, linear_from_arg: Sample, inverse_power: Sample) -> Sample {
        if value < Self::LINEAR_THRESHOLD {
            Self::inverse(value, inverse_power)
        } else {
            linear_from_arg + (value - Self::LINEAR_THRESHOLD)
        }
    }
}

impl CurveIterator for ExponentialOut {
    fn next(&mut self, arg_step: Sample) -> Option<Sample> {
        if self.arg < self.arg_to {
            let result = if self.arg < self.linear_from_arg {
                1.0 - (-self.power * self.arg).exp()
            } else {
                Self::LINEAR_THRESHOLD + (self.arg - self.linear_from_arg)
            };

            self.arg += arg_step;
            Some(result)
        } else {
            None
        }
    }
}

type CurveBox = Box<dyn CurveIterator + Send>;

struct CurveInvertor<T: CurveIterator + Send> {
    curve: T,
}

impl<T: CurveIterator + Send + 'static> CurveInvertor<T> {
    fn wrap(curve: T, inverted: bool) -> CurveBox {
        if inverted {
            Box::new(Self { curve })
        } else {
            Box::new(curve)
        }
    }
}

impl<T: CurveIterator + Send> CurveIterator for CurveInvertor<T> {
    fn next(&mut self, arg_step: Sample) -> Option<Sample> {
        self.curve.next(arg_step).map(|value| 1.0 - value)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum EnvelopeCurve {
    Linear,
    PowerIn(Sample),
    PowerOut(Sample),
    ExponentialIn(Sample),
    ExponentialOut(Sample),
}

impl EnvelopeCurve {
    fn curve_iter(&self, from: Sample, to: Sample) -> CurveBox {
        let (value_from, value_to, inverted) = if from > to {
            (1.0 - from, 1.0 - to, true)
        } else {
            (from, to, false)
        };

        match *self {
            Self::Linear => CurveInvertor::wrap(PowerIn::new(0.0, value_from, value_to), inverted),
            Self::PowerIn(curvature) => {
                CurveInvertor::wrap(PowerIn::new(curvature, value_from, value_to), inverted)
            }
            Self::PowerOut(curvature) => {
                CurveInvertor::wrap(PowerOut::new(curvature, value_from, value_to), inverted)
            }
            Self::ExponentialIn(curvature) => CurveInvertor::wrap(
                ExponentialIn::new(curvature, value_from, value_to),
                inverted,
            ),
            Self::ExponentialOut(curvature) => CurveInvertor::wrap(
                ExponentialOut::new(curvature, value_from, value_to),
                inverted,
            ),
        }
    }
}

pub struct EnvelopeUIData {
    pub attack: StereoSample,
    pub attack_curve: EnvelopeCurve,
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

impl Stage {
    fn next_level(&mut self, sustain: Sample) -> Option<Sample> {
        match self {
            Self::Attack(curve) => curve.next(0.0),
            Self::Hold { t: _ } => None,
            Self::Decay(curve) => curve.next(0.0),
            Self::Sustain => Some(sustain),
            Self::Release(curve) => curve.next(0.0),
            Self::Done => None,
        }
    }
}

struct Voice {
    stage: Stage,
    prev_output: Sample,
    output: Sample,
}

impl Default for Voice {
    fn default() -> Self {
        Self {
            stage: Stage::Done,
            prev_output: 0.0,
            output: 0.0,
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
            attack_curve: EnvelopeCurve::PowerIn(0.3),
            hold: 0.0,
            decay: from_ms(200.0),
            decay_curve: EnvelopeCurve::PowerOut(0.2),
            sustain: 1.0,
            release: from_ms(300.0),
            release_curve: EnvelopeCurve::PowerOut(0.2),
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
    config: ModuleConfigBox<EnvelopeConfig>,
    keep_voice_alive: bool,
    channels: [Channel; NUM_CHANNELS],
}

macro_rules! set_param_method {
    ($fn_name:ident, $param:ident, $transform:expr) => {
        pub fn $fn_name(&mut self, $param: StereoSample) -> &mut Self {
            for (channel, $param) in self.channels.iter_mut().zip($param.iter()) {
                channel.params.$param = $transform;
            }

            {
                let mut cfg = self.config.lock();

                for (cfg_channel, channel) in cfg.channels.iter_mut().zip(self.channels.iter()) {
                    cfg_channel.$param = channel.params.$param;
                }
            }

            self
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
            config,
            keep_voice_alive: true,
            channels: Default::default(),
        };

        {
            let cfg = env.config.lock();
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
            attack: extract_param!(self, attack),
            attack_curve: self.channels[0].params.attack_curve,
            decay: extract_param!(self, decay),
            decay_curve: self.channels[0].params.decay_curve,
            sustain: extract_param!(self, sustain),
            release: extract_param!(self, release),
            release_curve: self.channels[0].params.release_curve,
            keep_voice_alive: self.keep_voice_alive,
        }
    }

    pub fn set_keep_voice_alive(&mut self, keep_alive: bool) -> &mut Self {
        self.keep_voice_alive = keep_alive;

        {
            let mut cfg = self.config.lock();
            cfg.keep_voice_alive = keep_alive;
        }

        self
    }

    set_curve_method!(set_attack_curve, attack_curve);
    set_curve_method!(set_decay_curve, decay_curve);
    set_curve_method!(set_release_curve, release_curve);

    set_param_method!(set_attack, attack, *attack);
    set_param_method!(set_decay, decay, *decay);
    set_param_method!(set_sustain, sustain, *sustain);
    set_param_method!(set_release, release, release.max(from_ms(2.0)));

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

    fn process_channel_voice(
        id: ModuleId,
        channel: &mut Channel,
        params: &ProcessParams,
        voice_idx: usize,
        channel_idx: usize,
        router: &dyn Router,
    ) {
        let voice = &mut channel.voices[voice_idx];
        let env = &channel.params;
        let t_step = params.buffer_t_step;

        voice.prev_output = voice.output;

        voice.output = loop {
            voice.stage = match &mut voice.stage {
                Stage::Attack(curve) => {
                    let attack = env.attack
                        + router
                            .get_scalar_input(ModuleInput::attack(id), voice_idx, channel_idx)
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
                            .get_scalar_input(ModuleInput::hold(id), voice_idx, channel_idx)
                            .unwrap_or(0.0);

                    if *t < hold {
                        *t += t_step;
                        break 1.0;
                    }

                    Stage::Decay(env.decay_curve.curve_iter(1.0, env.sustain))
                }
                Stage::Decay(curve) => {
                    let decay = env.decay
                        + router
                            .get_scalar_input(ModuleInput::decay(id), voice_idx, channel_idx)
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
                            .get_scalar_input(ModuleInput::sustain(id), voice_idx, channel_idx)
                            .unwrap_or(0.0);
                }
                Stage::Release(curve) => {
                    let release = env.release
                        + router
                            .get_scalar_input(ModuleInput::release(id), voice_idx, channel_idx)
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
        };
    }

    fn reset_voice(channel: &ChannelParams, voice: &mut Voice, same_note_retrigger: bool) {
        let value_from = if same_note_retrigger {
            voice.stage.next_level(channel.sustain).unwrap_or(0.0)
        } else {
            0.0
        };

        voice.stage = Stage::Attack(channel.attack_curve.curve_iter(value_from, 1.0));
        voice.output = 0.0;
    }

    fn release_voice(params: &ChannelParams, voice: &mut Voice) {
        voice.stage = Stage::Release(
            params.release_curve.curve_iter(
                voice
                    .stage
                    .next_level(params.sustain)
                    .unwrap_or(params.sustain),
                0.0,
            ),
        );
    }
}

impl SynthModule for Envelope {
    fn id(&self) -> ModuleId {
        self.id
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

    fn note_on(&mut self, params: &NoteOnParams, _router: &dyn Router) {
        for channel in &mut self.channels {
            Self::reset_voice(
                &channel.params,
                &mut channel.voices[params.voice_idx],
                params.same_note_retrigger,
            );
        }
    }

    fn note_off(&mut self, params: &NoteOffParams) {
        for channel in &mut self.channels {
            Self::release_voice(&channel.params, &mut channel.voices[params.voice_idx]);
        }
    }

    fn process(&mut self, params: &ProcessParams, router: &dyn Router) {
        for (channel_idx, channel) in self.channels.iter_mut().enumerate() {
            for voice_idx in params.active_voices {
                Self::process_channel_voice(
                    self.id,
                    channel,
                    params,
                    *voice_idx,
                    channel_idx,
                    router,
                );
            }
        }
    }

    fn get_scalar_output(&self, voice_idx: usize, channel: usize) -> (Sample, Sample) {
        let voice = &self.channels[channel].voices[voice_idx];

        (voice.prev_output, voice.output)
    }
}
