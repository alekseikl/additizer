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
        coeffs::catmull_rom_from_powers,
        phase::{Phase, PhaseX4},
        routing::{
            AudioRouterType, DataType, Input, InputMeta, InputSlots, LEFT_CHANNEL, MAX_VOICES,
            ModuleId, NUM_CHANNELS, ProcessContext, RIGHT_CHANNEL, RouterFactory, SamplesOutput,
            SpectralInputSlot, VoiceEvent, VoiceRouter, VoiceTarget,
        },
        smooth::SmoothedSample,
        synth_module::SynthModule,
        types::{ComplexSample, Sample},
    },
    synth_engine::{db_to_gain, fast_pitch_to_freq_x4, from_st, pitch_to_freq, power_scale},
};

mod config;
mod link;
pub mod stub;

#[cfg(test)]
mod tests;

pub use config::OscillatorConfig;
pub use link::{OscillatorAudioEnd, OscillatorLinks, OscillatorUiEnd, UiEvent, Unison};

const WAVEFORM_BITS: usize = SPECTRUM_BITS + 1;
const WAVEFORM_SIZE: usize = 1 << WAVEFORM_BITS;
const WAVEFORM_PAD_LEFT: usize = 1;
const WAVEFORM_PAD_RIGHT: usize = 2;
const WAVEFORM_BUFFER_SIZE: usize = WAVEFORM_SIZE + WAVEFORM_PAD_LEFT + WAVEFORM_PAD_RIGHT;
const DFT_BUFFER_SIZE: usize = (1 << (WAVEFORM_BITS - 1)) + 1;

pub const MAX_UNISON_VOICES: usize = 16;

type WaveformBuffer = [Sample; WAVEFORM_BUFFER_SIZE];
type DftBuffer = [ComplexSample; DFT_BUFFER_SIZE];

struct Params {
    unison: usize,
    steal_phase: bool,
    phase_random: Sample, // [0.0, 1.0]
    mono_spectrum: bool,
}

impl Params {
    fn from_config(c: &config::OscillatorConfig) -> Self {
        Self {
            unison: c.unison_voices,
            steal_phase: c.steal_phase,
            phase_random: c.phase_random,
            mono_spectrum: c.mono_spectrum,
        }
    }
}

struct UnisonParams {
    initial_phase: Sample,
    phase_shift: Sample,
    phase_shift_to: Sample,
    gain: Sample,
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
    detune_power: Sample,
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
            detune_power: c.detune_power[channel_idx],
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
    freq_phase_mult: Sample,
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

struct UnisonStateUpdate {
    rate: Sample,
    phase_shift: Sample,
    gain: Sample,
}

/// Number of unison voices handled per SIMD step in the render loop.
const UNISON_LANES: usize = 4;
const UNISON_CHUNKS: usize = MAX_UNISON_VOICES / UNISON_LANES;

/// Parameters of `UNISON_LANES` unison voices in `from + delta * t` form,
/// structure-of-arrays so the per-sample interpolation is one FMA per
/// parameter for all lanes at once.
#[derive(Clone, Copy, Default)]
struct UnisonLaneParams {
    rate_from: f32x4,
    rate_delta: f32x4,
    phase_shift_from: f32x4,
    phase_shift_delta: f32x4,
    gain_from: f32x4,
    gain_delta: f32x4,
}

impl UnisonLaneParams {
    #[inline(always)]
    fn new(voices: &[UnisonVoice; UNISON_LANES]) -> Self {
        let lanes = |f: fn(&UnisonVoice) -> Sample| f32x4::new(array::from_fn(|i| f(&voices[i])));

        // Endpoints are wrapped once per block. A lerp of values in
        // `[-0.5, 0.5]` stays there, so the sample loop can skip the modulo.
        let phase_shift_from = PhaseX4::wrap_normalized(lanes(|uv| uv.phase_shift.from));
        let phase_shift_to = PhaseX4::wrap_normalized(lanes(|uv| uv.phase_shift.to));

        Self {
            rate_from: lanes(|uv| uv.rate.from),
            rate_delta: lanes(|uv| uv.rate.to - uv.rate.from),
            phase_shift_from,
            phase_shift_delta: phase_shift_to - phase_shift_from,
            gain_from: lanes(|uv| uv.gain.from),
            gain_delta: lanes(|uv| uv.gain.to - uv.gain.from),
        }
    }
}

/// Per-lane wavetable read positions and gain-scaled powers of the
/// fractional position, for `UNISON_LANES` unison voices at one sample.
struct UnisonTaps {
    idx: [u32; UNISON_LANES],
    g: f32x4,
    gt: f32x4,
    gt2: f32x4,
    gt3: f32x4,
}

/// Per-sample values shared by all unison voices.
#[derive(Clone, Copy)]
struct SampleCtx {
    buff_t: f32x4,
    phase_shift: PhaseX4,
    pitch_phase_inc: f32x4,
    freq_phase_inc: f32x4,
}

struct Voice {
    phase_reset: Option<PhaseReset>,
    unison_gain: Interpolated,
    unison: [UnisonVoice; MAX_UNISON_VOICES],
    phases: [Phase; MAX_UNISON_VOICES],
}

impl Default for Voice {
    fn default() -> Self {
        Self {
            phase_reset: None,
            phases: Default::default(),
            unison_gain: Interpolated { from: 1.0, to: 1.0 },
            unison: Default::default(),
        }
    }
}

struct VoiceBuffers {
    wave: Box<WaveformBuffer>,
}

impl Default for VoiceBuffers {
    fn default() -> Self {
        Self {
            wave: Box::new([0.0; WAVEFORM_BUFFER_SIZE]),
        }
    }
}

struct Buffers {
    tmp_wave: Box<WaveformBuffer>,
    tmp_spectral: DftBuffer,
    scratch: DftBuffer,
    pitch: Buffer,
    /// Per-sample phase increment from `pitch` alone (frequency shift excluded).
    pitch_phase_inc: Buffer,
    phase_shift: Buffer,
    frequency_shift: Buffer,
}

impl Default for Buffers {
    fn default() -> Self {
        Self {
            tmp_wave: Box::new([0.0; WAVEFORM_BUFFER_SIZE]),
            tmp_spectral: [ComplexSample::ZERO; DFT_BUFFER_SIZE],
            scratch: [ComplexSample::ZERO; DFT_BUFFER_SIZE],
            pitch: zero_buffer(),
            pitch_phase_inc: zero_buffer(),
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
    phase_shift: InputSlots,
    freq_shift: InputSlots,
    detune: InputSlots,
    detune_power: InputSlots,
    phase_steal: InputSlots,
    phases_blend: InputSlots,
    gains_blend: InputSlots,
}

impl Default for Inputs {
    fn default() -> Self {
        Self {
            spectrum: None,
            pitch: None,
            phase_shift: InputSlots::new(Input::PhaseShift),
            freq_shift: InputSlots::new(Input::FrequencyShift),
            detune: InputSlots::new(Input::Detune),
            detune_power: InputSlots::new(Input::DetunePower),
            phase_steal: InputSlots::new(Input::PhaseSteal),
            phases_blend: InputSlots::new(Input::PhasesBlend),
            gains_blend: InputSlots::new(Input::GainsBlend),
        }
    }
}

impl Inputs {
    fn from_slots(inputs: &[InputSlots], spectral_inputs: &[SpectralInputSlot]) -> Self {
        let mut result = Self::default();

        for input in inputs {
            match input.input_type {
                Input::Pitch => result.pitch = input.slots.first().map(|s| s.src_slot),
                Input::PhaseShift => result.phase_shift = input.clone(),
                Input::FrequencyShift => result.freq_shift = input.clone(),
                Input::Detune => result.detune = input.clone(),
                Input::DetunePower => result.detune_power = input.clone(),
                Input::PhaseSteal => result.phase_steal = input.clone(),
                Input::PhasesBlend => result.phases_blend = input.clone(),
                Input::GainsBlend => result.gains_blend = input.clone(),
                _ => (),
            }
        }

        for input in spectral_inputs {
            if matches!(input.input_type, Input::Spectrum) {
                result.spectrum = Some(input.slot);
            }
        }

        result
    }

    fn update_amount(&mut self, input_type: Input, src_slot: usize, amount: StereoSample) {
        match input_type {
            Input::PhaseShift => self.phase_shift.update_amount(src_slot, amount),
            Input::FrequencyShift => self.freq_shift.update_amount(src_slot, amount),
            Input::Detune => self.detune.update_amount(src_slot, amount),
            Input::DetunePower => self.detune_power.update_amount(src_slot, amount),
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
    inverse_fft: Arc<dyn ComplexToReal<Sample>>,
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
    center_phase_sync: Phase,
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
            inverse_fft: RealFftPlanner::<Sample>::new().plan_fft_inverse(WAVEFORM_SIZE),
            random: Pcg32::new(420, 1337),
            audio_end,
            ui_end,
            inputs: Inputs::default(),
            output_slot: usize::MAX,
            voices: new_voices_layout(),
            voice_buffers: new_voices_layout(),
            center_phase_sync: Phase::ZERO,
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
            steal_phase: self.params.steal_phase,
            phase_random: self.params.phase_random,
            mono_spectrum: self.params.mono_spectrum,
            detune: get_stereo_param!(self, detune),
            detune_power: get_stereo_param!(self, detune_power),
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
    set_mono_param!(set_steal_phase, steal_phase, bool);
    set_mono_param!(
        set_phase_random,
        phase_random,
        Sample,
        phase_random.clamp(0.0, 1.0)
    );
    set_mono_param!(set_mono_spectrum, mono_spectrum, bool);

    set_stereo_param!(set_detune, detune, detune.clamp(0.0, from_st(1.0)));
    set_stereo_param!(
        set_detune_power,
        detune_power,
        detune_power.clamp(-1.0, 1.0)
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
    fn get_wave_slice_mut(wave_buff: &mut WaveformBuffer) -> &mut [Sample] {
        &mut wave_buff[WAVEFORM_PAD_LEFT..(WAVEFORM_BUFFER_SIZE - WAVEFORM_PAD_RIGHT)]
    }

    #[inline(always)]
    fn load_segment(wave_buffer: &WaveformBuffer, idx: usize) -> f32x4 {
        let s = &wave_buffer[idx..idx + 4];

        f32x4::new([s[0], s[1], s[2], s[3]])
    }

    /// SIMD part of one sample for `UNISON_LANES` unison voices: interpolates
    /// the lane parameters, resolves the wavetable read positions and advances
    /// the phases.
    #[inline(always)]
    fn advance_unison_lanes(
        phases: &mut [Phase; UNISON_LANES],
        p: &UnisonLaneParams,
        s: &SampleCtx,
    ) -> UnisonTaps {
        let phase = PhaseX4::load(phases);

        let unison_shift = p.phase_shift_delta.mul_add(s.buff_t, p.phase_shift_from);
        let read_phase = phase + s.phase_shift + PhaseX4::from_wrapped(unison_shift);
        let idx = read_phase.wave_index::<WAVEFORM_BITS>();
        let t = read_phase.wave_index_fraction::<WAVEFORM_BITS>();

        let g = p.gain_delta.mul_add(s.buff_t, p.gain_from);
        let gt = g * t;
        let gt2 = gt * t;
        let gt3 = gt2 * t;

        let rate = p.rate_delta.mul_add(s.buff_t, p.rate_from);
        (phase + rate.mul_add(s.pitch_phase_inc, s.freq_phase_inc)).store(phases);

        UnisonTaps {
            idx,
            g,
            gt,
            gt2,
            gt3,
        }
    }

    /// Scalar-lane part of one sample for one unison voice: reads the 4-tap
    /// segment from both waves and accumulates it with the gain-scaled
    /// Catmull-Rom weights.
    ///
    /// The `from` and `to` waves are accumulated separately and crossfaded by
    /// the caller once per sample; this is algebraically identical to
    /// crossfading each segment but saves a vector sub + FMA per unison voice.
    #[inline(always)]
    fn accumulate_unison_lane(
        taps: &UnisonTaps,
        lane: usize,
        wave_from: &WaveformBuffer,
        wave_to: &WaveformBuffer,
        acc_from: &mut f32x4,
        acc_to: &mut f32x4,
    ) {
        let idx = taps.idx[lane] as usize;
        let weights = catmull_rom_from_powers(
            taps.g.as_array()[lane],
            taps.gt.as_array()[lane],
            taps.gt2.as_array()[lane],
            taps.gt3.as_array()[lane],
        );

        *acc_from = Self::load_segment(wave_from, idx).mul_add(weights, *acc_from);
        *acc_to = Self::load_segment(wave_to, idx).mul_add(weights, *acc_to);
    }

    fn build_wave(
        inverse_fft: &dyn ComplexToReal<Sample>,
        frequency: f32,
        sample_rate: f32,
        spectral_buff: &[ComplexSample],
        dft_buff: &mut DftBuffer,
        scratch_buff: &mut DftBuffer,
        out_wave_buff: &mut WaveformBuffer,
    ) {
        let frequency = frequency.abs();
        let max_frequency = 0.5 * sample_rate;

        let cutoff_index =
            ((max_frequency / frequency).floor() as usize + 1).min(spectral_buff.len());

        dft_buff[..cutoff_index].copy_from_slice(&spectral_buff[..cutoff_index]);
        dft_buff[cutoff_index..].fill(ComplexSample::ZERO);

        inverse_fft
            .process_with_scratch(
                dft_buff,
                Self::get_wave_slice_mut(out_wave_buff),
                scratch_buff,
            )
            .expect("ifft should succeed");

        out_wave_buff[0] = out_wave_buff[WAVEFORM_BUFFER_SIZE - WAVEFORM_PAD_RIGHT - 1];
        out_wave_buff[WAVEFORM_BUFFER_SIZE - WAVEFORM_PAD_RIGHT] = out_wave_buff[WAVEFORM_PAD_LEFT];
        out_wave_buff[WAVEFORM_BUFFER_SIZE - WAVEFORM_PAD_RIGHT + 1] =
            out_wave_buff[WAVEFORM_PAD_LEFT + 1];
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
        let pitch = if self.inputs.pitch.is_some() {
            router.direct(self.inputs.pitch)[0]
        } else {
            target.note_pitch()
        };
        let freq_shift = router.scalar(
            &self.inputs.freq_shift,
            self.channel_params[target.channel_idx]
                .frequency_shift
                .get(),
            true,
        );
        let spectrum = router.spectral(self.inputs.spectrum);

        Self::build_wave(
            self.inverse_fft.as_ref(),
            pitch_to_freq(pitch) + freq_shift,
            router.sample_rate(),
            spectrum,
            &mut self.buffers.tmp_spectral,
            &mut self.buffers.scratch,
            &mut self.voice_buffers[target.channel_idx][target.voice_idx].wave,
        );
    }

    fn calc_unison_update(
        unison: usize,
        this_frame: bool,
        channel: &ChannelParams,
        inputs: &Inputs,
        router: &mut Router<'_, '_, '_, L::EngineEnd>,
    ) -> impl Iterator<Item = UnisonStateUpdate> {
        const MAX_DETUNE: Sample = 1.0;
        const MAX_DETUNE_POWER: Sample = 5.0;

        let detune = router
            .scalar(&inputs.detune, channel.detune, this_frame)
            .clamp(0.0, MAX_DETUNE);

        let detune_power = router
            .scalar(&inputs.detune_power, channel.detune_power, this_frame)
            .clamp(-1.0, 1.0)
            * MAX_DETUNE_POWER;

        let phases_blend = router
            .scalar(&inputs.phases_blend, channel.phases_blend, this_frame)
            .clamp(0.0, 1.0);

        let gains_blend = router
            .scalar(&inputs.gains_blend, channel.gains_blend, this_frame)
            .clamp(0.0, 1.0);

        let center = 0.5 * (unison - 1) as Sample;
        let center_recip = center.recip();

        channel
            .unison
            .iter()
            .take(unison)
            .enumerate()
            .map(move |(idx, param)| {
                let spread = (idx as Sample - center) * center_recip;

                UnisonStateUpdate {
                    rate: (power_scale(spread.abs(), detune_power).copysign(spread) * detune)
                        .exp2(),
                    phase_shift: (param.phase_shift_to - param.phase_shift)
                        .mul_add(phases_blend, param.phase_shift),
                    gain: (param.gain_to - param.gain).mul_add(gains_blend, param.gain),
                }
            })
    }

    fn process_unison(
        &mut self,
        channel_idx: usize,
        voice_idx: usize,
        router: &mut Router<'_, '_, '_, L::EngineEnd>,
    ) {
        let channel = &self.channel_params[channel_idx];
        let voice = &mut self.voices[channel_idx][voice_idx];

        if self.params.unison < 2 {
            voice.unison[0] = UnisonVoice::default();
            voice.unison_gain = Interpolated { from: 1.0, to: 1.0 };
            return;
        }

        fn calc_unison_gain(gains: impl Iterator<Item = Sample>) -> Sample {
            gains
                .map(|gain| gain * gain)
                .sum::<Sample>()
                .sqrt()
                .max(1.0) //Don't amplify
                .recip()
        }

        if router.triggered() {
            for (state, update) in izip!(
                &mut voice.unison,
                Self::calc_unison_update(self.params.unison, true, channel, &self.inputs, router)
            ) {
                state.rate.from = update.rate;
                state.phase_shift.from = update.phase_shift;
                state.gain.from = update.gain;
            }

            voice.unison_gain.from = calc_unison_gain(
                voice
                    .unison
                    .iter()
                    .take(self.params.unison)
                    .map(|state| state.gain.from),
            );
        } else {
            for state in voice.unison.iter_mut().take(self.params.unison) {
                state.rate.advance();
                state.phase_shift.advance();
                state.gain.advance();
            }

            voice.unison_gain.advance();
        }

        for (state, update) in izip!(
            &mut voice.unison,
            Self::calc_unison_update(self.params.unison, false, channel, &self.inputs, router)
        ) {
            state.rate.to = update.rate;
            state.phase_shift.to = update.phase_shift;
            state.gain.to = update.gain;
        }

        voice.unison_gain.to = calc_unison_gain(
            voice
                .unison
                .iter()
                .take(self.params.unison)
                .map(|state| state.gain.to),
        );
    }

    fn process_phase_reset(
        &mut self,
        channel_idx: usize,
        voice_idx: usize,
        router: &mut Router<'_, '_, '_, L::EngineEnd>,
    ) {
        let Some(phase_reset) = self.voices[channel_idx][voice_idx].phase_reset.take() else {
            return;
        };

        let channel = &self.channel_params[channel_idx];
        let unison = self.params.unison;
        let voice = &mut self.voices[channel_idx][voice_idx];

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

        if self.params.phase_random > 1e-6 {
            for (phase, unison_voice, random) in izip!(
                voice.phases.iter_mut(),
                channel.unison.iter(),
                (&mut self.random).random_iter::<Sample>()
            )
            .take(unison)
            {
                *phase = Phase::from_normalized(unison_voice.initial_phase)
                    .add_normalized((random - 0.5) * self.params.phase_random);
            }

            if unison & 1 == 1 {
                let center = unison / 2;

                if channel_idx == LEFT_CHANNEL {
                    self.center_phase_sync = voice.phases[center];
                } else {
                    voice.phases[center] = self.center_phase_sync;
                }
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

            if self.voices[channel_idx][requester_idx]
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

    /// Converts a pitch buffer (octaves) to per-sample phase increments, four
    /// samples at a time. Keeps `exp2f` calls (and their register clobbering)
    /// out of the per-sample render loop.
    fn pitch_to_phase_inc(pitch: &[Sample], out: &mut [Sample], freq_phase_mult: Sample) {
        let mult = f32x4::splat(freq_phase_mult);
        let (pitch_chunks, pitch_rem) = pitch.as_chunks::<4>();
        let (out_chunks, out_rem) = out.as_chunks_mut::<4>();

        for (out, pitch) in out_chunks.iter_mut().zip(pitch_chunks) {
            *out = (fast_pitch_to_freq_x4(f32x4::new(*pitch)) * mult).to_array();
        }

        for (out, &pitch) in out_rem.iter_mut().zip(pitch_rem) {
            *out = pitch_to_freq(pitch) * freq_phase_mult;
        }
    }

    // Kept out of line on purpose: when inlined into the (very large) `process`
    // closure the hot loop loses registers and reloads the Catmull-Rom
    // constants from the stack on every unison iteration.
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

        let unison = self.params.unison.clamp(1, MAX_UNISON_VOICES);
        let voice = &mut self.voices[ctx.channel_idx][ctx.voice_idx];
        let wave_from: &WaveformBuffer = &self.voice_buffers[ctx.wave_channel][ctx.voice_idx].wave;
        let wave_to: &WaveformBuffer = &self.buffers.tmp_wave;
        let buff_t_inc = (ctx.samples as Sample).recip();
        let mut buff_t = start as Sample * buff_t_inc;

        let unison_gain_from = voice.unison_gain.from;
        let unison_gain_delta = voice.unison_gain.to - voice.unison_gain.from;

        let (unison_chunks, []) = voice.unison.as_chunks::<UNISON_LANES>() else {
            unreachable!("MAX_UNISON_VOICES is a multiple of UNISON_LANES");
        };
        let lane_params: [UnisonLaneParams; UNISON_CHUNKS] =
            array::from_fn(|i| UnisonLaneParams::new(&unison_chunks[i]));
        let (phase_chunks, []) = voice.phases.as_chunks_mut::<UNISON_LANES>() else {
            unreachable!("MAX_UNISON_VOICES is a multiple of UNISON_LANES");
        };

        // Full chunks use all lanes; the last chunk may be partial.
        let full_chunks = unison / UNISON_LANES;
        let rem_lanes = unison % UNISON_LANES;
        let (full_phases, rem_phases) = phase_chunks.split_at_mut(full_chunks);
        let (full_params, rem_params) = lane_params.split_at(full_chunks);

        for (out, &pitch_phase_inc, &phase_shift, &freq_shift) in izip!(
            output[start..end].iter_mut(),
            &self.buffers.pitch_phase_inc[start..end],
            &self.buffers.phase_shift[start..end],
            &self.buffers.frequency_shift[start..end],
        ) {
            let s = SampleCtx {
                buff_t: f32x4::splat(buff_t),
                phase_shift: PhaseX4::splat(Phase::from_normalized(phase_shift)),
                pitch_phase_inc: f32x4::splat(pitch_phase_inc),
                freq_phase_inc: f32x4::splat(freq_shift * ctx.freq_phase_mult),
            };

            let mut acc_from = [f32x4::ZERO; UNISON_LANES];
            let mut acc_to = [f32x4::ZERO; UNISON_LANES];

            for (phases, params) in full_phases.iter_mut().zip(full_params) {
                let taps = Self::advance_unison_lanes(phases, params, &s);

                for lane in 0..UNISON_LANES {
                    Self::accumulate_unison_lane(
                        &taps,
                        lane,
                        wave_from,
                        wave_to,
                        &mut acc_from[lane],
                        &mut acc_to[lane],
                    );
                }
            }

            if rem_lanes > 0 {
                let taps = Self::advance_unison_lanes(&mut rem_phases[0], &rem_params[0], &s);

                for lane in 0..rem_lanes {
                    Self::accumulate_unison_lane(
                        &taps,
                        lane,
                        wave_from,
                        wave_to,
                        &mut acc_from[lane],
                        &mut acc_to[lane],
                    );
                }
            }

            let acc_from = (acc_from[0] + acc_from[1]) + (acc_from[2] + acc_from[3]);
            let acc_to = (acc_to[0] + acc_to[1]) + (acc_to[2] + acc_to[3]);
            let acc = (acc_to - acc_from).mul_add(s.buff_t, acc_from);

            *out = acc.reduce_add() * unison_gain_delta.mul_add(buff_t, unison_gain_from);
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

        if inputs.pitch.is_some() {
            self.buffers.pitch[..samples].copy_from_slice(router.direct(inputs.pitch));
            Self::pitch_to_phase_inc(
                &self.buffers.pitch[..samples],
                &mut self.buffers.pitch_phase_inc[..samples],
                freq_phase_mult,
            );
        } else {
            let pitch = target.note_pitch();

            self.buffers.pitch[..samples].fill(pitch);
            self.buffers.pitch_phase_inc[..samples].fill(pitch_to_freq(pitch) * freq_phase_mult);
        }

        let mono_spectrum = self.params.mono_spectrum;
        let wave_channel = if mono_spectrum {
            LEFT_CHANNEL
        } else {
            channel_idx
        };

        if channel_idx == wave_channel {
            let last = samples.saturating_sub(1);

            Self::build_wave(
                self.inverse_fft.as_ref(),
                pitch_to_freq(self.buffers.pitch[last]) + self.buffers.frequency_shift[last],
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
            freq_phase_mult,
            samples,
        };
        let output = voice_output.output();
        let mut start = 0;

        for steal in &steals {
            let offset = (steal.offset as usize).min(ctx.samples);

            self.render_voice_samples(&ctx, output, start, offset);

            let phases = self.voices[channel_idx][voice_idx].phases;
            let requester = &mut self.voices[channel_idx][steal.voice_idx as usize];

            requester.phases = phases;

            if let Some(reset) = requester.phase_reset.as_mut() {
                reset.stolen = true;
            }
            start = offset;
        }

        self.render_voice_samples(&ctx, output, start, ctx.samples);

        if !mono_spectrum || channel_idx == RIGHT_CHANNEL {
            mem::swap(
                &mut self.voice_buffers[wave_channel][voice_idx].wave,
                &mut self.buffers.tmp_wave,
            );
        }
    }

    fn handle_trigger(
        &mut self,
        channel_idx: usize,
        replaced_voice_idx: Option<usize>,
        voice_idx: usize,
    ) {
        let voice = &mut self.voices[channel_idx][voice_idx];

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
            InputMeta::control(Input::DetunePower),
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

    fn set_input_slots(&mut self, inputs: &[InputSlots], spectral_inputs: &[SpectralInputSlot]) {
        self.inputs = Inputs::from_slots(inputs, spectral_inputs);
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
                    for channel_idx in 0..NUM_CHANNELS {
                        self.handle_trigger(channel_idx, *replaced_voice_idx, *voice_idx);
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
                    Input::DetunePower => self.set_detune_power(value),
                    Input::PhasesBlend => self.set_phases_blend(value),
                    Input::GainsBlend => self.set_gains_blend(value),
                    _ => (),
                },
                UiEvent::Unison(unison) => self.set_unison(unison),
                UiEvent::UnisonInitialPhase { idx, value } => self.set_initial_phase(idx, value),
                UiEvent::UnisonPhaseShift { idx, value } => self.set_unison_phase(idx, value),
                UiEvent::UnisonPhaseShiftTo { idx, value } => self.set_unison_phase_to(idx, value),
                UiEvent::UnisonGain { idx, value } => self.set_unison_gain(idx, value),
                UiEvent::UnisonGainTo { idx, value } => self.set_unison_gain_to(idx, value),
                UiEvent::StealPhase(steal_phase) => self.set_steal_phase(steal_phase),
                UiEvent::PhaseRandom(phase_random) => self.set_phase_random(phase_random),
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
