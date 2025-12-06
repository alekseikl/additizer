use std::any::Any;

use serde::{Deserialize, Serialize};

use crate::{
    synth_engine::{
        StereoSample,
        curves::{CurveFunction, ExponentialIn, ExponentialOut, PowerIn, PowerOut},
        routing::{DataType, Input, MAX_VOICES, ModuleId, ModuleType, NUM_CHANNELS, Router},
        synth_module::{
            InputInfo, ModuleConfigBox, NoteOffParams, NoteOnParams, ProcessParams, SynthModule,
            VoiceAlive, VoiceRouter,
        },
        types::{Sample, ScalarOutput},
    },
    utils::from_ms,
};

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

enum CurveResult {
    Value(Sample),
    TimeRemainder(Sample),
}

trait CurveIterator {
    fn next(&mut self, t_step: Sample, time: Sample) -> CurveResult;
}

type CurveBox = Box<dyn CurveIterator + Send>;

struct CurveIterParams {
    from: Sample,
    to: Sample,
    time: Sample,
    t_from: Sample,
}
struct CurveIter<T: CurveFunction + Send> {
    curve_fn: T,
    t_from: Sample,
    t: Sample,
    arg_to: Sample,
    value_from: Sample,
    interval: Sample,
}

impl<T: CurveFunction + Send + 'static> CurveIter<T> {
    fn iter(
        curve_fn: T,
        CurveIterParams {
            from,
            to,
            time,
            t_from,
        }: CurveIterParams,
        full_range: bool,
    ) -> CurveBox {
        let from = from.clamp(0.0, 1.0);
        let to = to.clamp(0.0, 1.0);

        let (arg_from, arg_to) = if full_range {
            (0.0, 1.0)
        } else if from > to {
            (
                curve_fn.calc_inverse(1.0 - from),
                curve_fn.calc_inverse(1.0 - to),
            )
        } else {
            (curve_fn.calc_inverse(from), curve_fn.calc_inverse(to))
        };

        let t = t_from + arg_from * time;

        let iter = if full_range {
            Self {
                curve_fn,
                t_from,
                t,
                arg_to,
                value_from: from,
                interval: to - from,
            }
        } else {
            Self {
                curve_fn,
                t_from,
                t,
                arg_to,
                value_from: if from > to { 1.0 } else { 0.0 },
                interval: (to - from).signum(),
            }
        };

        Box::new(iter)
    }
}

impl<T: CurveFunction + Send + 'static> CurveIterator for CurveIter<T> {
    fn next(&mut self, t_step: Sample, time: Sample) -> CurveResult {
        let t_to = self.arg_to * time;

        self.t = (self.t + t_step).max(0.0);

        if self.t < t_to {
            CurveResult::Value(self.value_from + self.interval * self.curve_fn.calc(self.t / time))
        } else {
            CurveResult::TimeRemainder(if time > 0.0 {
                (self.t - t_to).clamp(0.0, t_step)
            } else {
                self.t_from + t_step
            })
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum EnvelopeCurve {
    Linear { full_range: bool },
    PowerIn { full_range: bool, curvature: Sample },
    PowerOut { full_range: bool, curvature: Sample },
    ExponentialIn { full_range: bool },
    ExponentialOut { full_range: bool },
}

impl EnvelopeCurve {
    fn curve_iter(&self, from: Sample, to: Sample, time: Sample, t_from: Sample) -> CurveBox {
        let params = CurveIterParams {
            from,
            to,
            time,
            t_from,
        };

        match *self {
            Self::Linear { full_range } => CurveIter::iter(PowerIn::new(0.0), params, full_range),
            Self::PowerIn {
                curvature,
                full_range,
            } => CurveIter::iter(PowerIn::new(curvature), params, full_range),
            Self::PowerOut {
                full_range,
                curvature,
            } => CurveIter::iter(PowerOut::new(curvature), params, full_range),
            Self::ExponentialIn { full_range } => {
                CurveIter::iter(ExponentialIn::new(), params, full_range)
            }
            Self::ExponentialOut { full_range } => {
                CurveIter::iter(ExponentialOut::new(), params, full_range)
            }
        }
    }

    fn hold_iter(time: Sample, t_from: Sample) -> CurveBox {
        CurveIter::iter(
            PowerIn::new(0.0),
            CurveIterParams {
                from: 1.0,
                to: 1.0,
                time,
                t_from,
            },
            true,
        )
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
    Hold(CurveBox),
    Decay(CurveBox),
    Sustain,
    Release(CurveBox),
    Done,
}

struct Voice {
    stage: Stage,
    triggered: bool,
    released: bool,
    output: ScalarOutput,
}

impl Default for Voice {
    fn default() -> Self {
        Self {
            stage: Stage::Done,
            triggered: false,
            released: false,
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

    gen_downcast_methods!();

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

    fn process_voice(
        env: &ChannelParams,
        voice: &mut Voice,
        current: bool,
        t_step: Sample,
        router: &VoiceRouter,
    ) {
        let attack_time = || (env.attack + router.scalar(Input::Attack, current)).max(0.0);
        let hold_time = || (env.hold + router.scalar(Input::Hold, current)).max(0.0);
        let decay_time = || (env.decay + router.scalar(Input::Decay, current)).max(0.0);
        let release_time = || (env.release + router.scalar(Input::Release, current)).max(0.0);

        if voice.released {
            voice.stage = Stage::Release(env.release_curve.curve_iter(
                voice.output.current(),
                0.0,
                release_time(),
                0.0,
            ));
            voice.released = false;
        }

        if voice.triggered {
            voice.stage = Stage::Attack(env.attack_curve.curve_iter(
                voice.output.current(),
                1.0,
                attack_time(),
                0.0,
            ));
        }

        voice.output.advance(loop {
            voice.stage = match &mut voice.stage {
                Stage::Attack(curve) => match curve.next(t_step, attack_time()) {
                    CurveResult::Value(value) => break value,
                    CurveResult::TimeRemainder(t_rem) => {
                        Stage::Hold(EnvelopeCurve::hold_iter(hold_time(), t_rem - t_step))
                    }
                },
                Stage::Hold(curve) => match curve.next(t_step, hold_time()) {
                    CurveResult::Value(value) => break value,
                    CurveResult::TimeRemainder(t_rem) => Stage::Decay(env.decay_curve.curve_iter(
                        1.0,
                        env.sustain,
                        decay_time(),
                        t_rem - t_step,
                    )),
                },
                Stage::Decay(curve) => match curve.next(t_step, decay_time()) {
                    CurveResult::Value(value) => break value,
                    CurveResult::TimeRemainder(_) => Stage::Sustain,
                },
                Stage::Sustain => {
                    break (env.sustain + router.scalar(Input::Sustain, current)).clamp(0.0, 1.0);
                }
                Stage::Release(curve) => match curve.next(t_step, release_time()) {
                    CurveResult::Value(value) => break value,
                    CurveResult::TimeRemainder(_) => Stage::Done,
                },
                Stage::Done => {
                    break 0.0;
                }
            };
        });
    }

    fn trigger_voice(voice: &mut Voice, reset: bool) {
        if reset {
            voice.output = ScalarOutput::default();
        }

        voice.triggered = true;
    }

    fn release_voice(voice: &mut Voice) {
        voice.released = true;
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

    fn inputs(&self) -> &'static [InputInfo] {
        static INPUTS: &[InputInfo] = &[
            InputInfo::scalar(Input::Attack),
            InputInfo::scalar(Input::Hold),
            InputInfo::scalar(Input::Decay),
            InputInfo::scalar(Input::Sustain),
            InputInfo::scalar(Input::Release),
        ];

        INPUTS
    }

    fn outputs(&self) -> &'static [DataType] {
        &[DataType::Scalar]
    }

    fn note_on(&mut self, params: &NoteOnParams) {
        for channel in &mut self.channels {
            Self::trigger_voice(&mut channel.voices[params.voice_idx], params.reset);
        }
    }

    fn note_off(&mut self, params: &NoteOffParams) {
        for channel in &mut self.channels {
            Self::release_voice(&mut channel.voices[params.voice_idx]);
        }
    }

    fn poll_alive_voices(&self, alive_state: &mut [VoiceAlive]) {
        if self.keep_voice_alive {
            for channel in &self.channels {
                for voice_alive in alive_state.iter_mut().filter(|alive| !alive.killed()) {
                    let voice = &channel.voices[voice_alive.index()];

                    voice_alive.mark_alive(!matches!(voice.stage, Stage::Done) || voice.triggered);
                }
            }
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
                    module_id: self.id,
                    samples: params.samples,
                    voice_idx: *voice_idx,
                    channel_idx,
                };

                if voice.triggered {
                    Self::process_voice(env, voice, false, 0.0, &router);
                    voice.triggered = false;
                }
                Self::process_voice(env, voice, true, t_step, &router);
            }
        }
    }

    fn get_scalar_output(&self, current: bool, voice_idx: usize, channel: usize) -> Sample {
        self.channels[channel].voices[voice_idx].output.get(current)
    }
}
