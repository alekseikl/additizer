use std::{array, mem, sync::Arc};

use itertools::izip;
use rand::RngExt;
use rand_pcg::Pcg32;
use realfft::{ComplexToReal, RealFftPlanner};
use smallvec::SmallVec;
use wide::f32x4;

use crate::{
    synth_engine::{
        SmoothedSampleParams, StereoSample,
        buffer::{Buffer, SPECTRUM_BITS, VoicesLayout, new_voices_layout, zero_buffer},
        phase::{Phase, PhaseX4},
        routing::{
            AudioRouterType, DataType, Input, InputMeta, InputSlots, LEFT_CHANNEL, MAX_VOICES,
            MixedSlots, ModuleId, NUM_CHANNELS, ProcessContext, RIGHT_CHANNEL, RouterFactory,
            SamplesOutput, VoiceEvent, VoiceRouter, VoiceTarget,
        },
        smooth::SmoothedSample,
        synth_module::SynthModule,
        types::{ComplexSample, Sample},
    },
    synth_engine::{db_to_gain, fast_pitch_to_freq_x4, from_st, pitch_to_freq, zip_map_x4},
};

mod config;
mod lanes;
mod link;
pub mod stub;
mod unison;

use lanes::{SampleCtx, UNISON_CHUNKS, UNISON_LANES, UnisonLaneParams};
use unison::{ConvexUnison, FlatUnison, ManualUnison, UnisonType};

#[cfg(test)]
pub(crate) mod tests;

pub use config::{OscillatorConfig, UnisonStyle};
pub use link::{OscillatorAudioEnd, OscillatorLinks, OscillatorUiEnd, UiEvent, Unison};

const WAVEFORM_BITS: usize = SPECTRUM_BITS + 1;
const HALF_WAVEFORM_BITS: usize = WAVEFORM_BITS - 1;
const WAVEFORM_SIZE: usize = 1 << WAVEFORM_BITS;
const WAVEFORM_PAD_LEFT: usize = 1;
const WAVEFORM_PAD_RIGHT: usize = 2;
const WAVEFORM_BUFFER_SIZE: usize = WAVEFORM_SIZE + WAVEFORM_PAD_LEFT + WAVEFORM_PAD_RIGHT;
const DFT_BUFFER_SIZE: usize = (1 << (WAVEFORM_BITS - 1)) + 1;

pub const MAX_UNISON_VOICES: usize = 16;

type WaveformBuffer = [Sample; WAVEFORM_BUFFER_SIZE];
type DftBuffer = [ComplexSample; DFT_BUFFER_SIZE];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WaveformSize {
    Full,
    Half,
}

impl WaveformSize {
    const fn len(self) -> usize {
        match self {
            Self::Full => WAVEFORM_SIZE,
            Self::Half => WAVEFORM_SIZE / 2,
        }
    }
}

struct Waveform {
    samples: Box<WaveformBuffer>,
    size: WaveformSize,
}

impl Waveform {
    fn new() -> Self {
        Self {
            samples: Box::new([0.0; WAVEFORM_BUFFER_SIZE]),
            size: WaveformSize::Full,
        }
    }
}

struct IfftPlanners {
    full: Arc<dyn ComplexToReal<Sample>>,
    half: Arc<dyn ComplexToReal<Sample>>,
}

impl IfftPlanners {
    fn new() -> Self {
        let mut planner = RealFftPlanner::<Sample>::new();

        Self {
            full: planner.plan_fft_inverse(WAVEFORM_SIZE),
            half: planner.plan_fft_inverse(WAVEFORM_SIZE / 2),
        }
    }

    fn select(&self, cutoff_index: usize) -> (WaveformSize, &dyn ComplexToReal<Sample>) {
        if cutoff_index < self.half.complex_len() {
            (WaveformSize::Half, self.half.as_ref())
        } else {
            (WaveformSize::Full, self.full.as_ref())
        }
    }
}

struct Params {
    unison: usize,
    unison_style: UnisonStyle,
    unison_stereo: Sample, // [0.0, 1.0]
    steal_phase: bool,
    phase_random: Sample,        // [0.0, 1.0]
    phase_random_stereo: Sample, // [0.0, 1.0]
    mono_spectrum: bool,
}

impl Params {
    fn from_config(c: &config::OscillatorConfig) -> Self {
        Self {
            unison: c.unison_voices,
            unison_style: c.unison_style,
            unison_stereo: c.unison_stereo,
            steal_phase: c.steal_phase,
            phase_random: c.phase_random,
            phase_random_stereo: c.phase_random_stereo,
            mono_spectrum: c.mono_spectrum,
        }
    }
}

struct UnisonParams {
    initial_phase: Sample,
    phase_shift: Sample,
    phase_shift_to: Sample,
    /// Hand-set level. Read by [`UnisonStyle::Manual`] only.
    gain: Sample,
    /// Hand-set level at full gains blend. Read by [`UnisonStyle::Manual`] only.
    gain_to: Sample,
}

impl Default for UnisonParams {
    fn default() -> Self {
        Self {
            initial_phase: 0.0,
            phase_shift: 0.0,
            phase_shift_to: 0.0,
            gain: 1.0,
            gain_to: 1.0,
        }
    }
}

struct ChannelParams {
    detune: Sample, // Octaves
    detune_focus: Sample,
    phase_shift: SmoothedSample,
    frequency_shift: SmoothedSample,
    phases_blend: Sample,
    gains_blend: Sample,
    unison: [UnisonParams; MAX_UNISON_VOICES],
}

impl ChannelParams {
    fn from_config(c: &OscillatorConfig, channel_idx: usize) -> Self {
        Self {
            detune: c.detune[channel_idx],
            detune_focus: c.detune_focus[channel_idx],
            phase_shift: c.phase_shift[channel_idx].into(),
            frequency_shift: c.frequency_shift[channel_idx].into(),
            phases_blend: c.phases_blend[channel_idx],
            gains_blend: c.gains_blend[channel_idx],
            unison: array::from_fn(|i| UnisonParams {
                initial_phase: c.unison[i].initial_phase[channel_idx],
                phase_shift: c.unison[i].phase_shift[channel_idx],
                phase_shift_to: c.unison[i].phase_shift_to[channel_idx],
                gain: c.unison[i].gain[channel_idx],
                gain_to: c.unison[i].gain_to[channel_idx],
            }),
        }
    }

    pub fn advance_smoothers(&mut self, smooth_params: &SmoothedSampleParams, samples: usize) {
        self.phase_shift.advance(smooth_params, samples);
        self.frequency_shift.advance(smooth_params, samples);
    }
}

struct Interpolated {
    from: Sample,
    to: Sample,
}

impl Interpolated {
    #[inline(always)]
    fn advance(&mut self) {
        self.from = self.to;
    }
}

struct PhaseReset {
    steal_from: Option<usize>,
    stolen: bool,
}

struct PhaseSteal {
    offset: u16,
    voice_idx: u16,
}

struct VoiceRenderCtx {
    channel_idx: usize,
    voice_idx: usize,
    wave_channel: usize,
    samples: usize,
}

struct UnisonVoice {
    rate: Interpolated,
    phase_shift: Interpolated,
    gain: Interpolated,
}

impl Default for UnisonVoice {
    fn default() -> Self {
        Self {
            rate: Interpolated { from: 1.0, to: 1.0 },
            phase_shift: Interpolated { from: 0.0, to: 0.0 },
            gain: Interpolated { from: 1.0, to: 1.0 },
        }
    }
}

#[derive(Default)]
struct Voice {
    phase_reset: Option<PhaseReset>,
    unison: [UnisonVoice; MAX_UNISON_VOICES],
    phases: [Phase; MAX_UNISON_VOICES],
}

struct VoiceBuffers {
    wave: Waveform,
}

impl Default for VoiceBuffers {
    fn default() -> Self {
        Self {
            wave: Waveform::new(),
        }
    }
}

struct Buffers {
    tmp_wave: Waveform,
    tmp_spectral: DftBuffer,
    scratch: DftBuffer,
    phase_inc: Buffer,
    phase_shift: Buffer,
    frequency_shift: Buffer,
}

impl Default for Buffers {
    fn default() -> Self {
        Self {
            tmp_wave: Waveform::new(),
            tmp_spectral: [ComplexSample::ZERO; DFT_BUFFER_SIZE],
            scratch: [ComplexSample::ZERO; DFT_BUFFER_SIZE],
            phase_inc: zero_buffer(),
            phase_shift: zero_buffer(),
            frequency_shift: zero_buffer(),
        }
    }
}

macro_rules! set_unison_param {
    ($fn_name:ident, $param:ident) => {
        set_unison_param!($fn_name, $param, *$param);
    };
    ($fn_name:ident, $param:ident, $transform:expr) => {
        pub fn $fn_name(&mut self, voice_idx: usize, $param: StereoSample) {
            for (channel, $param) in self.channel_params.iter_mut().zip($param.iter()) {
                channel.unison[voice_idx].$param = $transform;
            }
        }
    };
}

macro_rules! get_unison_param {
    ($self:ident, $param:ident, $voice_idx:expr) => {
        StereoSample::from_iter(
            $self
                .channel_params
                .iter()
                .map(|channel| channel.unison[$voice_idx].$param),
        )
    };
}

#[derive(Clone, Copy)]
pub enum PhasesDst {
    Initial,
    From,
    To,
}

pub struct Inputs {
    spectrum: Option<usize>,
    pitch: Option<usize>,
    phase_shift: MixedSlots,
    freq_shift: MixedSlots,
    detune: MixedSlots,
    detune_focus: MixedSlots,
    phase_steal: MixedSlots,
    phases_blend: MixedSlots,
    gains_blend: MixedSlots,
}

impl Default for Inputs {
    fn default() -> Self {
        Self {
            spectrum: None,
            pitch: None,
            phase_shift: MixedSlots::new(Input::PhaseShift),
            freq_shift: MixedSlots::new(Input::FrequencyShift),
            detune: MixedSlots::new(Input::Detune),
            detune_focus: MixedSlots::new(Input::DetuneFocus),
            phase_steal: MixedSlots::new(Input::PhaseSteal),
            phases_blend: MixedSlots::new(Input::PhasesBlend),
            gains_blend: MixedSlots::new(Input::GainsBlend),
        }
    }
}

impl Inputs {
    fn from_slots(inputs: &[InputSlots]) -> Self {
        let mut result = Self::default();

        for input in inputs {
            match input {
                InputSlots::Direct { input_type, slot } => {
                    if matches!(input_type, Input::Pitch) {
                        result.pitch = Some(*slot);
                    }
                }
                InputSlots::Spectral { input_type, slot } => {
                    if matches!(input_type, Input::Spectrum) {
                        result.spectrum = Some(*slot);
                    }
                }
                InputSlots::Mixed(input) => match input.input_type {
                    Input::PhaseShift => result.phase_shift = input.clone(),
                    Input::FrequencyShift => result.freq_shift = input.clone(),
                    Input::Detune => result.detune = input.clone(),
                    Input::DetuneFocus => result.detune_focus = input.clone(),
                    Input::PhaseSteal => result.phase_steal = input.clone(),
                    Input::PhasesBlend => result.phases_blend = input.clone(),
                    Input::GainsBlend => result.gains_blend = input.clone(),
                    _ => (),
                },
            }
        }

        result
    }

    fn update_amount(&mut self, input_type: Input, src_slot: usize, amount: StereoSample) {
        match input_type {
            Input::PhaseShift => self.phase_shift.update_amount(src_slot, amount),
            Input::FrequencyShift => self.freq_shift.update_amount(src_slot, amount),
            Input::Detune => self.detune.update_amount(src_slot, amount),
            Input::DetuneFocus => self.detune_focus.update_amount(src_slot, amount),
            Input::PhaseSteal => self.phase_steal.update_amount(src_slot, amount),
            Input::PhasesBlend => self.phases_blend.update_amount(src_slot, amount),
            Input::GainsBlend => self.gains_blend.update_amount(src_slot, amount),
            _ => (),
        }
    }
}

type Router<'v, 'f, 'c, A> = VoiceRouter<'v, 'f, 'c, AudioRouterType, A>;

pub struct Oscillator<L: OscillatorLinks = stub::Links> {
    buffers: Buffers,
    ifft: IfftPlanners,
    random: Pcg32,
    id: ModuleId,
    params: Params,
    channel_params: [ChannelParams; NUM_CHANNELS],
    audio_end: L::AudioEnd,
    ui_end: Option<L::UiEnd>,
    inputs: Inputs,
    output_slot: usize,
    voices: VoicesLayout<Voice>,
    voice_buffers: VoicesLayout<VoiceBuffers>,
    lane_params: [UnisonLaneParams; UNISON_CHUNKS],
    shared_random_phases: [Phase; MAX_UNISON_VOICES],
}

impl<L: OscillatorLinks> Oscillator<L> {
    pub fn take_ui_end(&mut self) -> Option<L::UiEnd> {
        self.ui_end.take()
    }

    pub fn new(id: ModuleId) -> Self {
        Self::from_config(&OscillatorConfig {
            id,
            ..OscillatorConfig::default()
        })
    }

    pub fn from_config(config: &config::OscillatorConfig) -> Self {
        let (audio_end, ui_end) = L::create_link_pair();

        let mut osc = Self {
            id: config.id,
            params: Params::from_config(config),
            channel_params: array::from_fn(|channel_idx| {
                ChannelParams::from_config(config, channel_idx)
            }),
            buffers: Buffers::default(),
            ifft: IfftPlanners::new(),
            random: Pcg32::new(420, 1337),
            audio_end,
            ui_end,
            inputs: Inputs::default(),
            output_slot: usize::MAX,
            voices: new_voices_layout(),
            voice_buffers: new_voices_layout(),
            lane_params: Default::default(),
            shared_random_phases: [Phase::ZERO; MAX_UNISON_VOICES],
        };

        osc.publish_unison();
        osc
    }

    fn unison_snapshot(&self) -> Unison {
        Unison {
            initial_phases: array::from_fn(|i| get_unison_param!(self, initial_phase, i)),
            phase_shifts: array::from_fn(|i| get_unison_param!(self, phase_shift, i)),
            phase_shifts_to: array::from_fn(|i| get_unison_param!(self, phase_shift_to, i)),
            gains: array::from_fn(|i| get_unison_param!(self, gain, i)),
            gains_to: array::from_fn(|i| get_unison_param!(self, gain_to, i)),
        }
    }

    fn unison_config(&self) -> [config::UnisonConfig; MAX_UNISON_VOICES] {
        let unison = self.unison_snapshot();

        array::from_fn(|i| config::UnisonConfig {
            initial_phase: unison.initial_phases[i],
            phase_shift: unison.phase_shifts[i],
            phase_shift_to: unison.phase_shifts_to[i],
            gain: unison.gains[i],
            gain_to: unison.gains_to[i],
        })
    }

    fn publish_unison(&mut self) {
        self.audio_end.publish_unison(&self.unison_snapshot());
    }

    pub fn get_config(&self) -> OscillatorConfig {
        OscillatorConfig {
            id: self.id,
            unison_voices: self.params.unison,
            unison_style: self.params.unison_style,
            unison_stereo: self.params.unison_stereo,
            steal_phase: self.params.steal_phase,
            phase_random: self.params.phase_random,
            phase_random_stereo: self.params.phase_random_stereo,
            mono_spectrum: self.params.mono_spectrum,
            detune: get_stereo_param!(self, detune),
            detune_focus: get_stereo_param!(self, detune_focus),
            phase_shift: get_smoothed_param!(self, phase_shift),
            frequency_shift: get_smoothed_param!(self, frequency_shift),
            phases_blend: get_stereo_param!(self, phases_blend),
            gains_blend: get_stereo_param!(self, gains_blend),
            unison: self.unison_config(),
        }
    }

    set_mono_param!(
        set_unison,
        unison,
        usize,
        unison.clamp(1, MAX_UNISON_VOICES)
    );
    set_mono_param!(set_unison_style, unison_style, UnisonStyle);
    set_mono_param!(
        set_unison_stereo,
        unison_stereo,
        Sample,
        unison_stereo.clamp(0.0, 1.0)
    );
    set_mono_param!(set_steal_phase, steal_phase, bool);
    set_mono_param!(
        set_phase_random,
        phase_random,
        Sample,
        phase_random.clamp(0.0, 1.0)
    );
    set_mono_param!(
        set_phase_random_stereo,
        phase_random_stereo,
        Sample,
        phase_random_stereo.clamp(0.0, 1.0)
    );
    set_mono_param!(set_mono_spectrum, mono_spectrum, bool);

    set_stereo_param!(set_detune, detune, detune.clamp(0.0, from_st(1.0)));
    set_stereo_param!(
        set_detune_focus,
        detune_focus,
        detune_focus.clamp(-1.0, 1.0)
    );

    set_smoothed_param!(set_phase_shift, phase_shift, phase_shift.clamp(-1.0, 1.0));
    set_smoothed_param!(set_frequency_shift, frequency_shift);

    set_stereo_param!(set_phases_blend, phases_blend, phases_blend.clamp(0.0, 1.0));
    set_stereo_param!(set_gains_blend, gains_blend, gains_blend.clamp(0.0, 1.0));

    set_unison_param!(
        set_initial_phase,
        initial_phase,
        initial_phase.clamp(-1.0, 1.0)
    );
    set_unison_param!(set_unison_phase, phase_shift, phase_shift.clamp(-1.0, 1.0));
    set_unison_param!(
        set_unison_phase_to,
        phase_shift_to,
        phase_shift_to.clamp(-1.0, 1.0)
    );

    set_unison_param!(set_unison_gain, gain, gain.clamp(0.0, 10.0));
    set_unison_param!(set_unison_gain_to, gain_to, gain_to.clamp(0.0, 10.0));

    pub fn apply_unison_level_shape(
        &mut self,
        center: StereoSample,
        level: StereoSample,
        to: bool,
    ) {
        if self.params.unison < 2 {
            return;
        }

        for (center, edge_level, channel) in
            izip!(center.iter(), level.iter(), self.channel_params.iter_mut())
        {
            let center = center.clamp(0.0, 1.0);
            let step = ((self.params.unison - 1) as Sample).recip();

            for (idx, unison_params) in channel
                .unison
                .iter_mut()
                .enumerate()
                .take(self.params.unison)
            {
                let gain = if to {
                    &mut unison_params.gain_to
                } else {
                    &mut unison_params.gain
                };

                let pos = idx as Sample * step;

                let level = if pos < center {
                    let t = pos / center;
                    edge_level + (-edge_level) * t
                } else {
                    let t = (pos - center) / (1.0 - center + f32::EPSILON);
                    edge_level * t
                };

                *gain = db_to_gain(level);
            }
        }

        self.publish_unison();
    }

    pub fn randomize_phases(&mut self, amount: Sample, stereo_spread: Sample, dst: PhasesDst) {
        let amount = amount.clamp(0.0, 1.0);
        let stereo_spread = stereo_spread.clamp(0.0, 1.0);

        let randoms: [StereoSample; MAX_UNISON_VOICES] = array::from_fn(|_| {
            let center = amount * (self.random.random::<Sample>() - 0.5);
            let stereo_amount = amount * stereo_spread;
            let left = center + stereo_amount * (self.random.random::<Sample>() - 0.5);
            let right = center + stereo_amount * (self.random.random::<Sample>() - 0.5);

            StereoSample::new(left, right).map(|phase| phase.rem_euclid(1.0))
        });

        for (channel_idx, channel) in self.channel_params.iter_mut().enumerate() {
            for (unison, random) in channel.unison.iter_mut().zip(randoms) {
                let phase = match dst {
                    PhasesDst::Initial => &mut unison.initial_phase,
                    PhasesDst::From => &mut unison.phase_shift,
                    PhasesDst::To => &mut unison.phase_shift_to,
                };

                *phase = random[channel_idx];
            }
        }

        self.publish_unison();
    }

    #[inline]
    fn waveform_body_mut(wave_buff: &mut WaveformBuffer, size: WaveformSize) -> &mut [Sample] {
        let len = size.len();

        &mut wave_buff[WAVEFORM_PAD_LEFT..WAVEFORM_PAD_LEFT + len]
    }

    fn wrap_waveform(wave_buff: &mut WaveformBuffer, size: WaveformSize) {
        let last = WAVEFORM_PAD_LEFT + size.len() - 1;

        wave_buff[0] = wave_buff[last];
        wave_buff[last + 1] = wave_buff[WAVEFORM_PAD_LEFT];
        wave_buff[last + 2] = wave_buff[WAVEFORM_PAD_LEFT + 1];
    }

    fn build_wave(
        ifft: &IfftPlanners,
        frequency: f32,
        sample_rate: f32,
        spectral_buff: &[ComplexSample],
        dft_buff: &mut DftBuffer,
        scratch_buff: &mut DftBuffer,
        out_wave: &mut Waveform,
    ) {
        let frequency = frequency.abs();
        let max_frequency = 0.5 * sample_rate;

        let cutoff_index =
            ((max_frequency / frequency).floor() as usize + 1).min(spectral_buff.len());

        let (size, inverse_fft) = ifft.select(cutoff_index);
        let complex_len = inverse_fft.complex_len();
        debug_assert!(cutoff_index <= complex_len);
        debug_assert_eq!(inverse_fft.len(), size.len());

        let dft = &mut dft_buff[..complex_len];
        dft[..cutoff_index].copy_from_slice(&spectral_buff[..cutoff_index]);
        dft[cutoff_index..].fill(ComplexSample::ZERO);

        inverse_fft
            .process_with_scratch(
                dft,
                Self::waveform_body_mut(&mut out_wave.samples, size),
                scratch_buff,
            )
            .expect("ifft should succeed");

        Self::wrap_waveform(&mut out_wave.samples, size);
        out_wave.size = size;
    }

    fn build_this_frame_wave(
        &mut self,
        target: &VoiceTarget,
        rf: &mut RouterFactory<AudioRouterType, L::EngineEnd>,
    ) {
        if self.params.mono_spectrum && target.channel_idx == RIGHT_CHANNEL {
            return;
        }

        let mut router = rf.for_triggered_voice(target);
        let pitch = router.direct(self.inputs.pitch)[0];
        let freq_shift = router.scalar(
            &self.inputs.freq_shift,
            self.channel_params[target.channel_idx]
                .frequency_shift
                .get(),
            true,
        );
        let spectrum = router.spectral(self.inputs.spectrum);

        Self::build_wave(
            &self.ifft,
            pitch_to_freq(pitch) + freq_shift,
            router.sample_rate(),
            spectrum,
            &mut self.buffers.tmp_spectral,
            &mut self.buffers.scratch,
            &mut self
                .voice_buffers
                .at_mut(target.channel_idx, target.voice_idx)
                .wave,
        );
    }

    fn process_unison(
        &mut self,
        channel_idx: usize,
        voice_idx: usize,
        router: &mut Router<'_, '_, '_, L::EngineEnd>,
    ) {
        match self.params.unison_style {
            UnisonStyle::Manual => {
                self.fill_unison_voices(channel_idx, voice_idx, router, &ManualUnison);
            }
            UnisonStyle::Flat => {
                let style = FlatUnison::new(self.params.unison_stereo, channel_idx);
                self.fill_unison_voices(channel_idx, voice_idx, router, &style);
            }
            UnisonStyle::Convex => {
                let style = ConvexUnison::new(self.params.unison_stereo, channel_idx);
                self.fill_unison_voices(channel_idx, voice_idx, router, &style);
            }
        }

        self.store_unison_lanes(channel_idx, voice_idx);
    }

    fn fill_unison_voices(
        &mut self,
        channel_idx: usize,
        voice_idx: usize,
        router: &mut Router<'_, '_, '_, L::EngineEnd>,
        style: &impl UnisonType,
    ) {
        let unison = self.params.unison;
        let inputs = &self.inputs;
        let channel = &self.channel_params[channel_idx];
        let voice = self.voices.at_mut(channel_idx, voice_idx);

        if unison < 2 {
            voice.unison[0] = UnisonVoice::default();
            return;
        }

        if router.triggered() {
            for (state, update) in izip!(
                &mut voice.unison,
                style.process(true, unison, channel, inputs, router)
            ) {
                state.rate.from = update.rate;
                state.phase_shift.from = update.phase_shift;
                state.gain.from = update.gain;
            }
        } else {
            for state in voice.unison.iter_mut().take(unison) {
                state.rate.advance();
                state.phase_shift.advance();
                state.gain.advance();
            }
        }

        for (state, update) in izip!(
            &mut voice.unison,
            style.process(false, unison, channel, inputs, router)
        ) {
            state.rate.to = update.rate;
            state.phase_shift.to = update.phase_shift;
            state.gain.to = update.gain;
        }
    }

    fn store_unison_lanes(&mut self, channel_idx: usize, voice_idx: usize) {
        let unison = self.params.unison.clamp(1, MAX_UNISON_VOICES);
        let full_chunks = unison / UNISON_LANES;
        let rem_lanes = unison % UNISON_LANES;
        let voice = self.voices.at(channel_idx, voice_idx);

        for chunk_idx in 0..full_chunks {
            let start = chunk_idx * UNISON_LANES;
            let params = UnisonLaneParams::from_voices(&voice.unison[start..start + UNISON_LANES]);
            self.lane_params[chunk_idx] = params;
        }

        if rem_lanes > 0 {
            let start = full_chunks * UNISON_LANES;
            let params = UnisonLaneParams::from_voices(&voice.unison[start..start + rem_lanes]);
            self.lane_params[full_chunks] = params;
        }
    }

    fn process_phase_reset(
        &mut self,
        channel_idx: usize,
        voice_idx: usize,
        router: &mut Router<'_, '_, '_, L::EngineEnd>,
    ) {
        let Some(phase_reset) = self
            .voices
            .at_mut(channel_idx, voice_idx)
            .phase_reset
            .take()
        else {
            return;
        };

        let channel = &self.channel_params[channel_idx];
        let unison = self.params.unison;
        let voice = self.voices.at_mut(channel_idx, voice_idx);

        // When steal_phase set to false - control that toggle by input value
        let steal_phase = router.scalar(
            &self.inputs.phase_steal,
            Sample::from(self.params.steal_phase),
            true,
        ) >= 0.5;

        if phase_reset.stolen && steal_phase {
            // Phases already copied from another voice
            return;
        }

        let amount = self.params.phase_random;
        let stereo = self.params.phase_random_stereo;

        if amount > 1e-6 || stereo > 1e-6 {
            // Left runs first and stores the shared random phases. Each channel then
            // adds its own initial phase and a stereo offset.
            if channel_idx == LEFT_CHANNEL {
                for (phase, random) in izip!(
                    self.shared_random_phases.iter_mut(),
                    (&mut self.random).random_iter::<Sample>()
                )
                .take(unison)
                {
                    *phase = Phase::from_normalized((random - 0.5) * amount);
                }
            }

            for (phase, base, unison_voice, random) in izip!(
                voice.phases.iter_mut(),
                self.shared_random_phases.iter().copied(),
                channel.unison.iter(),
                (&mut self.random).random_iter::<Sample>()
            )
            .take(unison)
            {
                *phase = base
                    .add_normalized(unison_voice.initial_phase)
                    .add_normalized((random - 0.5) * stereo);
            }
        } else if unison > 1 {
            for (phase, unison_voice) in voice.phases.iter_mut().zip(&channel.unison).take(unison) {
                *phase = Phase::from_normalized(unison_voice.initial_phase);
            }
        } else {
            voice.phases[0] = Phase::ZERO;
        }
    }

    fn collect_phase_steals(
        &self,
        target: &VoiceTarget,
        rf: &RouterFactory<AudioRouterType, L::EngineEnd>,
    ) -> SmallVec<[PhaseSteal; MAX_VOICES]> {
        let mut steals = SmallVec::new();

        if target.triggered.is_some() || !rf.params().has_triggered_voices {
            return steals;
        }

        let channel_idx = target.channel_idx;
        let source_idx = target.voice_idx;

        for playing in rf.params().active_voices {
            let Some(offset) = playing.triggered() else {
                continue;
            };
            let requester_idx = playing.voice_idx();

            if self
                .voices
                .at(channel_idx, requester_idx)
                .phase_reset
                .as_ref()
                .is_some_and(|reset| reset.steal_from == Some(source_idx))
            {
                steals.push(PhaseSteal {
                    offset: offset as u16,
                    voice_idx: requester_idx as u16,
                });
            }
        }

        steals.sort_unstable_by_key(|steal| steal.offset);
        steals
    }

    fn pitch_to_phase_inc(
        pitch: &[Sample],
        freq_shift: &[Sample],
        out: &mut [Sample],
        freq_phase_mult: Sample,
    ) {
        let mult = f32x4::splat(freq_phase_mult);

        zip_map_x4(pitch, freq_shift, out, |pitch, shift| {
            shift.mul_add(mult, fast_pitch_to_freq_x4(pitch) * mult)
        });
    }

    #[inline(never)]
    fn render_voice_samples(
        &mut self,
        ctx: &VoiceRenderCtx,
        output: &mut [Sample],
        start: usize,
        end: usize,
    ) {
        if start >= end {
            return;
        }

        let from_size = self
            .voice_buffers
            .at(ctx.wave_channel, ctx.voice_idx)
            .wave
            .size;
        let to_size = self.buffers.tmp_wave.size;

        match (from_size, to_size) {
            (WaveformSize::Half, WaveformSize::Half) => {
                self.render_voice_samples_same::<HALF_WAVEFORM_BITS>(ctx, output, start, end)
            }
            (WaveformSize::Full, WaveformSize::Full) => {
                self.render_voice_samples_same::<WAVEFORM_BITS>(ctx, output, start, end)
            }
            (WaveformSize::Full, WaveformSize::Half) => self
                .render_voice_samples_sized::<WAVEFORM_BITS, HALF_WAVEFORM_BITS>(
                    ctx, output, start, end,
                ),
            (WaveformSize::Half, WaveformSize::Full) => self
                .render_voice_samples_sized::<HALF_WAVEFORM_BITS, WAVEFORM_BITS>(
                    ctx, output, start, end,
                ),
        }
    }

    #[inline(never)]
    fn render_voice_samples_same<const BITS: usize>(
        &mut self,
        ctx: &VoiceRenderCtx,
        output: &mut [Sample],
        start: usize,
        end: usize,
    ) {
        self.render_voice_loop(ctx, output, start, end, lanes::render_same::<BITS>);
    }

    #[inline(never)]
    fn render_voice_samples_sized<const FROM_BITS: usize, const TO_BITS: usize>(
        &mut self,
        ctx: &VoiceRenderCtx,
        output: &mut [Sample],
        start: usize,
        end: usize,
    ) {
        self.render_voice_loop(
            ctx,
            output,
            start,
            end,
            lanes::render_sized::<FROM_BITS, TO_BITS>,
        );
    }

    #[inline(always)]
    fn render_voice_loop(
        &mut self,
        ctx: &VoiceRenderCtx,
        output: &mut [Sample],
        start: usize,
        end: usize,
        mut render_lanes: impl FnMut(
            &mut [Phase; UNISON_LANES],
            &UnisonLaneParams,
            &mut SampleCtx,
            usize,
        ),
    ) {
        let unison = self.params.unison.clamp(1, MAX_UNISON_VOICES);
        let voice = self.voices.at_mut(ctx.channel_idx, ctx.voice_idx);
        let wave_from = self
            .voice_buffers
            .at(ctx.wave_channel, ctx.voice_idx)
            .wave
            .samples
            .as_ref();
        let wave_to = self.buffers.tmp_wave.samples.as_ref();
        let buff_t_inc = (ctx.samples as Sample).recip();
        let mut buff_t = start as Sample * buff_t_inc;

        let (phase_chunks, []) = voice.phases.as_chunks_mut::<UNISON_LANES>() else {
            unreachable!("MAX_UNISON_VOICES is a multiple of UNISON_LANES");
        };

        let full_chunks = unison / UNISON_LANES;
        let rem_lanes = unison % UNISON_LANES;
        let (full_phases, rem_phases) = phase_chunks.split_at_mut(full_chunks);
        let (full_params, rem_params) = self.lane_params.split_at(full_chunks);

        for (out, &phase_inc, &phase_shift) in izip!(
            output[start..end].iter_mut(),
            &self.buffers.phase_inc[start..end],
            &self.buffers.phase_shift[start..end],
        ) {
            let mut s = SampleCtx {
                buff_t: f32x4::splat(buff_t),
                phase_shift: PhaseX4::splat(Phase::from_normalized(phase_shift)),
                phase_inc: f32x4::splat(phase_inc),
                wave_from,
                wave_to,
                acc_from: [f32x4::ZERO; UNISON_LANES],
                acc_to: [f32x4::ZERO; UNISON_LANES],
            };

            for (phases, params) in full_phases.iter_mut().zip(full_params) {
                render_lanes(phases, params, &mut s, UNISON_LANES);
            }

            if rem_lanes > 0 {
                render_lanes(&mut rem_phases[0], &rem_params[0], &mut s, rem_lanes);
            }

            let acc_from = (s.acc_from[0] + s.acc_from[1]) + (s.acc_from[2] + s.acc_from[3]);
            let acc_to = (s.acc_to[0] + s.acc_to[1]) + (s.acc_to[2] + s.acc_to[3]);
            let acc = (acc_to - acc_from).mul_add(s.buff_t, acc_from);

            *out = acc.reduce_add();
            buff_t += buff_t_inc;
        }
    }

    fn process_voice(
        &mut self,
        target: &VoiceTarget,
        outputs: &mut VoicesLayout<SamplesOutput>,
        rf: &mut RouterFactory<AudioRouterType, L::EngineEnd>,
    ) {
        let channel_idx = target.channel_idx;
        let voice_idx = target.voice_idx;
        let steals = self.collect_phase_steals(target, rf);

        let (mut router, mut voice_output) = rf.for_voice(target, outputs);

        self.process_phase_reset(channel_idx, voice_idx, &mut router);
        self.process_unison(channel_idx, voice_idx, &mut router);

        let samples = router.samples();
        let channel = &self.channel_params[channel_idx];
        let inputs = &self.inputs;

        router.param(
            &inputs.phase_shift,
            &channel.phase_shift,
            &mut self.buffers.phase_shift,
        );
        router.param(
            &inputs.freq_shift,
            &channel.frequency_shift,
            &mut self.buffers.frequency_shift,
        );

        let freq_phase_mult = Phase::freq_phase_mult(router.sample_rate());

        Self::pitch_to_phase_inc(
            router.direct(inputs.pitch),
            &self.buffers.frequency_shift[..samples],
            &mut self.buffers.phase_inc[..samples],
            freq_phase_mult,
        );

        let mono_spectrum = self.params.mono_spectrum;
        let wave_channel = if mono_spectrum {
            LEFT_CHANNEL
        } else {
            channel_idx
        };

        if channel_idx == wave_channel {
            let last = samples.saturating_sub(1);

            Self::build_wave(
                &self.ifft,
                Phase::phase_inc_to_freq(self.buffers.phase_inc[last], freq_phase_mult),
                router.sample_rate(),
                router.spectral(inputs.spectrum),
                &mut self.buffers.tmp_spectral,
                &mut self.buffers.scratch,
                &mut self.buffers.tmp_wave,
            );
        }

        if router.need_update_ui_mono() {
            self.audio_end
                .update_spectrum(router.spectral(inputs.spectrum));
        }

        let ctx = VoiceRenderCtx {
            channel_idx,
            voice_idx,
            wave_channel,
            samples,
        };
        let output = voice_output.output();
        let mut start = 0;

        for steal in &steals {
            let offset = (steal.offset as usize).min(ctx.samples);

            self.render_voice_samples(&ctx, output, start, offset);

            let phases = self.voices.at(channel_idx, voice_idx).phases;
            let requester = self.voices.at_mut(channel_idx, steal.voice_idx as usize);

            requester.phases = phases;

            if let Some(reset) = requester.phase_reset.as_mut() {
                reset.stolen = true;
            }
            start = offset;
        }

        self.render_voice_samples(&ctx, output, start, ctx.samples);

        if !mono_spectrum || channel_idx == RIGHT_CHANNEL {
            mem::swap(
                &mut self.voice_buffers.at_mut(wave_channel, voice_idx).wave,
                &mut self.buffers.tmp_wave,
            );
        }
    }

    fn handle_trigger(voice: &mut Voice, replaced_voice_idx: Option<usize>) {
        voice.phase_reset = Some(PhaseReset {
            steal_from: replaced_voice_idx,
            stolen: false,
        });
    }

    pub(crate) fn process(&mut self, ctx: &mut ProcessContext<L::EngineEnd>) {
        ctx.audio(self.id, self.output_slot)
            .for_triggered_voices(|rf, target| {
                self.build_this_frame_wave(target, rf);
            })
            .for_voices(|rf, target, outputs| {
                self.process_voice(target, outputs, rf);
            })
            .for_channels(|rf, channel_idx| {
                self.channel_params[channel_idx]
                    .advance_smoothers(&rf.params().smooth_params, rf.params().samples);
            });
    }
}

impl<L: OscillatorLinks> SynthModule for Oscillator<L> {
    fn id(&self) -> ModuleId {
        self.id
    }

    fn inputs(&self) -> &'static [InputMeta] {
        static INPUTS: &[InputMeta] = &[
            InputMeta::spectral(Input::Spectrum),
            InputMeta::direct_control(Input::Pitch),
            InputMeta::audio(Input::PhaseShift),
            InputMeta::audio(Input::FrequencyShift),
            InputMeta::control(Input::Detune),
            InputMeta::control(Input::DetuneFocus),
            InputMeta::control(Input::PhaseSteal),
            InputMeta::control(Input::PhasesBlend),
            InputMeta::control(Input::GainsBlend),
        ];

        INPUTS
    }

    fn output_type(&self) -> DataType {
        DataType::Audio
    }

    fn output_slot(&self) -> usize {
        self.output_slot
    }

    fn set_output_slot(&mut self, slot: usize) {
        self.output_slot = slot;
    }

    fn set_input_slots(&mut self, inputs: &[InputSlots]) {
        self.inputs = Inputs::from_slots(inputs);
    }

    fn update_input_amount(&mut self, input_type: Input, src_slot: usize, amount: StereoSample) {
        self.inputs.update_amount(input_type, src_slot, amount);
    }

    fn process_events(&mut self, events: &[VoiceEvent]) {
        for event in events {
            match event {
                VoiceEvent::Reset {
                    voice_idx,
                    replaced_voice_idx,
                    ..
                } => {
                    for voice in self.voices.channels_at_mut(*voice_idx) {
                        Self::handle_trigger(voice, *replaced_voice_idx);
                    }
                }
                VoiceEvent::Update { .. } => {}
                _ => (),
            }
        }
    }

    fn process_ui_events(&mut self) {
        while let Some(event) = self.audio_end.pop_event() {
            match event {
                UiEvent::InputParam { input, value } => match input {
                    Input::PhaseShift => self.set_phase_shift(value),
                    Input::FrequencyShift => self.set_frequency_shift(value),
                    Input::Detune => self.set_detune(value),
                    Input::DetuneFocus => self.set_detune_focus(value),
                    Input::PhasesBlend => self.set_phases_blend(value),
                    Input::GainsBlend => self.set_gains_blend(value),
                    _ => (),
                },
                UiEvent::Unison(unison) => self.set_unison(unison),
                UiEvent::UnisonStyle(style) => self.set_unison_style(style),
                UiEvent::UnisonStereo(stereo) => self.set_unison_stereo(stereo),
                UiEvent::UnisonInitialPhase { idx, value } => self.set_initial_phase(idx, value),
                UiEvent::UnisonPhaseShift { idx, value } => self.set_unison_phase(idx, value),
                UiEvent::UnisonPhaseShiftTo { idx, value } => self.set_unison_phase_to(idx, value),
                UiEvent::UnisonGain { idx, value } => self.set_unison_gain(idx, value),
                UiEvent::UnisonGainTo { idx, value } => self.set_unison_gain_to(idx, value),
                UiEvent::StealPhase(steal_phase) => self.set_steal_phase(steal_phase),
                UiEvent::PhaseRandom(phase_random) => self.set_phase_random(phase_random),
                UiEvent::PhaseRandomStereo(phase_random_stereo) => {
                    self.set_phase_random_stereo(phase_random_stereo);
                }
                UiEvent::MonoSpectrum(mono_spectrum) => self.set_mono_spectrum(mono_spectrum),
                UiEvent::ApplyUnisonLevelShape { center, level, to } => {
                    self.apply_unison_level_shape(center, level, to);
                }
                UiEvent::RandomizePhases {
                    amount,
                    stereo_spread,
                    dst,
                } => {
                    self.randomize_phases(amount, stereo_spread, dst);
                }
            }
        }
    }
}
