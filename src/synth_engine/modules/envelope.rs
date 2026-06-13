use std::array;

mod config;
mod link;
mod ui_bridge;

pub use config::{EnvelopeConfig, EnvelopeCurve};
use link::{AudioEnd, UiEnd, UiEvent, create_link_pair};
pub use ui_bridge::EnvelopeUiBridge;

use crate::{
    synth_engine::{
        StereoSample,
        buffer::{Buffer, new_voices_layout, zero_buffer},
        curves::{CurveFunction, Exponential, ExponentialIn, ExponentialOut},
        routing::{
            DataType, Input, MAX_VOICES, ModuleId, ModuleType, NUM_CHANNELS, Router, VoiceEvent,
        },
        smooth::Smoother,
        synth_module::{ModInput, ProcessParams, SynthModule, VoiceRouter, VoiceRouterFactory},
        types::{Sample, ScalarOutput},
        voices_handler::DecayingVoice,
    },
    utils::from_ms,
};

const MIN_TIME_THRESHOLD: Sample = from_ms(0.5);

struct Params {
    keep_voice_alive: bool,
    attack_curve: EnvelopeCurve,
    decay_curve: EnvelopeCurve,
    release_curve: EnvelopeCurve,
}

impl Params {
    fn from_config(c: &config::EnvelopeConfig) -> Self {
        Self {
            keep_voice_alive: c.keep_voice_alive,
            attack_curve: c.attack_curve,
            decay_curve: c.decay_curve,
            release_curve: c.release_curve,
        }
    }
}

struct ChannelParams {
    delay: Sample,
    attack: Sample,
    hold: Sample,
    decay: Sample,
    sustain: Sample,
    release: Sample,
    smooth: Sample,
}

impl ChannelParams {
    fn from_config(c: &EnvelopeConfig, channel_idx: usize) -> Self {
        Self {
            delay: c.delay[channel_idx],
            attack: c.attack[channel_idx],
            hold: c.hold[channel_idx],
            decay: c.decay[channel_idx],
            sustain: c.sustain[channel_idx],
            release: c.release[channel_idx],
            smooth: c.smooth[channel_idx],
        }
    }
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
        let time_recip = time.recip();

        for out in output.iter_mut().take(samples) {
            *out = self
                .interval
                .mul_add(self.curve_fn.calc(self.t * time_recip), self.value_from);
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

type ChannelVoices = [Voice; MAX_VOICES];

pub struct Envelope {
    id: ModuleId,
    params: Params,
    channel_params: [ChannelParams; NUM_CHANNELS],
    audio_end: AudioEnd,
    ui_end: Option<UiEnd>,
    voices: Box<[ChannelVoices; NUM_CHANNELS]>,
}

impl Envelope {
    pub fn new(id: ModuleId) -> Self {
        Self::from_config(&EnvelopeConfig {
            id,
            ..EnvelopeConfig::default()
        })
    }

    pub fn from_config(config: &config::EnvelopeConfig) -> Self {
        let (audio_end, ui_end) = create_link_pair();

        Self {
            id: config.id,
            params: Params::from_config(config),
            channel_params: array::from_fn(|channel_idx| {
                ChannelParams::from_config(config, channel_idx)
            }),
            audio_end,
            ui_end: Some(ui_end),
            voices: new_voices_layout(),
        }
    }

    pub fn get_config(&self) -> EnvelopeConfig {
        EnvelopeConfig {
            id: self.id,
            keep_voice_alive: self.params.keep_voice_alive,
            delay: get_stereo_param!(self, delay),
            attack: get_stereo_param!(self, attack),
            attack_curve: self.params.attack_curve,
            hold: get_stereo_param!(self, hold),
            decay: get_stereo_param!(self, decay),
            decay_curve: self.params.decay_curve,
            sustain: get_stereo_param!(self, sustain),
            release: get_stereo_param!(self, release),
            release_curve: self.params.release_curve,
            smooth: get_stereo_param!(self, smooth),
        }
    }

    set_mono_param!(set_keep_voice_alive, keep_voice_alive, bool);
    set_mono_param!(set_attack_curve, attack_curve, EnvelopeCurve);
    set_mono_param!(set_decay_curve, decay_curve, EnvelopeCurve);
    set_mono_param!(set_release_curve, release_curve, EnvelopeCurve);

    set_stereo_param!(set_delay, delay);
    set_stereo_param!(set_attack, attack);
    set_stereo_param!(set_hold, hold);
    set_stereo_param!(set_decay, decay);
    set_stereo_param!(set_sustain, sustain);
    set_stereo_param!(set_release, release);
    set_stereo_param!(set_smooth, smooth);

    fn process_voice_buffer(&mut self, t_step: Sample, mut router: VoiceRouter<'_, '_>) {
        let params = &self.params;
        let channel = &mut self.channel_params[router.channel_idx()];
        let voice = &mut self.voices[router.channel_idx()][router.voice_idx()];

        if voice.triggered {
            voice.scalar_output = ScalarOutput::default();
            voice.smoother.reset(0.0);
            voice.stage = Stage::Delay(EnvelopeCurve::delay_iter(voice.scalar_output.current()));
        }

        if voice.released {
            voice.stage = Stage::Release(
                params
                    .release_curve
                    .curve_iter(voice.scalar_output.current(), 0.0),
            );
            voice.released = false;
        }

        let mut sample_from = if voice.triggered { 0 } else { 1 };
        let output = &mut voice.output[..router.samples() + 1];

        loop {
            voice.stage = match &mut voice.stage {
                Stage::Delay(curve) => {
                    match curve.next_block(
                        t_step,
                        router.scalar(Input::Delay, channel.delay, true),
                        &mut sample_from,
                        output,
                    ) {
                        CurveBlockResult::Done => Stage::Attack(
                            params
                                .attack_curve
                                .curve_iter(voice.scalar_output.current(), 1.0),
                        ),
                        CurveBlockResult::HasMore => break,
                    }
                }
                Stage::Attack(curve) => {
                    match curve.next_block(
                        t_step,
                        router.scalar(Input::Attack, channel.attack, true),
                        &mut sample_from,
                        output,
                    ) {
                        CurveBlockResult::Done => Stage::Hold(EnvelopeCurve::hold_iter()),
                        CurveBlockResult::HasMore => break,
                    }
                }
                Stage::Hold(curve) => {
                    match curve.next_block(
                        t_step,
                        router.scalar(Input::Hold, channel.hold, true),
                        &mut sample_from,
                        output,
                    ) {
                        CurveBlockResult::Done => {
                            Stage::Decay(params.decay_curve.curve_iter(1.0, channel.sustain))
                        }
                        CurveBlockResult::HasMore => break,
                    }
                }
                Stage::Decay(curve) => {
                    match curve.next_block(
                        t_step,
                        router.scalar(Input::Decay, channel.decay, true),
                        &mut sample_from,
                        output,
                    ) {
                        CurveBlockResult::Done => Stage::Sustain,
                        CurveBlockResult::HasMore => break,
                    }
                }
                Stage::Sustain => {
                    output[sample_from..].fill(
                        (router.scalar(Input::Sustain, channel.sustain, true)).clamp(0.0, 1.0),
                    );
                    break;
                }
                Stage::Release(curve) => {
                    match curve.next_block(
                        t_step,
                        router.scalar(Input::Release, channel.release, true),
                        &mut sample_from,
                        output,
                    ) {
                        CurveBlockResult::Done => Stage::Flush(EnvelopeCurve::flush_iter()),
                        CurveBlockResult::HasMore => break,
                    }
                }
                Stage::Flush(curve) => {
                    match curve.next_block(t_step, channel.smooth, &mut sample_from, output) {
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
            voice.scalar_output.advance(voice.output[router.samples()]);
            voice.triggered = false;
        } else {
            voice.output[0] = voice.scalar_output.current();
            voice.scalar_output.advance(voice.output[router.samples()]);
        }

        if channel.smooth >= MIN_TIME_THRESHOLD {
            voice.smoother.update(router.sample_rate(), channel.smooth);

            for sample in voice.output.iter_mut().take(router.samples()) {
                *sample = voice.smoother.tick(*sample);
            }
        }
    }

    fn trigger_voice(voice: &mut Voice) {
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
        DataType::Control
    }

    fn handle_events(&mut self, events: &[VoiceEvent]) {
        for channel in self.voices.iter_mut() {
            for event in events {
                match event {
                    VoiceEvent::Trigger { voice_idx, .. } => {
                        Self::trigger_voice(&mut channel[*voice_idx]);
                    }
                    VoiceEvent::Release { voice_idx, .. } => {
                        Self::release_voice(&mut channel[*voice_idx]);
                    }
                    _ => (),
                }
            }
        }
    }

    fn poll_decaying_voices(&self, decaying_voices: &mut [DecayingVoice]) {
        if self.params.keep_voice_alive {
            for decaying in decaying_voices.iter_mut().filter(|d| d.is_done()) {
                for channel in self.voices.iter() {
                    let voice = &channel[decaying.index()];

                    if !matches!(voice.stage, Stage::Done) || voice.triggered {
                        decaying.mark_active();
                    }
                }
            }
        }
    }

    fn handle_ui_events(&mut self) {
        while let Some(event) = self.audio_end.pop_event() {
            match event {
                UiEvent::InputParam { input, value } => match input {
                    Input::Delay => self.set_delay(value),
                    Input::Attack => self.set_attack(value),
                    Input::Hold => self.set_hold(value),
                    Input::Decay => self.set_decay(value),
                    Input::Sustain => self.set_sustain(value),
                    Input::Release => self.set_release(value),
                    _ => (),
                },
                UiEvent::Smooth(value) => self.set_smooth(value),
                UiEvent::AttackCurve(curve) => self.set_attack_curve(curve),
                UiEvent::DecayCurve(curve) => self.set_decay_curve(curve),
                UiEvent::ReleaseCurve(curve) => self.set_release_curve(curve),
                UiEvent::KeepVoiceAlive(value) => self.set_keep_voice_alive(value),
            }
        }
    }

    fn process(&mut self, params: &ProcessParams, router: &mut dyn Router) {
        let t_step = params.sample_rate.recip();
        let mut rf = VoiceRouterFactory::new(self.id, router, params);

        for channel_idx in 0..NUM_CHANNELS {
            for (seq_idx, voice_idx) in params.active_voices.iter().enumerate() {
                self.process_voice_buffer(t_step, rf.for_voice(*voice_idx, channel_idx, seq_idx));
            }
        }
    }

    fn get_buffer_output(&self, voice_idx: usize, channel_idx: usize) -> &Buffer {
        &self.voices[channel_idx][voice_idx].output
    }

    fn get_scalar_output(&self, current: bool, voice_idx: usize, channel: usize) -> Sample {
        self.voices[channel][voice_idx].scalar_output.get(current)
    }
}
