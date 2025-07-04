use crate::{
    VOLUME_POLY_MOD_ID,
    envelope::{Envelope, adsr::ADSR, fade_out::FadeOutEnvelope},
    oscillator::AdditiveOscillator,
    phase::Phase,
    stereo_sample::StereoSample,
    utils::GlobalParamValues,
};
use nih_plug::{prelude::*, util::f32_midi_note_to_freq};
use rand::Rng;
use rand_pcg::Pcg32;
use std::{collections::HashMap, sync::Arc};

#[derive(Debug, Clone, Copy)]
pub struct VoiceId {
    pub voice_id: Option<i32>,
    pub channel: u8,
    pub note: u8,
}

impl VoiceId {
    pub fn new(channel: u8, note: u8, voice_id: Option<i32>) -> Self {
        Self {
            channel,
            note,
            voice_id,
        }
    }

    pub fn match_voice(&self, other: Self) -> bool {
        match other.voice_id {
            Some(_) => other.voice_id == self.voice_id,
            None => other.note == self.note && other.channel == self.channel,
        }
    }

    pub fn match_by_voice_id(&self, other_voice_id: i32) -> bool {
        match self.voice_id {
            Some(voice_id) => voice_id == other_voice_id,
            None => false,
        }
    }

    pub fn match_by_note(&self, other: Self) -> bool {
        self.channel == other.channel && self.note == other.note
    }
}

pub struct Voice {
    oscillators: Vec<AdditiveOscillator>,
    id: VoiceId,
    running: bool,
    releasing: bool,
    amp_envelope: Box<dyn Envelope + Send>,
    poly_modulations: HashMap<u32, (f32, Smoother<f32>)>,
}

impl Voice {
    pub fn new(random: &mut Pcg32, id: VoiceId, unison: usize, sine_table: &Arc<Vec<f32>>) -> Self {
        let mut oscillators: Vec<AdditiveOscillator> = Vec::with_capacity(unison);
        let mut phases: Vec<f32> = vec![0.0; unison];

        random.fill(&mut phases[..]);

        for phase in &phases {
            oscillators.push(AdditiveOscillator::new(Phase::new(*phase), sine_table));
        }

        Self {
            oscillators,
            id,
            running: false,
            releasing: false,
            amp_envelope: Box::new(ADSR::new(8.0, 560.0, 0.5, 500.0)),
            poly_modulations: HashMap::with_capacity(1),
        }
    }

    pub fn id(&self) -> &VoiceId {
        &self.id
    }

    pub fn match_releasing(&self, id: VoiceId, releasing: bool) -> bool {
        self.id.match_voice(id) && self.releasing == releasing
    }

    pub fn release(&mut self) {
        self.releasing = true;
        self.amp_envelope.release();
    }

    pub fn fade_out(&mut self) {
        self.releasing = true;
        self.amp_envelope = Box::new(FadeOutEnvelope::new(self.amp_envelope.value()))
    }

    pub fn current_phase(&self) -> &Phase {
        self.oscillators[0].phase()
    }

    pub fn is_done(&self) -> bool {
        self.releasing && self.amp_envelope.is_done()
    }

    pub fn apply_poly_modulation(
        &mut self,
        sample_rate: f32,
        modulation_id: u32,
        param: &FloatParam,
        normalized_offset: f32,
    ) {
        let target_plain_value = param.preview_modulated(normalized_offset);
        let (offset, smoother) = self
            .poly_modulations
            .entry(modulation_id)
            .or_insert_with(|| (normalized_offset, param.smoothed.clone()));

        *offset = normalized_offset;

        if self.running {
            smoother.set_target(sample_rate, target_plain_value);
        } else {
            smoother.reset(target_plain_value);
        }
    }

    pub fn apply_mono_automation(
        &mut self,
        sample_rate: f32,
        modulation_id: u32,
        param: &FloatParam,
        normalized_value: f32,
    ) {
        if let Some((normalized_offset, smoother)) = self.poly_modulations.get(&modulation_id) {
            let target_plain_value = param.preview_plain(normalized_value + *normalized_offset);
            smoother.set_target(sample_rate, target_plain_value);
        }
    }

    fn next_modulation_value(&mut self, poly_modulation_id: u32, fallback_value: f32) -> f32 {
        self.poly_modulations
            .get(&poly_modulation_id)
            .map(|(_, smoother)| smoother.next())
            .unwrap_or(fallback_value)
    }

    pub fn tick(&mut self, sample_rate: f32, global_params: &GlobalParamValues) -> StereoSample {
        self.running = true;

        let volume = self.next_modulation_value(VOLUME_POLY_MOD_ID, global_params.volume);
        let gain = self.amp_envelope.value() * util::db_to_gain(volume);
        let mut result = StereoSample(0.0, 0.0);
        let note_range = 0.01 * global_params.detune;
        let note_step = note_range / self.oscillators.len() as f32;
        let start_note = self.id.note as f32 - 0.5 * note_range + 0.5 * note_step;
        let attenuate = ((self.oscillators.len() as f32).sqrt()).recip();

        for (i, osc) in self.oscillators.iter_mut().enumerate() {
            result += osc.tick(
                sample_rate,
                f32_midi_note_to_freq(start_note + note_step * i as f32),
                0.0,
                global_params,
            ) * attenuate;
        }

        self.amp_envelope.advance(sample_rate);
        result * gain
    }
}
