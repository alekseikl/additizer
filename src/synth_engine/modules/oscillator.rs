use std::{any::Any, f32, sync::Arc};

use itertools::izip;
use nih_plug::util::db_to_gain;
use realfft::{ComplexToReal, RealFftPlanner};
use serde::{Deserialize, Serialize};
use smallvec::{SmallVec, smallvec};
use wide::{f32x4, u32x4};

use crate::{
    synth_engine::{
        StereoSample,
        buffer::{BUFFER_SIZE, Buffer, SPECTRUM_BITS, SpectralBuffer, zero_buffer},
        phase::Phase,
        routing::{DataType, Input, MAX_VOICES, ModuleId, ModuleType, NUM_CHANNELS, Router},
        synth_module::{
            InputInfo, ModuleConfigBox, NoteOnParams, ProcessParams, SynthModule, VoiceRouter,
        },
        types::{ComplexSample, IntoSimdIter, Sample},
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

const VOICE_BLOCK_SIZE: usize = 4;
const MAX_UNISON_BLOCKS: usize = MAX_UNISON_VOICES.div_ceil(VOICE_BLOCK_SIZE);

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

struct VoiceState {
    octave: Sample,
    triggered: bool,
    phase_blocks: [u32x4; MAX_UNISON_BLOCKS],
    output: Buffer,
}

impl Default for VoiceState {
    fn default() -> Self {
        Self {
            octave: 0.0,
            triggered: false,
            phase_blocks: Default::default(),
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

struct Buffers {
    inverse_fft: Arc<dyn ComplexToReal<Sample>>,
    tmp_spectral: DftBuffer,
    scratch: DftBuffer,
    gain_mod: Buffer,
    pitch_shift_mod: Buffer,
    phase_shift_mod: Buffer,
    frequency_shift_mod: Buffer,
    detune_mod: Buffer,
    blocks_buff: [f32x4; BUFFER_SIZE],
}

impl Default for Buffers {
    fn default() -> Self {
        Self {
            inverse_fft: RealFftPlanner::<Sample>::new().plan_fft_inverse(WAVEFORM_SIZE),
            tmp_spectral: zero_dft_buffer(),
            scratch: zero_dft_buffer(),
            gain_mod: zero_buffer(),
            pitch_shift_mod: zero_buffer(),
            phase_shift_mod: zero_buffer(),
            frequency_shift_mod: zero_buffer(),
            detune_mod: zero_buffer(),
            blocks_buff: [f32x4::ZERO; BUFFER_SIZE],
        }
    }
}

pub struct Oscillator {
    id: ModuleId,
    label: String,
    config: ModuleConfigBox<OscillatorConfig>,
    params: Params,
    buffers: Buffers,
    channels: [Channel; NUM_CHANNELS],
}

impl Oscillator {
    pub fn new(id: ModuleId, config: ModuleConfigBox<OscillatorConfig>) -> Self {
        let mut osc = Self {
            id,
            label: format!("Oscillator {id}"),
            config,
            params: Params::default(),
            buffers: Buffers::default(),
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
    fn load_matrix(wave_buffer: &WaveformBuffer, indices: u32x4) -> [f32x4; 4] {
        indices.as_array().map(|idx| {
            let idx = idx as usize;

            f32x4::new([
                wave_buffer[idx],
                wave_buffer[idx + 1],
                wave_buffer[idx + 2],
                wave_buffer[idx + 3],
            ])
        })
    }

    #[inline(always)]
    fn interpolation_matrix(t: f32x4) -> [f32x4; 4] {
        let half_t = t * 0.5;
        let half_t2 = t * half_t;
        let half_t3 = half_t2 * t;
        let half_three_t3 = half_t3 * 3.0;

        [
            half_t2 * 2.0 - half_t3 - half_t,
            half_t2.mul_neg_add(5.0.into(), half_three_t3) + 1.0,
            half_t2.mul_add(4.0.into(), half_t) - half_three_t3,
            half_t3 - half_t2,
        ]
    }

    // #[inline(always)]
    #[unsafe(no_mangle)]
    fn get_interpolated_sample_wide(
        wave_from: &WaveformBuffer,
        wave_to: &WaveformBuffer,
        buff_t: f32x4,
        indices: u32x4,
        t: f32x4,
    ) -> f32x4 {
        let values = {
            let values_from = Self::load_matrix(wave_from, indices);
            let values_to = Self::load_matrix(wave_to, indices);

            f32x4::transpose([
                (values_to[0] - values_from[0]).mul_add(buff_t, values_from[0]),
                (values_to[1] - values_from[1]).mul_add(buff_t, values_from[1]),
                (values_to[2] - values_from[2]).mul_add(buff_t, values_from[2]),
                (values_to[3] - values_from[3]).mul_add(buff_t, values_from[3]),
            ])
        };

        let interpolation = Self::interpolation_matrix(t);
        let row01 = interpolation[1].mul_add(values[1], interpolation[0] * values[0]);
        let row012 = interpolation[2].mul_add(values[2], row01);

        interpolation[3].mul_add(values[3], row012)
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

    fn process_voice(
        params: &Params,
        channel: &ChannelParams,
        buffers: &mut Buffers,
        voice: &mut Voice,
        sample_rate: Sample,
        router: VoiceRouter,
    ) {
        let samples = router.samples;
        let voice_buffers = &mut voice.buffers;
        let voice = &mut voice.state;

        let gain_mod = router.buffer(Input::Gain, &mut buffers.gain_mod);
        let pitch_shift_mod = router.buffer(Input::PitchShift, &mut buffers.pitch_shift_mod);
        let phase_shift_mod = router.buffer(Input::PhaseShift, &mut buffers.phase_shift_mod);
        let freq_shift_mod = router.buffer(Input::FrequencyShift, &mut buffers.frequency_shift_mod);
        let detune_mod = router.buffer(Input::Detune, &mut buffers.detune_mod);

        let wave_octave_fixed = voice.octave + channel.pitch_shift;

        if voice.triggered {
            let spectrum_from = router.spectral(Input::Spectrum, false);

            Self::build_wave(
                buffers.inverse_fft.as_ref(),
                octave_to_freq(wave_octave_fixed + pitch_shift_mod[0])
                    + channel.frequency_shift
                    + freq_shift_mod[0],
                sample_rate,
                spectrum_from,
                &mut buffers.tmp_spectral,
                &mut buffers.scratch,
                &mut voice_buffers.wave_buffers.0,
            );

            voice_buffers.wave_buffers_swapped = false;
            voice.triggered = false;
        }

        let spectrum = router.spectral(Input::Spectrum, true);

        let (wave_from, wave_to) = if voice_buffers.wave_buffers_swapped {
            (
                &voice_buffers.wave_buffers.1,
                &mut voice_buffers.wave_buffers.0,
            )
        } else {
            (
                &voice_buffers.wave_buffers.0,
                &mut voice_buffers.wave_buffers.1,
            )
        };

        Self::build_wave(
            buffers.inverse_fft.as_ref(),
            octave_to_freq(wave_octave_fixed + pitch_shift_mod[router.samples - 1])
                + channel.frequency_shift
                + freq_shift_mod[router.samples - 1],
            sample_rate,
            spectrum,
            &mut buffers.tmp_spectral,
            &mut buffers.scratch,
            wave_to,
        );
        voice_buffers.wave_buffers_swapped = !voice_buffers.wave_buffers_swapped;

        struct UnisonBlock {
            pitch_spread: f32x4,
            gain: f32x4,
        }

        let (unison_blocks, unison_scale): (SmallVec<[UnisonBlock; MAX_UNISON_BLOCKS]>, Sample) =
            if params.unison > 1 {
                let center = 0.5 * (params.unison - 1) as Sample;
                let center_recip = center.recip();

                let pitch_spread = (0..params.unison)
                    .map(move |idx| (idx as Sample - center) * center_recip)
                    .simd();

                let gain = channel
                    .unison_gains
                    .iter()
                    .take(params.unison)
                    .copied()
                    .simd();

                (
                    pitch_spread
                        .zip(gain)
                        .map(|(pitch_spread, gain)| UnisonBlock { pitch_spread, gain })
                        .collect(),
                    1.0 / (params.unison as Sample).sqrt(),
                )
            } else {
                (
                    smallvec![UnisonBlock {
                        pitch_spread: f32x4::ZERO,
                        gain: f32x4::new([1.0, 0.0, 0.0, 0.0]),
                    }],
                    1.0,
                )
            };

        let freq_phase_mult = Phase::freq_phase_mult(sample_rate);
        let buff_t_mult = (samples as f32).recip();
        let fixed_pitch = voice.octave + channel.pitch_shift;

        const INTERMEDIATE_BITS: u32 = 32 - WAVEFORM_BITS as u32;
        const INTERMEDIATE_MASK: u32x4 = u32x4::splat((1 << INTERMEDIATE_BITS) - 1);
        const INTERMEDIATE_MULT: f32x4 = f32x4::splat(((1 << INTERMEDIATE_BITS) as f32).recip());

        buffers.blocks_buff[..samples].fill(f32x4::ZERO);

        for (out, pitch_shift_mod, phase_shift_mod, freq_shift_mod, detune_mod, sample_idx) in izip!(
            buffers.blocks_buff.iter_mut(),
            pitch_shift_mod,
            phase_shift_mod,
            freq_shift_mod,
            detune_mod,
            0..samples
        ) {
            let buff_t = f32x4::splat(sample_idx as Sample * buff_t_mult);
            let pitch = fixed_pitch + *pitch_shift_mod;
            let detune = 0.5 * (channel.detune + *detune_mod);
            let phase_shift = Phase::from_normalized(channel.phase_shift + *phase_shift_mod);
            let freq_shift = channel.frequency_shift + *freq_shift_mod;

            for (phase, block) in voice.phase_blocks.iter_mut().zip(unison_blocks.iter()) {
                let shifted_phase = *phase + phase_shift.value();
                let indices = shifted_phase >> INTERMEDIATE_BITS;
                let t = f32x4::from_i32x4(bytemuck::cast(shifted_phase & INTERMEDIATE_MASK))
                    * INTERMEDIATE_MULT;

                *out += Self::get_interpolated_sample_wide(wave_from, wave_to, buff_t, indices, t)
                    * block.gain;

                let block_pitch = block.pitch_spread * detune + pitch;
                let phase_inc_float =
                    (((block_pitch * f32x4::LN_2).exp() * 440.0) + freq_shift) * freq_phase_mult;

                *phase += u32x4::new(phase_inc_float.to_array().map(|phase| phase as i64 as u32));
            }
        }

        for (out, block, gain_mod) in izip!(voice.output.iter_mut(), &buffers.blocks_buff, gain_mod)
        {
            *out = block.reduce_add() * unison_scale * (channel.gain + gain_mod);
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
                for (phase, initial_phase) in voice
                    .state
                    .phase_blocks
                    .iter_mut()
                    .zip(channel.params.unison_phases.iter().copied().simd())
                {
                    *phase = u32x4::new(
                        initial_phase
                            .to_array()
                            .map(|phase| Phase::from_normalized(phase).value()),
                    )
                }
            }
        }
    }

    fn process(&mut self, params: &ProcessParams, router: &dyn Router) {
        for (channel_idx, channel) in self.channels.iter_mut().enumerate() {
            for voice_idx in params.active_voices {
                let router = VoiceRouter {
                    router,
                    module_id: self.id,
                    samples: params.samples,
                    voice_idx: *voice_idx,
                    channel_idx,
                };

                Self::process_voice(
                    &self.params,
                    &channel.params,
                    &mut self.buffers,
                    &mut channel.voices[*voice_idx],
                    params.sample_rate,
                    router,
                );
            }
        }
    }

    fn get_buffer_output(&self, voice_idx: usize, channel: usize) -> &Buffer {
        &self.channels[channel].voices[voice_idx].state.output
    }
}
