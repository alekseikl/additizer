use std::{any::Any, f32, sync::Arc};

use itertools::izip;
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
    initial_phases: [Sample; MAX_UNISON_VOICES],
}

impl Default for ChannelParams {
    fn default() -> Self {
        Self {
            gain: 1.0,
            pitch_shift: 0.0,
            detune: st_to_octave(0.2),
            phase_shift: 0.0,
            frequency_shift: 0.0,
            initial_phases: INITIAL_PHASES,
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
    pub initial_phases: [StereoSample; MAX_UNISON_VOICES],
}

struct Voice {
    octave: Sample,
    wave_buffers_swapped: bool,
    triggered: bool,
    phases: [Phase; MAX_UNISON_VOICES],
    output: Buffer,
    wave_buffers: (WaveformBuffer, WaveformBuffer),
}

impl Default for Voice {
    fn default() -> Self {
        Self {
            octave: 0.0,
            wave_buffers_swapped: false,
            triggered: false,
            phases: Default::default(),
            output: zero_buffer(),
            wave_buffers: (make_zero_wave_buffer(), make_zero_wave_buffer()),
        }
    }
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
            initial_phases: std::array::from_fn(|i| {
                StereoSample::from_iter(
                    self.channels
                        .iter()
                        .map(|channel| channel.params.initial_phases[i]),
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

    pub fn set_initial_phase(&mut self, voice_idx: usize, phase: StereoSample) {
        for (channel, phase) in self.channels.iter_mut().zip(phase.iter()) {
            channel.params.initial_phases[voice_idx] = *phase;
        }

        let mut cfg = self.config.lock();

        for (channel_cfg, channel) in cfg.channels.iter_mut().zip(self.channels.iter()) {
            channel_cfg.initial_phases[voice_idx] = channel.params.initial_phases[voice_idx];
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

    #[inline(always)]
    #[allow(clippy::too_many_arguments)]
    // #[unsafe(no_mangle)]
    fn process_sample(
        octave: Sample,
        phase_shift: Phase,
        freq_shift: Sample,
        buff_t: Sample,
        wave_from: &WaveformBuffer,
        wave_to: &WaveformBuffer,
        freq_phase_mult: Sample,
        phase: &mut Phase,
    ) -> Sample {
        let shifted_phase = *phase + phase_shift;
        let idx = shifted_phase.wave_index::<WAVEFORM_BITS>();
        let t = shifted_phase.wave_index_fraction::<WAVEFORM_BITS>();
        let result = Self::get_interpolated_sample(wave_from, wave_to, buff_t, idx, t);

        *phase += (octave_to_freq(octave) + freq_shift) * freq_phase_mult;

        result
    }

    fn process_voice(
        params: &Params,
        channel: &ChannelParams,
        buffers: &mut Buffers,
        voice: &mut Voice,
        sample_rate: Sample,
        router: VoiceRouter,
    ) {
        let gain_mod = router.buffer(Input::Gain, &mut buffers.gain_mod);
        let pitch_shift_mod = router.buffer(Input::PitchShift, &mut buffers.pitch_shift_mod);
        let phase_shift_mod = router.buffer(Input::PhaseShift, &mut buffers.phase_shift_mod);
        let freq_shift_mod = router.buffer(Input::FrequencyShift, &mut buffers.frequency_shift_mod);

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
                &mut voice.wave_buffers.0,
            );

            voice.wave_buffers_swapped = false;
            voice.triggered = false;
        }

        let spectrum = router.spectral(Input::Spectrum, true);

        let (wave_from, wave_to) = if voice.wave_buffers_swapped {
            (&voice.wave_buffers.1, &mut voice.wave_buffers.0)
        } else {
            (&voice.wave_buffers.0, &mut voice.wave_buffers.1)
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
        voice.wave_buffers_swapped = !voice.wave_buffers_swapped;

        let freq_phase_mult = Phase::freq_phase_mult(sample_rate);
        let buff_t_mult = (router.samples as f32).recip();
        let fixed_octave = voice.octave + channel.pitch_shift;

        if params.unison > 1 {
            let detune_mod = router.buffer(Input::Detune, &mut buffers.detune_mod);

            let unison_mult = ((params.unison - 1) as Sample).recip();
            let unison_scale = 1.0 / (params.unison as Sample).sqrt();

            for (
                out,
                gain_mod,
                pitch_shift_mod,
                phase_shift_mod,
                freq_shift_mod,
                detune_mod,
                sample_idx,
            ) in izip!(
                &mut voice.output,
                gain_mod,
                pitch_shift_mod,
                phase_shift_mod,
                freq_shift_mod,
                detune_mod,
                0..router.samples
            ) {
                let mut sample: Sample = 0.0;
                let buff_t = sample_idx as Sample * buff_t_mult;
                let octave = fixed_octave + *pitch_shift_mod;
                let detune = channel.detune + *detune_mod;
                let unison_pitch_step = detune * unison_mult;
                let unison_pitch_from = -0.5 * detune;
                let phase_shift = Phase::from_normalized(channel.phase_shift + *phase_shift_mod);
                let freq_shift = channel.frequency_shift + *freq_shift_mod;

                for unison_idx in 0..params.unison {
                    let unison_idx_float = unison_idx as Sample;
                    let unison_pitch_shift =
                        unison_pitch_from + unison_pitch_step * unison_idx_float;
                    let phase = &mut voice.phases[unison_idx];

                    sample += Self::process_sample(
                        octave + unison_pitch_shift,
                        phase_shift,
                        freq_shift,
                        buff_t,
                        wave_from,
                        wave_to,
                        freq_phase_mult,
                        phase,
                    );
                }

                *out = sample * unison_scale * (channel.gain + gain_mod);
            }
        } else {
            let phase = &mut voice.phases[0];

            for (out, gain_mod, pitch_shift_mod, phase_shift_mod, freq_shift_mod, sample_idx) in izip!(
                &mut voice.output,
                gain_mod,
                pitch_shift_mod,
                phase_shift_mod,
                freq_shift_mod,
                0..router.samples
            ) {
                *out = Self::process_sample(
                    fixed_octave + *pitch_shift_mod,
                    Phase::from_normalized(channel.phase_shift + *phase_shift_mod),
                    channel.frequency_shift + *freq_shift_mod,
                    sample_idx as Sample * buff_t_mult,
                    wave_from,
                    wave_to,
                    freq_phase_mult,
                    phase,
                ) * (channel.gain + gain_mod);
            }
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

            voice.octave = note_to_octave(params.note);
            voice.triggered = true;

            if params.reset || self.params.reset_phase {
                for (phase, initial_phase) in
                    voice.phases.iter_mut().zip(channel.params.initial_phases)
                {
                    *phase = Phase::from_normalized(initial_phase);
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
        &self.channels[channel].voices[voice_idx].output
    }
}
