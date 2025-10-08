use std::{f32, sync::Arc};

use itertools::izip;
use nih_plug::util::f32_midi_note_to_freq;
use rand::Rng;
use rand_pcg::Pcg32;
use realfft::{ComplexToReal, RealFftPlanner};

use crate::synth_engine::{
    buffer::{
        BUFFER_SIZE, Buffer, ComplexSample, ONES_BUFFER, Phase, Sample, SpectralBuffer,
        WAVEFORM_BITS, WAVEFORM_SIZE, WaveformBuffer, ZEROES_BUFFER, get_interpolated_sample,
        get_wave_slice_mut, make_zero_buffer, make_zero_spectral_buffer, make_zero_wave_buffer,
        wrap_wave_buffer,
    },
    routing::{MAX_VOICES, ModuleId, ModuleInput, Router},
    synth_module::{NoteOnParams, ProcessParams, SynthModule},
};

const FULL_PHASE: f32 = ((u32::MAX as u64) + 1) as f32;
const INTERMEDIATE_BITS: usize = 32 - WAVEFORM_BITS;
const INTERMEDIATE_MASK: u32 = (1 << INTERMEDIATE_BITS) - 1;
const INTERMEDIATE_MULT: f32 = ((1 << INTERMEDIATE_BITS) as f32).recip();
const PITCH_MOD_RANGE: f32 = 48.0;
const DETUNE_MOD_RANGE: f32 = 1.0;
const MAX_UNISON_VOICES: usize = 16;

struct OscillatorVoice {
    note: f32,
    wave_buffers_initialized: bool,
    wave_buffers_swapped: bool,
    phases: [Phase; MAX_UNISON_VOICES],
    output: Buffer,
    wave_buffers: (WaveformBuffer, WaveformBuffer),
}

impl Default for OscillatorVoice {
    fn default() -> Self {
        Self {
            note: 0.0,
            wave_buffers_initialized: false,
            wave_buffers_swapped: false,
            phases: Default::default(),
            output: make_zero_buffer(),
            wave_buffers: (make_zero_wave_buffer(), make_zero_wave_buffer()),
        }
    }
}

pub struct OscillatorModule {
    module_id: ModuleId,
    level: f32,
    pitch_shift: f32,
    unison: usize,
    detune: f32,
    random: Pcg32,
    inverse_fft: Arc<dyn ComplexToReal<Sample>>,
    tmp_spectral_buff: SpectralBuffer,
    scratch_buff: SpectralBuffer,
    level_mod_input: Buffer,
    pitch_shift_input: Buffer,
    detune_mod_input: Buffer,
    voices: [OscillatorVoice; MAX_VOICES],
}

impl OscillatorModule {
    pub fn new() -> Self {
        Self {
            module_id: 0,
            level: 0.5,
            pitch_shift: 0.0,
            unison: 1,
            detune: 0.3,
            random: Pcg32::new(3537, 9573),
            inverse_fft: RealFftPlanner::<Sample>::new().plan_fft_inverse(WAVEFORM_SIZE),
            tmp_spectral_buff: make_zero_spectral_buffer(),
            scratch_buff: make_zero_spectral_buffer(),
            level_mod_input: make_zero_buffer(),
            pitch_shift_input: make_zero_buffer(),
            detune_mod_input: make_zero_buffer(),
            voices: Default::default(),
        }
    }

    pub(super) fn set_id(&mut self, module_id: ModuleId) {
        self.module_id = module_id;
    }

    pub fn set_unison(&mut self, unison: usize) -> &mut Self {
        self.unison = unison.clamp(1, MAX_UNISON_VOICES);
        self
    }

    pub fn set_detune(&mut self, detune: f32) -> &mut Self {
        self.detune = detune;
        self
    }

    #[inline(always)]
    fn calc_frequency(note: f32, pitch_shift: f32, pitch_shift_mod: f32) -> f32 {
        f32_midi_note_to_freq(note + pitch_shift + pitch_shift_mod * PITCH_MOD_RANGE)
    }

    fn build_wave(
        inverse_fft: &dyn ComplexToReal<Sample>,
        frequency: f32,
        sample_rate: f32,
        spectral_buff: &SpectralBuffer,
        tmp_spectral_buff: &mut SpectralBuffer,
        scratch_buff: &mut SpectralBuffer,
        out_wave_buff: &mut WaveformBuffer,
    ) {
        let cutoff_index =
            ((0.5 * sample_rate / frequency).floor() as usize + 1).min(spectral_buff.len() - 1);

        *tmp_spectral_buff = *spectral_buff;
        tmp_spectral_buff[cutoff_index..].fill(ComplexSample::ZERO);

        inverse_fft
            .process_with_scratch(
                tmp_spectral_buff,
                get_wave_slice_mut(out_wave_buff),
                scratch_buff,
            )
            .unwrap();
        wrap_wave_buffer(out_wave_buff);
    }

    fn prepare_wave_buffers(
        ifft: &dyn ComplexToReal<Sample>,
        frequency: f32,
        sample_rate: f32,
        (spectral_buff_from, spectral_buff_to): (&SpectralBuffer, &SpectralBuffer),
        tmp_spectral_buff: &mut SpectralBuffer,
        scratch_buff: &mut SpectralBuffer,
        voice: &mut OscillatorVoice,
    ) {
        if voice.wave_buffers_initialized {
            let next_wave_buff = if voice.wave_buffers_swapped {
                &mut voice.wave_buffers.1
            } else {
                &mut voice.wave_buffers.0
            };

            Self::build_wave(
                ifft,
                frequency,
                sample_rate,
                spectral_buff_to,
                tmp_spectral_buff,
                scratch_buff,
                next_wave_buff,
            );

            voice.wave_buffers_swapped = !voice.wave_buffers_swapped;
        } else {
            Self::build_wave(
                ifft,
                frequency,
                sample_rate,
                spectral_buff_from,
                tmp_spectral_buff,
                scratch_buff,
                &mut voice.wave_buffers.0,
            );

            Self::build_wave(
                ifft,
                frequency,
                sample_rate,
                spectral_buff_to,
                tmp_spectral_buff,
                scratch_buff,
                &mut voice.wave_buffers.1,
            );

            voice.wave_buffers_swapped = false;
            voice.wave_buffers_initialized = true;
        }
    }

    #[inline(always)]
    fn process_sample(
        note: f32,
        buff_t: f32,
        wave_from: &WaveformBuffer,
        wave_to: &WaveformBuffer,
        freq_phase_mult: f32,
        phase: &mut Phase,
    ) -> Sample {
        let frequency = f32_midi_note_to_freq(note);
        let idx = (*phase >> INTERMEDIATE_BITS) as usize;
        let t = (*phase & INTERMEDIATE_MASK) as f32 * INTERMEDIATE_MULT;
        let sample_from = get_interpolated_sample(wave_from, idx, t);
        let sample_to = get_interpolated_sample(wave_to, idx, t);

        *phase = phase.wrapping_add((frequency * freq_phase_mult) as u32);
        sample_from * (1.0 - buff_t) + sample_to * buff_t
    }

    fn process_voice(&mut self, params: &ProcessParams, router: &dyn Router, voice_idx: usize) {
        let sample_rate = params.sample_rate;
        let voice = &mut self.voices[voice_idx];
        let level_mod = router
            .get_input(
                ModuleInput::OscillatorLevel(self.module_id),
                voice_idx,
                &mut self.level_mod_input,
            )
            .unwrap_or(&ONES_BUFFER);
        let pitch_shift_mod = router
            .get_input(
                ModuleInput::OscillatorPitchShift(self.module_id),
                voice_idx,
                &mut self.pitch_shift_input,
            )
            .unwrap_or(&ZEROES_BUFFER);

        Self::prepare_wave_buffers(
            self.inverse_fft.as_ref(),
            Self::calc_frequency(voice.note, self.pitch_shift, pitch_shift_mod[0]),
            sample_rate,
            router.get_spectral_input(voice_idx).unwrap(),
            &mut self.tmp_spectral_buff,
            &mut self.scratch_buff,
            voice,
        );

        let (wave_from, wave_to) = if voice.wave_buffers_swapped {
            (&voice.wave_buffers.1, &voice.wave_buffers.0)
        } else {
            (&voice.wave_buffers.0, &voice.wave_buffers.1)
        };

        let freq_phase_mult = FULL_PHASE / sample_rate;
        let buff_t_mult = (BUFFER_SIZE as f32).recip();
        let fixed_note = voice.note + self.pitch_shift;

        if self.unison > 1 {
            let detune_mod = router
                .get_input(
                    ModuleInput::OscillatorDetune(self.module_id),
                    voice_idx,
                    &mut self.detune_mod_input,
                )
                .unwrap_or(&ZEROES_BUFFER);

            let unison_mult = ((self.unison - 1) as Sample).recip();
            let unison_scale = 1.0 / (self.unison as Sample).sqrt();

            for (out, level_mod, pitch_shift_mod, detune_mod, sample_idx) in izip!(
                &mut voice.output,
                level_mod,
                pitch_shift_mod,
                detune_mod,
                0..params.samples
            ) {
                let mut sample: Sample = 0.0;
                let buff_t = sample_idx as f32 * buff_t_mult;
                let note = fixed_note + *pitch_shift_mod * PITCH_MOD_RANGE;
                let detune = self.detune + *detune_mod * DETUNE_MOD_RANGE;
                let unison_pitch_step = detune * unison_mult;
                let unison_pitch_from = -0.5 * detune;

                for unison_idx in 0..self.unison {
                    let unison_idx_float = unison_idx as f32;
                    let unison_pitch_shift =
                        unison_pitch_from + unison_pitch_step * unison_idx_float;
                    let phase = &mut voice.phases[unison_idx];

                    sample += Self::process_sample(
                        note + unison_pitch_shift,
                        buff_t,
                        wave_from,
                        wave_to,
                        freq_phase_mult,
                        phase,
                    );
                }

                *out = sample * unison_scale * self.level * level_mod;
            }
        } else {
            let phase = &mut voice.phases[0];

            for (out, level_mod, pitch_shift_mod, sample_idx) in izip!(
                &mut voice.output,
                level_mod,
                pitch_shift_mod,
                0..params.samples
            ) {
                *out = Self::process_sample(
                    fixed_note + *pitch_shift_mod * PITCH_MOD_RANGE,
                    sample_idx as f32 * buff_t_mult,
                    wave_from,
                    wave_to,
                    freq_phase_mult,
                    phase,
                ) * self.level
                    * level_mod;
            }
        }
    }
}

impl SynthModule for OscillatorModule {
    fn get_id(&self) -> ModuleId {
        self.module_id
    }

    fn get_output(&self, voice_idx: usize) -> &Buffer {
        &self.voices[voice_idx].output
    }

    fn note_on(&mut self, params: &NoteOnParams) {
        let voice = &mut self.voices[params.voice_idx];

        voice.note = params.note;
        voice.wave_buffers_initialized = false;
        voice.wave_buffers_swapped = false;

        if !params.same_note_retrigger {
            self.random.fill(&mut voice.phases[..self.unison]);
        }
    }

    fn note_off(&mut self, _: &super::synth_module::NoteOffParams) {}

    fn process(&mut self, params: &ProcessParams, router: &dyn Router) {
        for voice_idx in params.active_voices {
            self.process_voice(params, router, *voice_idx);
        }
    }
}
