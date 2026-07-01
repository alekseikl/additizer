use std::{array, convert::identity, f32, sync::Arc};

use itertools::izip;
use nih_plug::util::db_to_gain;
use rand::RngExt;
use rand_pcg::Pcg32;
use realfft::{ComplexToReal, RealFftPlanner};
use wide::f32x4;

use crate::{
    synth_engine::{
        StereoSample,
        buffer::{
            Buffer, SPECTRUM_BITS, SpectralBuffer, VoicesLayout, add_buffer_value,
            new_voices_layout, zero_buffer,
        },
        oscillator::link::{AudioEnd, UiEnd, UiEvent, create_link_pair},
        phase::Phase,
        routing::{
            AudioRouterType, DataType, Input, InputMeta, InputSlots, ModuleId, NUM_CHANNELS,
            ProcessContext, SamplesOutput, SpectralInputSlot, VoiceEvent, VoiceRouter,
        },
        smooth::SmoothedSample,
        synth_module::SynthModule,
        types::{ComplexSample, Sample},
    },
    utils::{from_ms, pitch_to_freq, power_scale, st_to_octave},
};

mod config;
mod link;
mod ui_bridge;

#[cfg(test)]
mod tests;

pub use config::OscillatorConfig;
pub use ui_bridge::OscillatorUiBridge;

const WAVEFORM_BITS: usize = SPECTRUM_BITS + 1;
const WAVEFORM_SIZE: usize = 1 << WAVEFORM_BITS;
const WAVEFORM_PAD_LEFT: usize = 1;
const WAVEFORM_PAD_RIGHT: usize = 2;
const WAVEFORM_BUFFER_SIZE: usize = WAVEFORM_SIZE + WAVEFORM_PAD_LEFT + WAVEFORM_PAD_RIGHT;
const DFT_BUFFER_SIZE: usize = (1 << (WAVEFORM_BITS - 1)) + 1;

pub const MAX_UNISON_VOICES: usize = 16;
const MAX_GLIDE: Sample = 5.0;

type WaveformBuffer = [Sample; WAVEFORM_BUFFER_SIZE];
type DftBuffer = [ComplexSample; DFT_BUFFER_SIZE];

const fn make_zero_wave_buffer() -> WaveformBuffer {
    [0.0; WAVEFORM_BUFFER_SIZE]
}

const fn zero_dft_buffer() -> DftBuffer {
    [ComplexSample::ZERO; DFT_BUFFER_SIZE]
}

struct Params {
    unison: usize,
    steal_phase: bool,
}

impl Params {
    fn from_config(c: &config::OscillatorConfig) -> Self {
        Self {
            unison: c.unison_voices,
            steal_phase: c.steal_phase,
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
    gain: SmoothedSample,
    pitch_shift: SmoothedSample, //Octaves
    detune: Sample,              //Octaves
    detune_power: Sample,
    glide: Sample,
    glide_slope: Sample,
    phase_shift: SmoothedSample,
    frequency_shift: SmoothedSample,
    phases_blend: Sample,
    gains_blend: Sample,
    unison: [UnisonParams; MAX_UNISON_VOICES],
}

impl ChannelParams {
    fn from_config(c: &OscillatorConfig, channel_idx: usize) -> Self {
        Self {
            gain: c.gain[channel_idx].into(),
            pitch_shift: c.pitch_shift[channel_idx].into(),
            detune: c.detune[channel_idx],
            detune_power: c.detune_power[channel_idx],
            glide: c.glide[channel_idx],
            glide_slope: c.glide_slope[channel_idx],
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

struct Glide {
    t: Sample,
    pitch_from: Sample,
    current_pitch: Sample,
}

impl Glide {
    fn new(pitch_from: Sample) -> Self {
        Self {
            pitch_from,
            current_pitch: pitch_from,
            t: 0.0,
        }
    }
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

struct VoiceState {
    triggered: bool,
    pitch: Sample, // Octave units
    glide: Option<Glide>,
    unison_gain: Interpolated,
    unison: [UnisonVoice; MAX_UNISON_VOICES],
    phases: [Phase; MAX_UNISON_VOICES],
}

impl Default for VoiceState {
    fn default() -> Self {
        Self {
            triggered: false,
            pitch: 0.0,
            glide: None,
            phases: Default::default(),
            unison_gain: Interpolated { from: 1.0, to: 1.0 },
            unison: Default::default(),
        }
    }
}

struct VoiceBuffers {
    wave_buffers: (WaveformBuffer, WaveformBuffer),
    wave_buffers_swapped: bool,
}

impl Default for VoiceBuffers {
    fn default() -> Self {
        Self {
            wave_buffers_swapped: false,
            wave_buffers: (make_zero_wave_buffer(), make_zero_wave_buffer()),
        }
    }
}

struct Buffers {
    tmp_spectral: DftBuffer,
    scratch: DftBuffer,
    gain: Buffer,
    pitch: Buffer,
    phase_shift: Buffer,
    frequency_shift: Buffer,
}

impl Default for Buffers {
    fn default() -> Self {
        Self {
            tmp_spectral: zero_dft_buffer(),
            scratch: zero_dft_buffer(),
            gain: zero_buffer(),
            pitch: zero_buffer(),
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
    gain: InputSlots,
    pitch_shift: InputSlots,
    phase_shift: InputSlots,
    freq_shift: InputSlots,
    detune: InputSlots,
    detune_power: InputSlots,
    glide: InputSlots,
    glide_slope: InputSlots,
    phases_blend: InputSlots,
    gains_blend: InputSlots,
}

impl Default for Inputs {
    fn default() -> Self {
        Self {
            spectrum: None,
            gain: InputSlots::empty(Input::Gain),
            pitch_shift: InputSlots::empty(Input::PitchShift),
            phase_shift: InputSlots::empty(Input::PhaseShift),
            freq_shift: InputSlots::empty(Input::FrequencyShift),
            detune: InputSlots::empty(Input::Detune),
            detune_power: InputSlots::empty(Input::DetunePower),
            glide: InputSlots::empty(Input::Glide),
            glide_slope: InputSlots::empty(Input::GlideSlope),
            phases_blend: InputSlots::empty(Input::PhasesBlend),
            gains_blend: InputSlots::empty(Input::GainsBlend),
        }
    }
}

impl Inputs {
    fn from_slots(inputs: &[InputSlots], spectral_inputs: &[SpectralInputSlot]) -> Self {
        let mut result = Self::default();

        for input in inputs {
            match input.input_type {
                Input::Gain => result.gain = input.clone(),
                Input::PitchShift => result.pitch_shift = input.clone(),
                Input::PhaseShift => result.phase_shift = input.clone(),
                Input::FrequencyShift => result.freq_shift = input.clone(),
                Input::Detune => result.detune = input.clone(),
                Input::DetunePower => result.detune_power = input.clone(),
                Input::Glide => result.glide = input.clone(),
                Input::GlideSlope => result.glide_slope = input.clone(),
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
            Input::Gain => self.gain.update_amount(src_slot, amount),
            Input::PitchShift => self.pitch_shift.update_amount(src_slot, amount),
            Input::PhaseShift => self.phase_shift.update_amount(src_slot, amount),
            Input::FrequencyShift => self.freq_shift.update_amount(src_slot, amount),
            Input::Detune => self.detune.update_amount(src_slot, amount),
            Input::DetunePower => self.detune_power.update_amount(src_slot, amount),
            Input::Glide => self.glide.update_amount(src_slot, amount),
            Input::GlideSlope => self.glide_slope.update_amount(src_slot, amount),
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
    voices: VoicesLayout<VoiceState>,
    voice_buffers: VoicesLayout<VoiceBuffers>,
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

        Self {
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
        }
    }

    pub fn get_config(&self) -> OscillatorConfig {
        OscillatorConfig {
            id: self.id,
            unison_voices: self.params.unison,
            steal_phase: self.params.steal_phase,
            gain: get_smoothed_param!(self, gain),
            pitch_shift: get_smoothed_param!(self, pitch_shift),
            detune: get_stereo_param!(self, detune),
            detune_power: get_stereo_param!(self, detune_power),
            glide: get_stereo_param!(self, glide),
            glide_slope: get_stereo_param!(self, glide_slope),
            phase_shift: get_smoothed_param!(self, phase_shift),
            frequency_shift: get_smoothed_param!(self, frequency_shift),
            phases_blend: get_stereo_param!(self, phases_blend),
            gains_blend: get_stereo_param!(self, gains_blend),
            unison: array::from_fn(|i| config::UnisonConfig {
                initial_phase: get_unison_param!(self, initial_phase, i),
                phase_shift: get_unison_param!(self, phase_shift, i),
                phase_shift_to: get_unison_param!(self, phase_shift_to, i),
                gain: get_unison_param!(self, gain, i),
                gain_to: get_unison_param!(self, gain_to, i),
            }),
        }
    }

    set_mono_param!(
        set_unison,
        unison,
        usize,
        unison.clamp(1, MAX_UNISON_VOICES)
    );
    set_mono_param!(set_steal_phase, steal_phase, bool);

    set_smoothed_param!(set_gain, gain, gain.clamp(0.0, 1.0));
    set_smoothed_param!(
        set_pitch_shift,
        pitch_shift,
        pitch_shift.clamp(st_to_octave(-60.0), st_to_octave(60.0))
    );
    set_stereo_param!(set_detune, detune, detune.clamp(0.0, st_to_octave(1.0)));
    set_stereo_param!(
        set_detune_power,
        detune_power,
        detune_power.clamp(-5.0, 5.0)
    );

    set_stereo_param!(set_glide, glide, glide.clamp(0.0, MAX_GLIDE));
    set_stereo_param!(set_glide_slope, glide_slope, glide_slope.clamp(-1.0, 1.0));

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

        self.audio_end.push_refresh_state();
    }

    pub fn randomize_phases(
        &mut self,
        from: Sample,
        to: Sample,
        stereo_spread: Sample,
        dst: PhasesDst,
    ) {
        let from = from.clamp(0.0, 1.0);
        let to = to.clamp(0.0, 1.0);

        let randoms: [StereoSample; MAX_UNISON_VOICES] = array::from_fn(|_| {
            let left = from + (to - from) * self.random.random::<Sample>();
            let right = left - 0.5 * stereo_spread + stereo_spread * self.random.random::<Sample>();

            StereoSample::new(left, right).clamp(0.0, 1.0)
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

        self.audio_end.push_refresh_state();
    }

    #[inline]
    fn get_wave_slice_mut(wave_buff: &mut WaveformBuffer) -> &mut [Sample] {
        &mut wave_buff[WAVEFORM_PAD_LEFT..(WAVEFORM_BUFFER_SIZE - WAVEFORM_PAD_RIGHT)]
    }

    #[inline]
    fn get_display_wave_slice(wave_buff: &WaveformBuffer) -> &[Sample] {
        &wave_buff[WAVEFORM_PAD_LEFT..(WAVEFORM_BUFFER_SIZE - WAVEFORM_PAD_RIGHT + 1)]
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

    #[inline(always)]
    fn wrap_wave_buffer(wave_buff: &mut WaveformBuffer) {
        wave_buff[0] = wave_buff[WAVEFORM_BUFFER_SIZE - WAVEFORM_PAD_RIGHT - 1];
        wave_buff[WAVEFORM_BUFFER_SIZE - WAVEFORM_PAD_RIGHT] = wave_buff[WAVEFORM_PAD_LEFT];
        wave_buff[WAVEFORM_BUFFER_SIZE - WAVEFORM_PAD_RIGHT + 1] = wave_buff[WAVEFORM_PAD_LEFT + 1];
    }

    fn build_wave(
        inverse_fft: &dyn ComplexToReal<Sample>,
        frequency: f32,
        sample_rate: f32,
        spectral_buff: &SpectralBuffer,
        tmp_spectral_buff: &mut DftBuffer,
        scratch_buff: &mut DftBuffer,
        out_wave_buff: &mut WaveformBuffer,
    ) {
        let frequency = frequency.abs();
        let max_frequency = 0.5 * sample_rate;

        let cutoff_index =
            ((max_frequency / frequency).floor() as usize + 1).min(spectral_buff.len());

        tmp_spectral_buff[..cutoff_index].copy_from_slice(&spectral_buff[..cutoff_index]);
        tmp_spectral_buff[cutoff_index..].fill(ComplexSample::ZERO);

        inverse_fft
            .process_with_scratch(
                tmp_spectral_buff,
                Self::get_wave_slice_mut(out_wave_buff),
                scratch_buff,
            )
            .unwrap();
        Self::wrap_wave_buffer(out_wave_buff);
    }

    fn build_waveforms(
        inverse_fft: &dyn ComplexToReal<Sample>,
        inputs: &Inputs,
        voice_buffers: &mut VoiceBuffers,
        buffers: &mut Buffers,
        triggered: bool,
        router: &Router<'_, '_, '_>,
    ) {
        if triggered {
            let spectrum_from = router.spectral(inputs.spectrum, true);

            Self::build_wave(
                inverse_fft,
                pitch_to_freq(buffers.pitch[0]) + buffers.frequency_shift[0],
                router.sample_rate(),
                spectrum_from,
                &mut buffers.tmp_spectral,
                &mut buffers.scratch,
                &mut voice_buffers.wave_buffers.0,
            );

            voice_buffers.wave_buffers_swapped = false;
        }

        let spectrum = router.spectral(inputs.spectrum, false);

        let wave_to = if voice_buffers.wave_buffers_swapped {
            &mut voice_buffers.wave_buffers.0
        } else {
            &mut voice_buffers.wave_buffers.1
        };

        Self::build_wave(
            inverse_fft,
            pitch_to_freq(buffers.pitch[router.samples() - 1])
                + buffers.frequency_shift[router.samples() - 1],
            router.sample_rate(),
            spectrum,
            &mut buffers.tmp_spectral,
            &mut buffers.scratch,
            wave_to,
        );
        voice_buffers.wave_buffers_swapped = !voice_buffers.wave_buffers_swapped;
    }

    fn process_unison(
        unison: usize,
        channel: &ChannelParams,
        inputs: &Inputs,
        voice: &mut VoiceState,
        router: &mut Router<'_, '_, '_>,
    ) {
        const MAX_DETUNE: Sample = 1.0;
        const MAX_DETUNE_POWER: Sample = 5.0;

        if unison < 2 {
            voice.unison[0] = UnisonVoice::default();
            voice.unison_gain = Interpolated { from: 1.0, to: 1.0 };
            return;
        }

        struct StateUpdate {
            rate: Sample,
            phase_shift: Sample,
            gain: Sample,
        }

        fn calc_update(
            unison: usize,
            triggered: bool,
            channel: &ChannelParams,
            inputs: &Inputs,
            router: &mut Router<'_, '_, '_>,
        ) -> impl Iterator<Item = StateUpdate> {
            let detune = router
                .scalar_param(&inputs.detune, channel.detune, triggered)
                .clamp(0.0, MAX_DETUNE);

            let detune_power = router
                .scalar_param(&inputs.detune_power, channel.detune_power, triggered)
                .clamp(-MAX_DETUNE_POWER, MAX_DETUNE_POWER);

            let phases_blend = router
                .scalar_param(&inputs.phases_blend, channel.phases_blend, triggered)
                .clamp(0.0, 1.0);

            let gains_blend = router
                .scalar_param(&inputs.gains_blend, channel.gains_blend, triggered)
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

                    StateUpdate {
                        rate: (power_scale(spread.abs(), detune_power).copysign(spread) * detune)
                            .exp2(),
                        phase_shift: (param.phase_shift_to - param.phase_shift)
                            .mul_add(phases_blend, param.phase_shift),
                        gain: (param.gain_to - param.gain).mul_add(gains_blend, param.gain),
                    }
                })
        }

        fn cal_unison_gain(gains: impl Iterator<Item = Sample>) -> Sample {
            gains
                .map(|gain| gain * gain)
                .sum::<Sample>()
                .sqrt()
                .max(1.0) //Don't amplify
                .recip()
        }

        if voice.triggered {
            for (state, update) in izip!(
                &mut voice.unison,
                calc_update(unison, true, channel, inputs, router)
            ) {
                state.rate.from = update.rate;
                state.phase_shift.from = update.phase_shift;
                state.gain.from = update.gain;
            }

            voice.unison_gain.from = cal_unison_gain(
                voice
                    .unison
                    .iter()
                    .take(unison)
                    .map(|state| state.gain.from),
            );
        } else {
            for state in voice.unison.iter_mut().take(unison) {
                state.rate.advance();
                state.phase_shift.advance();
                state.gain.advance();
            }

            voice.unison_gain.advance();
        }

        for (state, update) in izip!(
            &mut voice.unison,
            calc_update(unison, false, channel, inputs, router)
        ) {
            state.rate.to = update.rate;
            state.phase_shift.to = update.phase_shift;
            state.gain.to = update.gain;
        }

        voice.unison_gain.to =
            cal_unison_gain(voice.unison.iter().take(unison).map(|state| state.gain.to));
    }

    fn process_glide(
        channel: &ChannelParams,
        inputs: &Inputs,
        buffers: &mut Buffers,
        voice: &mut VoiceState,
        router: &mut Router<'_, '_, '_>,
    ) {
        const GLIDE_TIME_THRESHOLD: Sample = from_ms(1.0);
        const GLIDE_POWER_MAX: Sample = 6.0;
        const POWER_LINEAR_THRESHOLD: Sample = 0.005;

        let pitch = voice.pitch;

        let Some(glide) = voice.glide.as_mut() else {
            return;
        };

        let glide_time = router
            .scalar_param(&inputs.glide, channel.glide, false)
            .clamp(0.0, MAX_GLIDE);
        let time_left = glide_time - glide.t;

        if glide_time < GLIDE_TIME_THRESHOLD || time_left <= 0.0 {
            voice.glide = None;
            return;
        }

        let glide_slope = router
            .scalar_param(&inputs.glide_slope, channel.glide_slope, false)
            .clamp(-1.0, 1.0);
        let glide_power = -glide_slope * GLIDE_POWER_MAX;
        let t_step = router.sample_rate().recip();
        let samples = router
            .samples()
            .min((time_left * router.sample_rate()) as usize);
        let pitch_buff = &mut buffers.pitch[..samples];

        #[inline(always)]
        fn process(
            buff: &mut [Sample],
            glide: &mut Glide,
            glide_time: Sample,
            pitch: Sample,
            t_step: Sample,
            curve: impl Fn(Sample) -> Sample,
        ) {
            let pitch_diff = pitch - glide.pitch_from;
            let glide_time_recip = glide_time.recip();

            for out_pitch in buff {
                let diff = pitch_diff * (1.0 - curve(glide.t * glide_time_recip));

                glide.current_pitch = pitch - diff;
                *out_pitch -= diff;
                glide.t += t_step;
            }
        }

        if glide_power.abs() < POWER_LINEAR_THRESHOLD {
            process(pitch_buff, glide, glide_time, pitch, t_step, identity);
        } else {
            let denominator_mult = (glide_power.exp() - 1.0).recip();

            process(pitch_buff, glide, glide_time, pitch, t_step, |v| {
                ((v * glide_power).exp() - 1.0) * denominator_mult
            });
        }

        if samples < router.samples() {
            voice.glide = None;
        }
    }

    fn process_voice(
        &mut self,
        mono_spectrum: bool,
        output: &mut VoicesLayout<SamplesOutput>,
        mut router: Router<'_, '_, '_>,
    ) {
        let channel_idx = router.channel_idx();
        let voice_idx = router.voice_idx();
        let inputs = &self.inputs;
        let buffers = &mut self.buffers;
        let channel = &mut self.channel_params[channel_idx];
        let voice = &mut self.voices[channel_idx][voice_idx];
        let output = output[channel_idx][voice_idx].output(router.samples());
        let samples = router.samples();

        router.buff_param(&inputs.gain, &mut channel.gain, &mut buffers.gain);

        router.buff_param(
            &inputs.pitch_shift,
            &mut channel.pitch_shift,
            &mut buffers.pitch,
        );

        add_buffer_value(&mut buffers.pitch[..samples], voice.pitch);

        router.buff_param(
            &inputs.phase_shift,
            &mut channel.phase_shift,
            &mut buffers.phase_shift,
        );

        router.buff_param(
            &inputs.freq_shift,
            &mut channel.frequency_shift,
            &mut buffers.frequency_shift,
        );

        let voice_buffers = if mono_spectrum && channel_idx != 0 {
            &self.voice_buffers[0][voice_idx]
        } else {
            let vb = &mut self.voice_buffers[channel_idx][voice_idx];

            Self::build_waveforms(
                self.inverse_fft.as_ref(),
                inputs,
                vb,
                buffers,
                voice.triggered,
                &router,
            );
            vb
        };

        let (wave_from, wave_to) = if voice_buffers.wave_buffers_swapped {
            (&voice_buffers.wave_buffers.0, &voice_buffers.wave_buffers.1)
        } else {
            (&voice_buffers.wave_buffers.1, &voice_buffers.wave_buffers.0)
        };

        if router.need_update_ui() {
            self.audio_end
                .update_waveform(Self::get_display_wave_slice(wave_from));
        }

        Self::process_unison(self.params.unison, channel, inputs, voice, &mut router);
        Self::process_glide(channel, inputs, buffers, voice, &mut router);

        if voice.triggered {
            voice.triggered = false;
        }

        let freq_phase_mult = Phase::freq_phase_mult(router.sample_rate());
        let buff_t_mult = (samples as f32).recip();

        for (out, pitch, phase_shift, freq_shift, gain, sample_idx) in izip!(
            output,
            &buffers.pitch,
            &buffers.phase_shift,
            &buffers.frequency_shift,
            &buffers.gain,
            0..samples
        ) {
            let mut sample_acc = f32x4::splat(0.0);
            let buff_t = sample_idx as Sample * buff_t_mult;
            let phase_shift = Phase::from_normalized(*phase_shift);
            let pitch_phase_inc = pitch_to_freq(*pitch) * freq_phase_mult;
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

            *out = sample_acc.reduce_add() * gain * voice.unison_gain.interpolate(buff_t);
        }
    }

    fn handle_trigger(
        &mut self,
        channel_idx: usize,
        prev_voice_idx: Option<usize>,
        voice_idx: usize,
        pitch: Sample,
    ) {
        let channel = &self.channel_params[channel_idx];
        let voices = &mut self.voices[channel_idx];

        if let Some(prev_voice_idx) = prev_voice_idx {
            let prev_voice_state = &voices[prev_voice_idx];
            let prev_pitch = prev_voice_state
                .glide
                .as_ref()
                .map_or(prev_voice_state.pitch, |g| g.current_pitch);

            voices[voice_idx].glide = Some(Glide::new(prev_pitch));
        } else {
            voices[voice_idx].glide = None;
        }

        let voice = &mut voices[voice_idx];

        voice.pitch = pitch;
        voice.triggered = true;

        if let Some(prev_voice_idx) = prev_voice_idx
            && self.params.steal_phase
        {
            voices[voice_idx].phases = voices[prev_voice_idx].phases;
        } else {
            for (phase, unison_voice) in voice.phases.iter_mut().zip(&channel.unison) {
                *phase = Phase::from_normalized(unison_voice.initial_phase);
            }
        }
    }

    fn handle_update(&mut self, channel_idx: usize, voice_idx: usize, pitch: Sample) {
        let voice = &mut self.voices[channel_idx][voice_idx];

        voice.glide = Some(Glide::new(
            voice
                .glide
                .as_ref()
                .map_or(voice.pitch, |g| g.current_pitch),
        ));
        voice.pitch = pitch;
    }
}

impl SynthModule for Oscillator {
    fn id(&self) -> ModuleId {
        self.id
    }

    fn inputs(&self) -> &'static [InputMeta] {
        static INPUTS: &[InputMeta] = &[
            InputMeta::spectral(Input::Spectrum),
            InputMeta::control(Input::Gain),
            InputMeta::control(Input::PitchShift),
            InputMeta::audio_mixed(Input::PhaseShift),
            InputMeta::audio_mixed(Input::FrequencyShift),
            InputMeta::control(Input::Detune),
            InputMeta::control(Input::DetunePower),
            InputMeta::control(Input::Glide),
            InputMeta::control(Input::GlideSlope),
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
        for channel_idx in 0..NUM_CHANNELS {
            for event in events {
                match event {
                    VoiceEvent::Trigger {
                        voice_idx,
                        prev_voice_idx,
                        pitch,
                        ..
                    } => self.handle_trigger(channel_idx, *prev_voice_idx, *voice_idx, *pitch),
                    VoiceEvent::Update {
                        voice_idx, pitch, ..
                    } => self.handle_update(channel_idx, *voice_idx, *pitch),
                    _ => (),
                }
            }
        }
    }

    fn process_ui_events(&mut self) {
        while let Some(event) = self.audio_end.pop_event() {
            match event {
                UiEvent::InputParam { input, value } => match input {
                    Input::Gain => self.set_gain(value),
                    Input::PitchShift => self.set_pitch_shift(value),
                    Input::PhaseShift => self.set_phase_shift(value),
                    Input::FrequencyShift => self.set_frequency_shift(value),
                    Input::Detune => self.set_detune(value),
                    Input::DetunePower => self.set_detune_power(value),
                    Input::Glide => self.set_glide(value),
                    Input::GlideSlope => self.set_glide_slope(value),
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
                UiEvent::ApplyUnisonLevelShape { center, level, to } => {
                    self.apply_unison_level_shape(center, level, to);
                }
                UiEvent::RandomizePhases {
                    from,
                    to,
                    stereo_spread,
                    dst,
                } => {
                    self.randomize_phases(from, to, stereo_spread, dst);
                }
            }
        }
    }

    fn process(&mut self, ctx: &mut ProcessContext) {
        ctx.for_audio(self.id, self.output_slot, |router, output| {
            let mono_spectrum = router.params().spectrum_channels < NUM_CHANNELS;
            let num_active_voices = router.params().active_voices.len();

            for channel_idx in 0..NUM_CHANNELS {
                for seq_idx in 0..num_active_voices {
                    let voice_idx = router.params().active_voices[seq_idx];

                    self.process_voice(
                        mono_spectrum,
                        output,
                        router.for_voice(channel_idx, voice_idx, seq_idx),
                    );
                }
            }
        });
    }
}
