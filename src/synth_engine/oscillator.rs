use std::f32;

use itertools::izip;
use nih_plug::util::f32_midi_note_to_freq;

use crate::{
    buffer::{Buffer, ONES_BUFFER, ZEROES_BUFFER, make_zero_buffer},
    synth_engine::{
        routing::{MAX_VOICES, ModuleId, ModuleInput, Router},
        synth_module::{NoteOnParams, ProcessParams, SynthModule},
    },
};

const FULL_PHASE: f32 = ((u32::MAX as u64) + 1) as f32;

struct OscillatorVoice {
    note: f32,
    phase: u32,
    level_mod_input: Buffer,
    pitch_shift_input: Buffer,
    output: Buffer,
}

impl Default for OscillatorVoice {
    fn default() -> Self {
        Self {
            note: 0.0,
            phase: 0,
            level_mod_input: make_zero_buffer(),
            pitch_shift_input: make_zero_buffer(),
            output: make_zero_buffer(),
        }
    }
}

pub struct OscillatorModule {
    module_id: ModuleId,
    level: f32,
    pitch_shift: f32,
    voices: [OscillatorVoice; MAX_VOICES],
}

impl OscillatorModule {
    pub fn new(module_id: ModuleId) -> Self {
        Self {
            module_id,
            level: 0.5,
            pitch_shift: 0.0,
            voices: Default::default(),
        }
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

        for (out, level_mod, pitch_shift_mod, _) in izip!(
            &mut voice.output,
            level_mod,
            pitch_shift_mod,
            0..params.samples
        ) {
            let frequency =
                f32_midi_note_to_freq(voice.note + self.pitch_shift + pitch_shift_mod * 48.0);

            *out =
                (voice.phase as f32 / FULL_PHASE * f32::consts::TAU).sin() * self.level * level_mod;
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
