use std::{any::Any, f32, sync::Arc};

use itertools::izip;
use nih_plug::util::db_to_gain;
use realfft::{ComplexToReal, RealFftPlanner};
use serde::{Deserialize, Serialize};
use wide::f32x4;

use crate::{
    synth_engine::{
        StereoSample,
        buffer::{Buffer, SPECTRUM_BITS, SpectralBuffer, zero_buffer},
        phase::Phase,
        routing::{DataType, Input, MAX_VOICES, ModuleId, ModuleType, NUM_CHANNELS, Router},
        synth_module::{
            InputInfo, ModuleConfigBox, NoteOnParams, ProcessParams, SynthModule, VoiceRouter,
        },
        types::{ComplexSample, Sample},
    },
    utils::{note_to_octave, octave_to_freq, st_to_octave},
};

const WAVEFORM_BITS: usize = SPECTRUM_BITS + 1;
const WAVEFORM_SIZE: usize = 1 << WAVEFORM_BITS;
const WAVEFORM_PAD_LEFT: usize = 1;
const WAVEFORM_PAD_RIGHT: usize = 2;
const WAVEFORM_BUFFER_SIZE: usize = WAVEFORM_SIZE + WAVEFORM_PAD_LEFT + WAVEFORM_PAD_RIGHT;

const DFT_BUFFER_SIZE: usize = (1 << (WAVEFORM_BITS - 1)) + 1;

const MAX_UNISON_VOICES: usize = 16;

const INITIAL_PHASES: [Sample; MAX_UNISON_VOICES] = [
    0.0, 0.9068176, 0.6544455, 0.26577616, 0.24667478, 0.12834072, 0.5805929, 0.55541587,
    0.58291245, 0.03298676, 0.8845756, 0.96093744, 0.42001683, 0.63606197, 0.28810132, 0.5167134,
];

type WaveformBuffer = [Sample; WAVEFORM_BUFFER_SIZE];
type DftBuffer = [ComplexSample; DFT_BUFFER_SIZE];

const fn make_zero_wave_buffer() -> WaveformBuffer {
    [0.0; WAVEFORM_BUFFER_SIZE]
}

const fn zero_dft_buffer() -> DftBuffer {
    [ComplexSample::ZERO; DFT_BUFFER_SIZE]
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Params {
    unison: usize,
    reset_phase: bool,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            unison: 1,
            reset_phase: false,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ChannelParams {
    gain: Sample,
    pitch_shift: Sample, //Octaves
    detune: Sample,      //Octaves
    phase_shift: Sample,
    frequency_shift: Sample,
    unison_phases: [Sample; MAX_UNISON_VOICES],
    unison_gains: [Sample; MAX_UNISON_VOICES],
}

impl Default for ChannelParams {
    fn default() -> Self {
        Self {
            gain: 1.0,
            pitch_shift: 0.0,
            detune: st_to_octave(0.2),
            phase_shift: 0.0,
            frequency_shift: 0.0,
            unison_phases: INITIAL_PHASES,
            unison_gains: [1.0; MAX_UNISON_VOICES],
        }
    }
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct OscillatorConfig {
    label: Option<String>,
    params: Params,
    channels: [ChannelParams; NUM_CHANNELS],
}

pub struct OscillatorUIData {
    pub label: String,
    pub gain: StereoSample,
    pub pitch_shift: StereoSample,
    pub detune: StereoSample,
    pub phase_shift: StereoSample,
    pub frequency_shift: StereoSample,
    pub unison: usize,
    pub reset_phase: bool,
    pub unison_phases: [StereoSample; MAX_UNISON_VOICES],
    pub unison_gains: [StereoSample; MAX_UNISON_VOICES],
}

struct UnisonVoice {
    rate_from: Sample,
    rate_to: Sample,
    rate_diff: Sample,
    gain: Sample,
    phase_shift: Sample,
}

impl Default for UnisonVoice {
    fn default() -> Self {
        Self {
            rate_from: 1.0,
            rate_to: 1.0,
            rate_diff: 0.0,
            gain: 1.0,
            phase_shift: 0.0,
        }
    }
}

struct VoiceState {
    octave: Sample,
    triggered: bool,
    unison_gain: Sample,
    unison: [UnisonVoice; MAX_UNISON_VOICES],
    phases: [Phase; MAX_UNISON_VOICES],
    output: Buffer,
}

impl Default for VoiceState {
    fn default() -> Self {
        Self {
            octave: 0.0,
            triggered: false,
            phases: Default::default(),
            unison_gain: 1.0,
            unison: Default::default(),
            output: zero_buffer(),
        }
    }
}

struct VoiceBuffers {
    wave_buffers_swapped: bool,
    wave_buffers: (WaveformBuffer, WaveformBuffer),
}

impl Default for VoiceBuffers {
    fn default() -> Self {
        Self {
            wave_buffers_swapped: false,
            wave_buffers: (make_zero_wave_buffer(), make_zero_wave_buffer()),
        }
    }
}

#[derive(Default)]
struct Voice {
    state: VoiceState,
    buffers: VoiceBuffers,
}

#[derive(Default)]
struct Channel {
    params: ChannelParams,
    voices: [Voice; MAX_VOICES],
}

struct OscState {
    inverse_fft: Arc<dyn ComplexToReal<Sample>>,
    tmp_spectral: DftBuffer,
    scratch: DftBuffer,
    gain: Buffer,
    pitch: Buffer,
    phase_shift: Buffer,
    frequency_shift: Buffer,
}

impl Default for OscState {
    fn default() -> Self {
        Self {
            inverse_fft: RealFftPlanner::<Sample>::new().plan_fft_inverse(WAVEFORM_SIZE),
            tmp_spectral: zero_dft_buffer(),
            scratch: zero_dft_buffer(),
            gain: zero_buffer(),
            pitch: zero_buffer(),
            phase_shift: zero_buffer(),
            frequency_shift: zero_buffer(),
        }
    }
}

pub struct Oscillator {
    id: ModuleId,
    label: String,
    config: ModuleConfigBox<OscillatorConfig>,
    params: Params,
    osc_state: OscState,
    channels: [Channel; NUM_CHANNELS],
}

impl Oscillator {
    pub fn new(id: ModuleId, config: ModuleConfigBox<OscillatorConfig>) -> Self {
        let mut osc = Self {
            id,
            label: format!("Oscillator {id}"),
            config,
            params: Params::default(),
            osc_state: OscState::default(),
            channels: Default::default(),
        };

        load_module_config!(osc);
        osc
    }

    gen_downcast_methods!();

    pub fn get_ui(&self) -> OscillatorUIData {
        OscillatorUIData {
            label: self.label.clone(),
            gain: get_stereo_param!(self, gain),
            pitch_shift: get_stereo_param!(self, pitch_shift),
            detune: get_stereo_param!(self, detune),
            phase_shift: get_stereo_param!(self, phase_shift),
            frequency_shift: get_stereo_param!(self, frequency_shift),
            unison: self.params.unison,
            reset_phase: self.params.reset_phase,
            unison_phases: std::array::from_fn(|i| {
                StereoSample::from_iter(
                    self.channels
                        .iter()
                        .map(|channel| channel.params.unison_phases[i]),
                )
            }),
            unison_gains: std::array::from_fn(|i| {
                StereoSample::from_iter(
                    self.channels
                        .iter()
                        .map(|channel| channel.params.unison_gains[i]),
                )
            }),
        }
    }

    set_mono_param!(
        set_unison,
        unison,
        usize,
        unison.clamp(1, MAX_UNISON_VOICES)
    );
    set_mono_param!(set_reset_phase, reset_phase, bool);

    set_stereo_param!(set_gain, gain);
    set_stereo_param!(
        set_pitch_shift,
        pitch_shift,
        pitch_shift.clamp(st_to_octave(-60.0), st_to_octave(60.0))
    );
    set_stereo_param!(set_detune, detune, detune.clamp(0.0, st_to_octave(1.0)));
    set_stereo_param!(set_phase_shift, phase_shift, phase_shift.clamp(-1.0, 1.0));
    set_stereo_param!(set_frequency_shift, frequency_shift);

    pub fn set_unison_phase(&mut self, voice_idx: usize, phase: StereoSample) {
        for (channel, phase) in self.channels.iter_mut().zip(phase.iter()) {
            channel.params.unison_phases[voice_idx] = *phase;
        }

        let mut cfg = self.config.lock();

        for (channel_cfg, channel) in cfg.channels.iter_mut().zip(self.channels.iter()) {
            channel_cfg.unison_phases[voice_idx] = channel.params.unison_phases[voice_idx];
        }
    }

    pub fn set_unison_gain(&mut self, voice_idx: usize, gain: StereoSample) {
        for (channel, gain) in self.channels.iter_mut().zip(gain.iter()) {
            channel.params.unison_gains[voice_idx] = *gain;
        }

        let mut cfg = self.config.lock();

        for (channel_cfg, channel) in cfg.channels.iter_mut().zip(self.channels.iter()) {
            channel_cfg.unison_gains[voice_idx] = channel.params.unison_gains[voice_idx];
        }
    }

    pub fn apply_unison_level_shape(&mut self, center: StereoSample, level: StereoSample) {
        if self.params.unison < 2 {
            return;
        }

        for (center, edge_level, channel) in
            izip!(center.iter(), level.iter(), self.channels.iter_mut())
        {
            let center = center.clamp(0.0, 1.0);
            let step = ((self.params.unison - 1) as Sample).recip();

            for (idx, gain) in channel
                .params
                .unison_gains
                .iter_mut()
                .enumerate()
                .take(self.params.unison)
            {
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

        let mut cfg = self.config.lock();

        for (channel_cfg, channel) in cfg.channels.iter_mut().zip(self.channels.iter()) {
            channel_cfg.unison_gains = channel.params.unison_gains;
        }
    }

    #[inline(always)]
    fn get_wave_slice_mut(wave_buff: &mut WaveformBuffer) -> &mut [Sample] {
        &mut wave_buff[WAVEFORM_PAD_LEFT..(WAVEFORM_BUFFER_SIZE - WAVEFORM_PAD_RIGHT)]
    }

    #[inline(always)]
    fn load_segment(wave_buffer: &WaveformBuffer, idx: usize) -> f32x4 {
        let s = &wave_buffer[idx..idx + 4];

        f32x4::new([s[0], s[1], s[2], s[3]])
    }

    #[inline(always)]
    fn get_interpolated_sample(
        wave_from: &WaveformBuffer,
        wave_to: &WaveformBuffer,
        buff_t: Sample,
        idx: usize,
        t: Sample,
    ) -> Sample {
        const B0: f32x4 = f32x4::new([-1.0 / 2.0, 3.0 / 2.0, -3.0 / 2.0, 1.0 / 2.0]);
        const B1: f32x4 = f32x4::new([1.0, -5.0 / 2.0, 4.0 / 2.0, -1.0 / 2.0]);
        const B2: f32x4 = f32x4::new([-1.0 / 2.0, 0.0 / 2.0, 1.0 / 2.0, 0.0 / 2.0]);
        const B3: f32x4 = f32x4::new([0.0 / 2.0, 1.0, 0.0 / 2.0, 0.0 / 2.0]);

        let c_from = Self::load_segment(wave_from, idx);
        let c_to = Self::load_segment(wave_to, idx);

        let c = (c_to - c_from).mul_add(f32x4::splat(buff_t), c_from);
        let t = f32x4::splat(t);

        (c * B0)
            .mul_add(t, c * B1)
            .mul_add(t, c * B2)
            .mul_add(t, c * B3)
            .reduce_add()
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

    fn build_waveforms<'a>(
        voice_buffers: &'a mut VoiceBuffers,
        mono_voice_buffers: Option<&'a VoiceBuffers>,
        osc_state: &mut OscState,
        triggered: bool,
        router: &VoiceRouter,
    ) -> (&'a WaveformBuffer, &'a WaveformBuffer) {
        let voice_buffers = if let Some(mono_voice_buffers) = mono_voice_buffers {
            mono_voice_buffers
        } else {
            if triggered {
                let spectrum_from = router.spectral(Input::Spectrum, false);

                Self::build_wave(
                    osc_state.inverse_fft.as_ref(),
                    octave_to_freq(osc_state.pitch[0]) + osc_state.frequency_shift[0],
                    router.sample_rate,
                    spectrum_from,
                    &mut osc_state.tmp_spectral,
                    &mut osc_state.scratch,
                    &mut voice_buffers.wave_buffers.0,
                );

                voice_buffers.wave_buffers_swapped = false;
            }

            let spectrum = router.spectral(Input::Spectrum, true);

            let wave_to = if voice_buffers.wave_buffers_swapped {
                &mut voice_buffers.wave_buffers.0
            } else {
                &mut voice_buffers.wave_buffers.1
            };

            Self::build_wave(
                osc_state.inverse_fft.as_ref(),
                octave_to_freq(osc_state.pitch[router.samples - 1])
                    + osc_state.frequency_shift[router.samples - 1],
                router.sample_rate,
                spectrum,
                &mut osc_state.tmp_spectral,
                &mut osc_state.scratch,
                wave_to,
            );
            voice_buffers.wave_buffers_swapped = !voice_buffers.wave_buffers_swapped;

            voice_buffers
        };

        if voice_buffers.wave_buffers_swapped {
            (&voice_buffers.wave_buffers.0, &voice_buffers.wave_buffers.1)
        } else {
            (&voice_buffers.wave_buffers.1, &voice_buffers.wave_buffers.0)
        }
    }

    fn process_unison(
        unison: usize,
        channel: &ChannelParams,
        voice: &mut VoiceState,
        router: &VoiceRouter,
    ) {
        const MAX_DETUNE: Sample = 1.0;

        if unison < 2 {
            voice.unison[0] = UnisonVoice::default();
            voice.unison_gain = 1.0;
            return;
        }

        struct UnisonParam {
            rate: Sample,
        }

        fn calc_params(unison: usize, detune: Sample) -> impl Iterator<Item = UnisonParam> {
            let center = 0.5 * (unison - 1) as Sample;
            let center_recip = center.recip();

            (0..unison).map(move |idx| {
                let spread = (idx as Sample - center) * center_recip;

                UnisonParam {
                    rate: (spread * detune).exp2(),
                }
            })
        }

        if voice.triggered {
            let detune = (channel.detune + router.scalar(Input::Detune, false)).min(MAX_DETUNE);

            for (state, gain, phase, param) in izip!(
                &mut voice.unison,
                &channel.unison_gains,
                &channel.unison_phases,
                calc_params(unison, detune)
            ) {
                state.rate_from = param.rate;
                state.phase_shift = *phase;
                state.gain = *gain;
            }
        } else {
            for state in &mut voice.unison {
                state.rate_from = state.rate_to;
            }
        }

        let detune = (channel.detune + router.scalar(Input::Detune, true)).min(MAX_DETUNE);

        for (state, gain, phase, param) in izip!(
            &mut voice.unison,
            &channel.unison_gains,
            &channel.unison_phases,
            calc_params(unison, detune)
        ) {
            state.rate_to = param.rate;
            state.rate_diff = state.rate_to - state.rate_from;
            state.phase_shift = *phase;
            state.gain = *gain;
        }

        voice.unison_gain = channel
            .unison_gains
            .iter()
            .take(unison)
            .map(|gain| gain * gain)
            .sum::<Sample>()
            .sqrt()
            .max(1.0) //Don't amplify
            .recip();
    }

    fn process_voice(
        params: &Params,
        channel: &ChannelParams,
        osc_state: &mut OscState,
        voice: &mut Voice,
        mono_voice_buffers: Option<&VoiceBuffers>,
        router: VoiceRouter,
    ) {
        let samples = router.samples;

        router.fill_and_add_input(Input::Gain, channel.gain, &mut osc_state.gain);
        router.fill_and_add_input(
            Input::PitchShift,
            voice.state.octave + channel.pitch_shift,
            &mut osc_state.pitch,
        );
        router.fill_and_add_input(
            Input::PhaseShift,
            channel.phase_shift,
            &mut osc_state.phase_shift,
        );
        router.fill_and_add_input(
            Input::FrequencyShift,
            channel.frequency_shift,
            &mut osc_state.frequency_shift,
        );

        let (wave_from, wave_to) = Self::build_waveforms(
            &mut voice.buffers,
            mono_voice_buffers,
            osc_state,
            voice.state.triggered,
            &router,
        );

        Self::process_unison(params.unison, channel, &mut voice.state, &router);

        if voice.state.triggered {
            voice.state.triggered = false;
        }

        let voice = &mut voice.state;
        let freq_phase_mult = Phase::freq_phase_mult(router.sample_rate);
        let buff_t_mult = (samples as f32).recip();

        for (out, pitch, phase_shift, freq_shift, gain, sample_idx) in izip!(
            &mut voice.output,
            &osc_state.pitch,
            &osc_state.phase_shift,
            &osc_state.frequency_shift,
            &osc_state.gain,
            0..samples
        ) {
            let mut sample: Sample = 0.0;
            let buff_t = sample_idx as Sample * buff_t_mult;
            let phase_shift = Phase::from_normalized(*phase_shift);
            let pitch_phase_inc = octave_to_freq(*pitch) * freq_phase_mult;
            let freq_phase_inc = freq_shift * freq_phase_mult;

            for (phase, unison_voice) in voice
                .phases
                .iter_mut()
                .zip(voice.unison.iter())
                .take(params.unison)
            {
                let read_phase =
                    *phase + phase_shift + Phase::from_normalized(unison_voice.phase_shift);
                let idx = read_phase.wave_index::<WAVEFORM_BITS>();
                let t = read_phase.wave_index_fraction::<WAVEFORM_BITS>();

                sample += Self::get_interpolated_sample(wave_from, wave_to, buff_t, idx, t)
                    * unison_voice.gain;

                *phase += pitch_phase_inc
                    * unison_voice
                        .rate_diff
                        .mul_add(buff_t, unison_voice.rate_from)
                    + freq_phase_inc;
            }

            *out = sample * voice.unison_gain * gain;
        }
    }
}

impl SynthModule for Oscillator {
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
        ModuleType::Oscillator
    }

    fn inputs(&self) -> &'static [InputInfo] {
        static INPUTS: &[InputInfo] = &[
            InputInfo::spectral(Input::Spectrum),
            InputInfo::buffer(Input::Gain),
            InputInfo::buffer(Input::PitchShift),
            InputInfo::buffer(Input::PhaseShift),
            InputInfo::buffer(Input::FrequencyShift),
            InputInfo::buffer(Input::Detune),
        ];

        INPUTS
    }

    fn output(&self) -> DataType {
        DataType::Buffer
    }

    fn note_on(&mut self, params: &NoteOnParams) {
        for channel in self.channels.iter_mut() {
            let voice = &mut channel.voices[params.voice_idx];

            voice.state.octave = note_to_octave(params.note);
            voice.state.triggered = true;

            if params.reset || self.params.reset_phase {
                voice.state.phases = Default::default();
            }
        }
    }

    fn process(&mut self, params: &ProcessParams, router: &dyn Router) {
        for (channel_idx, channel) in self
            .channels
            .iter_mut()
            .enumerate()
            .take(params.spectrum_channels)
        {
            for voice_idx in params.active_voices {
                let router = VoiceRouter {
                    router,
                    module_id: self.id,
                    samples: params.samples,
                    sample_rate: params.sample_rate,
                    voice_idx: *voice_idx,
                    channel_idx,
                };

                Self::process_voice(
                    &self.params,
                    &channel.params,
                    &mut self.osc_state,
                    &mut channel.voices[*voice_idx],
                    None,
                    router,
                );
            }
        }

        if params.spectrum_channels > 0 && params.spectrum_channels < self.channels.len() {
            let (left, right) = self.channels.split_at_mut(params.spectrum_channels);
            let left = &left[0];

            for (idx, channel) in right.iter_mut().enumerate() {
                let channel_idx = idx + params.spectrum_channels;

                for voice_idx in params.active_voices {
                    let router = VoiceRouter {
                        router,
                        module_id: self.id,
                        samples: params.samples,
                        sample_rate: params.sample_rate,
                        voice_idx: *voice_idx,
                        channel_idx,
                    };

                    Self::process_voice(
                        &self.params,
                        &channel.params,
                        &mut self.osc_state,
                        &mut channel.voices[*voice_idx],
                        Some(&left.voices[*voice_idx].buffers),
                        router,
                    );
                }
            }
        }
    }

    fn get_buffer_output(&self, voice_idx: usize, channel: usize) -> &Buffer {
        &self.channels[channel].voices[voice_idx].state.output
    }
}
