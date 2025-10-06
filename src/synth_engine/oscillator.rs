use std::{f32, sync::Arc};

use itertools::izip;
use nih_plug::util::f32_midi_note_to_freq;
use realfft::{ComplexToReal, RealFftPlanner};
use uniform_cubic_splines::{CatmullRom, spline};

use crate::synth_engine::{
    buffer::{
        Buffer, ComplexSample, ONES_BUFFER, Sample, SpectralBuffer, WAVE_BITS, WAVE_PAD_LEFT,
        WAVE_PAD_RIGHT, WAVE_SIZE, WaveBuffer, ZEROES_BUFFER, get_wave_slice_mut, make_zero_buffer,
        make_zero_spectral_buffer, make_zero_wave_buffer, wrap_wave_buffer,
    },
    routing::{MAX_VOICES, ModuleId, ModuleInput, Router},
    synth_module::{NoteOnParams, ProcessParams, SynthModule},
};

const FULL_PHASE: f32 = ((u32::MAX as u64) + 1) as f32;
const INTERMEDIATE_BITS: usize = 32 - WAVE_BITS;
const INTERMEDIATE_MASK: u32 = (1 << INTERMEDIATE_BITS) - 1;
const INTERMEDIATE_MULT: f32 = ((1 << INTERMEDIATE_BITS) as f32).recip();
const PITCH_SHIFT_MOD_RANGE: f32 = 48.0;

struct OscillatorVoice {
    note: f32,
    phase: u32,
    wave_buffers_initialized: bool,
    wave_buffers_swapped: bool,
    level_mod_input: Buffer,
    pitch_shift_input: Buffer,
    output: Buffer,
    wave_buffers: (WaveBuffer, WaveBuffer),
}

impl Default for OscillatorVoice {
    fn default() -> Self {
        Self {
            note: 0.0,
            phase: 0,
            wave_buffers_initialized: false,
            wave_buffers_swapped: false,
            level_mod_input: make_zero_buffer(),
            pitch_shift_input: make_zero_buffer(),
            output: make_zero_buffer(),
            wave_buffers: (make_zero_wave_buffer(), make_zero_wave_buffer()),
        }
    }
}

pub struct OscillatorModule {
    module_id: ModuleId,
    level: f32,
    pitch_shift: f32,
    inverse_fft: Arc<dyn ComplexToReal<Sample>>,
    tmp_spectral_buff: SpectralBuffer,
    scratch_buff: SpectralBuffer,
    voices: [OscillatorVoice; MAX_VOICES],
}

impl OscillatorModule {
    pub fn new(module_id: ModuleId) -> Self {
        Self {
            module_id,
            level: 1.0,
            pitch_shift: 0.0,
            inverse_fft: RealFftPlanner::<Sample>::new().plan_fft_inverse(WAVE_SIZE),
            tmp_spectral_buff: make_zero_spectral_buffer(),
            scratch_buff: make_zero_spectral_buffer(),
            voices: Default::default(),
        }
    }

    #[inline(always)]
    fn calc_frequency(note: f32, pitch_shift: f32, pitch_shift_mod: f32) -> f32 {
        f32_midi_note_to_freq(note + pitch_shift + pitch_shift_mod * PITCH_SHIFT_MOD_RANGE)
    }

    fn build_wave(
        inverse_fft: &dyn ComplexToReal<Sample>,
        frequency: f32,
        sample_rate: f32,
        spectral_buff: &SpectralBuffer,
        tmp_spectral_buff: &mut SpectralBuffer,
        scratch_buff: &mut SpectralBuffer,
        out_wave_buff: &mut WaveBuffer,
    ) {
        let cutoff_index =
            ((0.5 * sample_rate / frequency).floor() as usize + 1).min(spectral_buff.len());

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

    fn process_voice(&mut self, params: &ProcessParams, router: &dyn Router, voice_idx: usize) {
        let sample_rate = params.sample_rate;
        let voice = &mut self.voices[voice_idx];
        let level_mod = router
            .get_input(
                ModuleInput::OscillatorLevel(self.module_id),
                voice_idx,
                &mut voice.level_mod_input,
            )
            .unwrap_or(&ONES_BUFFER);
        let pitch_shift_mod = router
            .get_input(
                ModuleInput::OscillatorPitchShift(self.module_id),
                voice_idx,
                &mut voice.pitch_shift_input,
            )
            .unwrap_or(&ZEROES_BUFFER);

        if !voice.wave_buffers_initialized {
            let spectral_buff = router.get_spectral_input(voice_idx).unwrap();
            let frequency = Self::calc_frequency(voice.note, self.pitch_shift, pitch_shift_mod[0]);

            Self::build_wave(
                self.inverse_fft.as_ref(),
                frequency,
                sample_rate,
                spectral_buff,
                &mut self.tmp_spectral_buff,
                &mut self.scratch_buff,
                &mut voice.wave_buffers.0,
            );

            voice.wave_buffers_initialized = true;
        }

        for (out, level_mod, pitch_shift_mod, _) in izip!(
            &mut voice.output,
            level_mod,
            pitch_shift_mod,
            0..params.samples
        ) {
            let frequency = Self::calc_frequency(voice.note, self.pitch_shift, *pitch_shift_mod);
            // let buff = get_wave_slice_mut(&mut voice.wave_buffers.0);
            let idx = (voice.phase >> INTERMEDIATE_BITS) as usize + WAVE_PAD_LEFT;
            let t = (voice.phase & INTERMEDIATE_MASK) as f32 * INTERMEDIATE_MULT;
            let knots = &voice.wave_buffers.0[(idx - WAVE_PAD_LEFT)..(idx + WAVE_PAD_RIGHT + 1)];
            let sample = spline::<CatmullRom, _, _>(t, knots).unwrap();

            *out = sample * self.level * level_mod;
            voice.phase = voice
                .phase
                .wrapping_add((frequency / sample_rate * FULL_PHASE) as u32);
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
            voice.phase = params.initial_phase;
        }
    }

    fn note_off(&mut self, _: &super::synth_module::NoteOffParams) {}

    fn process(&mut self, params: &ProcessParams, router: &dyn Router) {
        for voice_idx in &params.active_voices {
            self.process_voice(params, router, *voice_idx);
        }
    }
}
