use std::{array, f32, mem, sync::Arc};

use itertools::izip;
use rand::RngExt;
use rand_pcg::Pcg32;
use realfft::{ComplexToReal, RealFftPlanner};
use wide::f32x4;

use crate::{
    synth_engine::{
        SmoothedSampleParams, StereoSample,
        buffer::{Buffer, SPECTRUM_BITS, VoicesLayout, new_voices_layout, zero_buffer},
        oscillator::link::{AudioEnd, UiEnd, UiEvent, create_link_pair},
        phase::Phase,
        routing::{
            AudioRouterType, DataType, Input, InputMeta, InputSlots, LEFT_CHANNEL, ModuleId,
            NUM_CHANNELS, ProcessContext, RIGHT_CHANNEL, RouterFactory, SamplesOutput,
            SpectralInputSlot, VoiceEvent, VoiceRouter, VoiceTarget,
        },
        smooth::SmoothedSample,
        synth_module::SynthModule,
        types::{ComplexSample, Sample},
    },
    utils::{db_to_gain, from_st, pan_gain, pitch_to_freq, power_scale},
};

mod config;
mod link;
mod ui_bridge;

#[cfg(test)]
mod tests;

pub use config::OscillatorConfig;
pub use link::Unison;
pub use ui_bridge::OscillatorUiBridge;

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
    pan: SmoothedSample,
    gain: SmoothedSample,
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
            pan: c.pan[channel_idx].into(),
            gain: c.gain[channel_idx].into(),
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
        self.pan.advance(smooth_params, samples);
        self.gain.advance(smooth_params, samples);
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

    #[inline(always)]
    fn interpolate(&self, t: Sample) -> Sample {
        (self.to - self.from).mul_add(t, self.from)
    }
}

struct PhaseReset {
    steal_from: Option<usize>,
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
    pan: Buffer,
    gain: Buffer,
    phase_shift: Buffer,
    frequency_shift: Buffer,
}

impl Default for Buffers {
    fn default() -> Self {
        Self {
            tmp_wave: Box::new([0.0; WAVEFORM_BUFFER_SIZE]),
            tmp_spectral: [ComplexSample::ZERO; DFT_BUFFER_SIZE],
            scratch: [ComplexSample::ZERO; DFT_BUFFER_SIZE],
            pan: zero_buffer(),
            gain: zero_buffer(),
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
    pan: InputSlots,
    gain: InputSlots,
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
            pan: InputSlots::new(Input::Pan),
            gain: InputSlots::new(Input::Gain),
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
                Input::Pan => result.pan = input.clone(),
                Input::Gain => result.gain = input.clone(),
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
            Input::Pan => self.pan.update_amount(src_slot, amount),
            Input::Gain => self.gain.update_amount(src_slot, amount),
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

type Router<'v, 'f, 'c> = VoiceRouter<'v, 'f, 'c, AudioRouterType>;

pub struct Oscillator {
    buffers: Buffers,
    inverse_fft: Arc<dyn ComplexToReal<Sample>>,
    random: Pcg32,
    id: ModuleId,
    params: Params,
    channel_params: [ChannelParams; NUM_CHANNELS],
    audio_end: AudioEnd,
    ui_end: Option<UiEnd>,
    inputs: Inputs,
    output_slot: usize,
    voices: VoicesLayout<Voice>,
    voice_buffers: VoicesLayout<VoiceBuffers>,
    last_voice_idx: Option<usize>,
}

impl Oscillator {
    pub fn new(id: ModuleId) -> Self {
        Self::from_config(&OscillatorConfig {
            id,
            ..OscillatorConfig::default()
        })
    }

    pub fn from_config(config: &config::OscillatorConfig) -> Self {
        let (audio_end, ui_end) = create_link_pair();

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
            ui_end: Some(ui_end),
            inputs: Inputs::default(),
            output_slot: usize::MAX,
            voices: new_voices_layout(),
            voice_buffers: new_voices_layout(),
            last_voice_idx: None,
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
            pan: get_smoothed_param!(self, pan),
            gain: get_smoothed_param!(self, gain),
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

    set_smoothed_param!(set_pan, pan, pan.clamp(-1.0, 1.0));
    set_smoothed_param!(set_gain, gain, gain.clamp(-1.0, 1.0));
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

    #[inline(always)]
    fn interpolated_segment(
        wave_from: &WaveformBuffer,
        wave_to: &WaveformBuffer,
        buff_t: Sample,
        idx: usize,
        t: Sample,
    ) -> f32x4 {
        const B0: f32x4 = f32x4::new([-1.0 / 2.0, 3.0 / 2.0, -3.0 / 2.0, 1.0 / 2.0]);
        const B1: f32x4 = f32x4::new([1.0, -5.0 / 2.0, 4.0 / 2.0, -1.0 / 2.0]);
        const B2: f32x4 = f32x4::new([-1.0 / 2.0, 0.0 / 2.0, 1.0 / 2.0, 0.0 / 2.0]);
        const B3: f32x4 = f32x4::new([0.0 / 2.0, 1.0, 0.0 / 2.0, 0.0 / 2.0]);

        let c_from = Self::load_segment(wave_from, idx);
        let c_to = Self::load_segment(wave_to, idx);

        let c = (c_to - c_from).mul_add(f32x4::splat(buff_t), c_from);
        let t = f32x4::splat(t);
        let poly = B0.mul_add(t, B1).mul_add(t, B2).mul_add(t, B3);

        poly * c
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
        rf: &mut RouterFactory<AudioRouterType>,
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
        router: &mut Router<'_, '_, '_>,
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
        router: &mut Router<'_, '_, '_>,
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
        router: &mut Router<'_, '_, '_>,
    ) {
        let Some(phase_reset) = self.voices[channel_idx][voice_idx].phase_reset.take() else {
            return;
        };

        let channel = &self.channel_params[channel_idx];
        let voices = &mut self.voices[channel_idx];
        let unison = self.params.unison;
        let voice = &mut voices[voice_idx];

        // When steal_phase set to false - control that toggle by input value
        let steal_phase = router.scalar(
            &self.inputs.phase_steal,
            Sample::from(self.params.steal_phase),
            true,
        ) >= 0.5;

        if let Some(replaced_voice_idx) = phase_reset.steal_from
            && steal_phase
        {
            voices[voice_idx].phases = voices[replaced_voice_idx].phases;
        } else if self.params.phase_random > 1e-6 {
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

            if unison & 1 == 1 && channel_idx == RIGHT_CHANNEL {
                let center = unison / 2;

                self.voices[channel_idx][voice_idx].phases[center] =
                    self.voices[LEFT_CHANNEL][voice_idx].phases[center];
            }
        } else if unison > 1 {
            for (phase, unison_voice) in voice.phases.iter_mut().zip(&channel.unison).take(unison) {
                *phase = Phase::from_normalized(unison_voice.initial_phase);
            }
        } else {
            voice.phases[0] = Phase::ZERO;
        }
    }

    fn process_voice(
        &mut self,
        target: &VoiceTarget,
        outputs: &mut VoicesLayout<SamplesOutput>,
        rf: &mut RouterFactory<AudioRouterType>,
    ) {
        let channel_idx = target.channel_idx;
        let voice_idx = target.voice_idx;
        let (mut router, mut voice_output) = rf.for_voice(target, outputs);

        self.process_phase_reset(channel_idx, voice_idx, &mut router);
        self.process_unison(channel_idx, voice_idx, &mut router);

        let samples = router.samples();
        let channel = &self.channel_params[channel_idx];
        let voice = &mut self.voices[channel_idx][voice_idx];
        let inputs = &self.inputs;
        let buffers = &mut self.buffers;

        router.param(
            &inputs.phase_shift,
            &channel.phase_shift,
            &mut buffers.phase_shift,
        );
        router.param(
            &inputs.freq_shift,
            &channel.frequency_shift,
            &mut buffers.frequency_shift,
        );

        let pitch_in = router.direct(inputs.pitch);

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
                pitch_to_freq(pitch_in[last]) + buffers.frequency_shift[last],
                router.sample_rate(),
                router.spectral(inputs.spectrum),
                &mut buffers.tmp_spectral,
                &mut buffers.scratch,
                &mut buffers.tmp_wave,
            );
        }

        if router.need_update_ui_mono() {
            self.audio_end
                .update_spectrum(router.spectral(inputs.spectrum));
        }

        let freq_phase_mult = Phase::freq_phase_mult(router.sample_rate());
        let buff_t_inc = (samples as f32).recip();
        let mut buff_t = 0.0;
        let output = voice_output.output();

        let wave_from = &self.voice_buffers[wave_channel][voice_idx].wave;
        let wave_to = &buffers.tmp_wave;

        for (out, &pitch, &phase_shift, freq_shift) in izip!(
            output.iter_mut(),
            pitch_in,
            &buffers.phase_shift,
            &buffers.frequency_shift,
        ) {
            let mut sample_acc = f32x4::ZERO;
            let phase_shift = Phase::from_normalized(phase_shift);
            let pitch_phase_inc = pitch_to_freq(pitch) * freq_phase_mult;
            let freq_phase_inc = freq_shift * freq_phase_mult;

            for (phase, uv) in voice
                .phases
                .iter_mut()
                .zip(voice.unison.iter())
                .take(self.params.unison)
            {
                let read_phase = *phase
                    + phase_shift
                    + Phase::from_normalized(uv.phase_shift.interpolate(buff_t));
                let idx = read_phase.wave_index::<WAVEFORM_BITS>();
                let t = read_phase.wave_index_fraction::<WAVEFORM_BITS>();
                let segment = Self::interpolated_segment(wave_from, wave_to, buff_t, idx, t);

                sample_acc = segment.mul_add(f32x4::splat(uv.gain.interpolate(buff_t)), sample_acc);
                *phase += pitch_phase_inc.mul_add(uv.rate.interpolate(buff_t), freq_phase_inc);
            }

            *out = sample_acc.reduce_add() * voice.unison_gain.interpolate(buff_t);
            buff_t += buff_t_inc;
        }

        if !router.param_stationary_at(&inputs.pan, &channel.pan, 0.0) {
            router.param(&inputs.pan, &channel.pan, &mut buffers.pan);

            for (out, &pan) in output.iter_mut().zip(&buffers.pan) {
                *out *= pan_gain(pan, channel_idx);
            }
        }

        if !router.param_stationary_at(&inputs.gain, &channel.gain, 1.0) {
            router.param(&inputs.gain, &channel.gain, &mut buffers.gain);

            for (out, gain) in output.iter_mut().zip(&buffers.gain) {
                *out *= gain;
            }
        }

        if !mono_spectrum || channel_idx == RIGHT_CHANNEL {
            mem::swap(
                &mut self.voice_buffers[wave_channel][voice_idx].wave,
                &mut buffers.tmp_wave,
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
            steal_from: replaced_voice_idx.or(self.last_voice_idx),
        });
    }
}

impl SynthModule for Oscillator {
    fn id(&self) -> ModuleId {
        self.id
    }

    fn inputs(&self) -> &'static [InputMeta] {
        static INPUTS: &[InputMeta] = &[
            InputMeta::spectral(Input::Spectrum),
            InputMeta::direct_control(Input::Pitch),
            InputMeta::control(Input::Gain),
            InputMeta::control(Input::Pan),
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

                    self.last_voice_idx = Some(*voice_idx);
                }
                VoiceEvent::Update { voice_idx, .. } => {
                    self.last_voice_idx = Some(*voice_idx);
                }
                _ => (),
            }
        }
    }

    fn process_ui_events(&mut self) {
        while let Some(event) = self.audio_end.pop_event() {
            match event {
                UiEvent::InputParam { input, value } => match input {
                    Input::Pan => self.set_pan(value),
                    Input::Gain => self.set_gain(value),
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

    fn process(&mut self, ctx: &mut ProcessContext) {
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
