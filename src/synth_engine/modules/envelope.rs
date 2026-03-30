use std::any::Any;

use serde::{Deserialize, Serialize};

use crate::{
    synth_engine::{
        StereoSample,
        buffer::{Buffer, zero_buffer},
        curves::{CurveFunction, Exponential, ExponentialIn, ExponentialOut},
        routing::{DataType, Input, MAX_VOICES, ModuleId, ModuleType, NUM_CHANNELS, Router},
        smoother::Smoother,
        synth_module::{
            ModInput, ModuleConfigBox, NoteOffParams, NoteOnParams, ProcessParams, SynthModule,
            VoiceAlive, VoiceRouter,
        },
        types::{Sample, ScalarOutput},
    },
    utils::from_ms,
};

const MIN_TIME_THRESHOLD: Sample = from_ms(0.5);

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct Params {
    keep_voice_alive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelParams {
    delay: Sample,
    attack: Sample,
    attack_curve: EnvelopeCurve,
    hold: Sample,
    decay: Sample,
    decay_curve: EnvelopeCurve,
    sustain: Sample,
    release: Sample,
    release_curve: EnvelopeCurve,
    smooth: Sample,
}

impl Default for ChannelParams {
    fn default() -> Self {
        Self {
            delay: 0.0,
            attack: 0.0,
            attack_curve: EnvelopeCurve::Exponential { curvature: 0.3 },
            hold: 0.0,
            decay: from_ms(200.0),
            decay_curve: EnvelopeCurve::Exponential { curvature: 0.2 },
            sustain: 1.0,
            release: from_ms(300.0),
            release_curve: EnvelopeCurve::Exponential { curvature: 0.2 },
            smooth: 0.0,
        }
    }
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct EnvelopeConfig {
    label: Option<String>,
    params: Params,
    channels: [ChannelParams; NUM_CHANNELS],
}

enum CurveBlockResult {
    HasMore,
    Done,
}

trait CurveIterator {
    fn next_block(
        &mut self,
        t_step: Sample,
        time: Sample,
        sample_from: &mut usize,
        output: &mut [Sample],
    ) -> CurveBlockResult;
}

type CurveBox = Box<dyn CurveIterator + Send>;

struct CurveIterParams {
    from: Sample,
    to: Sample,
}
struct CurveIter<T: CurveFunction + Send> {
    curve_fn: T,
    t: Sample,
    value_from: Sample,
    interval: Sample,
}

impl<T: CurveFunction + Send + 'static> CurveIter<T> {
    fn iter(curve_fn: T, CurveIterParams { from, to }: CurveIterParams) -> CurveBox {
        let from = from.clamp(0.0, 1.0);
        let to = to.clamp(0.0, 1.0);

        Box::new(Self {
            curve_fn,
            t: 0.0,
            value_from: from,
            interval: to - from,
        })
    }
}

impl<T: CurveFunction + Send + 'static> CurveIterator for CurveIter<T> {
    fn next_block(
        &mut self,
        t_step: Sample,
        time: Sample,
        sample_from: &mut usize,
        output: &mut [Sample],
    ) -> CurveBlockResult {
        if time < MIN_TIME_THRESHOLD {
            return CurveBlockResult::Done;
        }

        let output = &mut output[*sample_from..];

        let samples = output
            .len()
            .min(((time - self.t).max(0.0) / t_step) as usize);

        for out in output.iter_mut().take(samples) {
            *out = self
                .interval
                .mul_add(self.curve_fn.calc(self.t / time), self.value_from);
            self.t += t_step;
        }

        *sample_from += samples;

        if samples < output.len() {
            CurveBlockResult::Done
        } else {
            CurveBlockResult::HasMore
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum EnvelopeCurve {
    Linear,
    Exponential { curvature: Sample },
    ExponentialIn,
    ExponentialOut,
}

impl EnvelopeCurve {
    fn curve_iter(&self, from: Sample, to: Sample) -> CurveBox {
        let params = CurveIterParams { from, to };

        match *self {
            Self::Linear => CurveIter::iter(Exponential::new(0.0), params),
            Self::Exponential { curvature } => CurveIter::iter(Exponential::new(curvature), params),
            Self::ExponentialIn => CurveIter::iter(ExponentialIn::new(), params),
            Self::ExponentialOut => CurveIter::iter(ExponentialOut::new(), params),
        }
    }

    fn delay_iter(level: Sample) -> CurveBox {
        CurveIter::iter(
            Exponential::new(0.0),
            CurveIterParams {
                from: level,
                to: level,
            },
        )
    }

    fn hold_iter() -> CurveBox {
        CurveIter::iter(
            Exponential::new(0.0),
            CurveIterParams { from: 1.0, to: 1.0 },
        )
    }

    fn flush_iter() -> CurveBox {
        CurveIter::iter(
            Exponential::new(0.0),
            CurveIterParams { from: 0.0, to: 0.0 },
        )
    }
}

pub struct EnvelopeUIData {
    pub label: String,
    pub delay: StereoSample,
    pub attack: StereoSample,
    pub attack_curve: EnvelopeCurve,
    pub hold: StereoSample,
    pub decay: StereoSample,
    pub decay_curve: EnvelopeCurve,
    pub sustain: StereoSample,
    pub release: StereoSample,
    pub release_curve: EnvelopeCurve,
    pub smooth: StereoSample,
    pub keep_voice_alive: bool,
}

enum Stage {
    Delay(CurveBox),
    Attack(CurveBox),
    Hold(CurveBox),
    Decay(CurveBox),
    Sustain,
    Release(CurveBox),
    Flush(CurveBox),
    Done,
}

struct Voice {
    stage: Stage,
    triggered: bool,
    released: bool,
    scalar_output: ScalarOutput,
    smoother: Smoother,
    output: Buffer,
}

impl Default for Voice {
    fn default() -> Self {
        Self {
            stage: Stage::Done,
            triggered: false,
            released: false,
            scalar_output: ScalarOutput::default(),
            smoother: Smoother::default(),
            output: zero_buffer(),
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
    params: Params,
    channels: [Channel; NUM_CHANNELS],
}

macro_rules! set_curve_method {
    ($fn_name:ident, $param:ident) => {
        pub fn $fn_name(&mut self, $param: EnvelopeCurve) {
            for channel in &mut self.channels {
                channel.params.$param = $param;
            }

            let mut cfg = self.config.lock();

            for channel in &mut cfg.channels {
                channel.$param = $param;
            }
        }
    };
}

impl Envelope {
    pub fn new(id: ModuleId, config: ModuleConfigBox<EnvelopeConfig>) -> Self {
        let mut env = Self {
            id,
            label: format!("Envelope {id}"),
            config,
            params: Params::default(),
            channels: Default::default(),
        };

        load_module_config!(env);
        env
    }

    gen_downcast_methods!();

    pub fn get_ui(&self) -> EnvelopeUIData {
        EnvelopeUIData {
            label: self.label.clone(),
            delay: get_stereo_param!(self, delay),
            attack: get_stereo_param!(self, attack),
            attack_curve: self.channels[0].params.attack_curve,
            hold: get_stereo_param!(self, hold),
            decay: get_stereo_param!(self, decay),
            decay_curve: self.channels[0].params.decay_curve,
            sustain: get_stereo_param!(self, sustain),
            release: get_stereo_param!(self, release),
            release_curve: self.channels[0].params.release_curve,
            smooth: get_stereo_param!(self, smooth),
            keep_voice_alive: self.params.keep_voice_alive,
        }
    }

    set_mono_param!(set_keep_voice_alive, keep_voice_alive, bool);

    set_curve_method!(set_attack_curve, attack_curve);
    set_curve_method!(set_decay_curve, decay_curve);
    set_curve_method!(set_release_curve, release_curve);

    set_stereo_param!(set_delay, delay);
    set_stereo_param!(set_attack, attack);
    set_stereo_param!(set_hold, hold);
    set_stereo_param!(set_decay, decay);
    set_stereo_param!(set_sustain, sustain);
    set_stereo_param!(set_release, release);
    set_stereo_param!(set_smooth, smooth);

    fn process_voice_buffer(
        env: &ChannelParams,
        voice: &mut Voice,
        t_step: Sample,
        router: &VoiceRouter,
    ) {
        let delay_time = || env.delay + router.scalar(Input::Delay, true);
        let attack_time = || env.attack + router.scalar(Input::Attack, true);
        let hold_time = || env.hold + router.scalar(Input::Hold, true);
        let decay_time = || env.decay + router.scalar(Input::Decay, true);
        let release_time = || env.release + router.scalar(Input::Release, true);

        if voice.released {
            voice.stage = Stage::Release(
                env.release_curve
                    .curve_iter(voice.scalar_output.current(), 0.0),
            );
            voice.released = false;
        }

        if voice.triggered {
            voice.stage = Stage::Delay(EnvelopeCurve::delay_iter(voice.scalar_output.current()));
        }

        let mut sample_from = if voice.triggered { 0 } else { 1 };
        let output = &mut voice.output[..router.samples + 1];

        loop {
            voice.stage = match &mut voice.stage {
                Stage::Delay(curve) => {
                    match curve.next_block(t_step, delay_time(), &mut sample_from, output) {
                        CurveBlockResult::Done => Stage::Attack(
                            env.attack_curve
                                .curve_iter(voice.scalar_output.current(), 1.0),
                        ),
                        CurveBlockResult::HasMore => break,
                    }
                }
                Stage::Attack(curve) => {
                    match curve.next_block(t_step, attack_time(), &mut sample_from, output) {
                        CurveBlockResult::Done => Stage::Hold(EnvelopeCurve::hold_iter()),
                        CurveBlockResult::HasMore => break,
                    }
                }
                Stage::Hold(curve) => {
                    match curve.next_block(t_step, hold_time(), &mut sample_from, output) {
                        CurveBlockResult::Done => {
                            Stage::Decay(env.decay_curve.curve_iter(1.0, env.sustain))
                        }
                        CurveBlockResult::HasMore => break,
                    }
                }
                Stage::Decay(curve) => {
                    match curve.next_block(t_step, decay_time(), &mut sample_from, output) {
                        CurveBlockResult::Done => Stage::Sustain,
                        CurveBlockResult::HasMore => break,
                    }
                }
                Stage::Sustain => {
                    output[sample_from..]
                        .fill((env.sustain + router.scalar(Input::Sustain, true)).clamp(0.0, 1.0));
                    break;
                }
                Stage::Release(curve) => {
                    match curve.next_block(t_step, release_time(), &mut sample_from, output) {
                        CurveBlockResult::Done => Stage::Flush(EnvelopeCurve::flush_iter()),
                        CurveBlockResult::HasMore => break,
                    }
                }
                Stage::Flush(curve) => {
                    match curve.next_block(t_step, env.smooth, &mut sample_from, output) {
                        CurveBlockResult::Done => Stage::Done,
                        CurveBlockResult::HasMore => break,
                    }
                }
                Stage::Done => {
                    output[sample_from..].fill(0.0);
                    break;
                }
            };
        }

        if voice.triggered {
            voice.scalar_output.advance(voice.output[0]);
            voice.scalar_output.advance(voice.output[router.samples]);
            voice.triggered = false;
        } else {
            voice.output[0] = voice.scalar_output.current();
            voice.scalar_output.advance(voice.output[router.samples]);
        }

        if env.smooth >= MIN_TIME_THRESHOLD {
            voice.smoother.update(router.sample_rate, env.smooth);

            for sample in voice.output.iter_mut().take(router.samples) {
                *sample = voice.smoother.tick(*sample);
            }
        }
    }

    fn trigger_voice(voice: &mut Voice, reset: bool) {
        if reset {
            voice.scalar_output = ScalarOutput::default();
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

    fn inputs(&self) -> &'static [ModInput] {
        static INPUTS: &[ModInput] = &[
            ModInput::scalar(Input::Delay),
            ModInput::scalar(Input::Attack),
            ModInput::scalar(Input::Hold),
            ModInput::scalar(Input::Decay),
            ModInput::scalar(Input::Sustain),
            ModInput::scalar(Input::Release),
        ];

        INPUTS
    }

    fn output(&self) -> DataType {
        DataType::Scalar
    }

    fn note_on(&mut self, params: &NoteOnParams) {
        for channel in &mut self.channels {
            let voice = &mut channel.voices[params.voice_idx];

            Self::trigger_voice(voice, params.reset);

            if params.reset {
                voice.smoother.reset(0.0);
            }
        }
    }

    fn note_off(&mut self, params: &NoteOffParams) {
        for channel in &mut self.channels {
            Self::release_voice(&mut channel.voices[params.voice_idx]);
        }
    }

    fn poll_alive_voices(&self, alive_state: &mut [VoiceAlive]) {
        if self.params.keep_voice_alive {
            for voice_alive in alive_state.iter_mut().filter(|va| !va.alive()) {
                for channel in &self.channels {
                    let voice = &channel.voices[voice_alive.index()];

                    voice_alive.mark_alive(!matches!(voice.stage, Stage::Done) || voice.triggered);
                }
            }
        }
    }

    fn process(&mut self, params: &ProcessParams, router: &dyn Router) {
        let t_step = params.sample_rate.recip();

        for (channel_idx, channel) in self.channels.iter_mut().enumerate() {
            let env = &channel.params;

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

                Self::process_voice_buffer(env, voice, t_step, &router);
            }
        }
    }

    fn get_buffer_output(&self, voice_idx: usize, channel_idx: usize) -> &Buffer {
        &self.channels[channel_idx].voices[voice_idx].output
    }

    fn get_scalar_output(&self, current: bool, voice_idx: usize, channel: usize) -> Sample {
        self.channels[channel].voices[voice_idx]
            .scalar_output
            .get(current)
    }
}
