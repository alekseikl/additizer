use std::array;

use rand::RngExt;
use rand_pcg::Pcg32;

use crate::{
    synth_engine::{
        ComplexSample, Sample, StereoSample,
        buffer::{DC_OFFSET, SPECTRAL_BUFFER_SIZE, SpectralBuffer, VoicesLayout},
        harmonic_editor::config::fill_default_harmonics,
        routing::{
            DataType, InputMeta, LEFT_CHANNEL, ModuleId, NUM_CHANNELS, ProcessContext,
            RIGHT_CHANNEL, RouterFactory, SpectralOutput, SpectralRouterType, VoiceTarget,
        },
        synth_module::SynthModule,
    },
    utils::{NthElement, db_to_gain},
};

mod config;
mod link;
mod ui_bridge;

pub use config::{HarmonicEditorConfig, sawtooth_phase};
pub use link::Harmonics;
pub use ui_bridge::HarmonicEditorUiBridge;

use itertools::izip;
use link::{AudioEnd, UiEnd, UiEvent, create_link_pair};

const DB_LIMIT: Sample = 24.0;
const MIN_RANDOM_LEVEL_DB: Sample = -48.0;

#[derive(Clone, Copy)]
pub enum EditRequest {
    Range {
        harmonic_from: u16,
        harmonic_to: u16,
        gain: StereoSample,
    },
    NthElement {
        harmonic_from: u16,
        harmonic_to: u16,
        mul: u8,
        add: u8,
        gain: StereoSample,
    },
    RandomAmplitudes {
        level_from: Sample,
        level_to: Sample,
        stereo: Sample,
    },
}

pub struct HarmonicEditor {
    id: ModuleId,
    audio_end: AudioEnd,
    ui_end: Option<UiEnd>,
    output_slot: usize,
    amplitudes: [Box<[Sample; SPECTRAL_BUFFER_SIZE]>; NUM_CHANNELS],
    phases: [Box<[Sample; SPECTRAL_BUFFER_SIZE]>; NUM_CHANNELS],
    amplitudes_draft: [Box<[Sample; SPECTRAL_BUFFER_SIZE]>; NUM_CHANNELS],
    draft_enabled: bool,
    output_harmonics: [Box<SpectralBuffer>; NUM_CHANNELS],
    random: Pcg32,
}

impl HarmonicEditor {
    pub fn new(id: ModuleId) -> Self {
        Self::from_config(&HarmonicEditorConfig {
            id,
            ..HarmonicEditorConfig::default()
        })
    }

    pub fn from_config(config: &config::HarmonicEditorConfig) -> Self {
        let (audio_end, ui_end) = create_link_pair();

        let mut amplitudes = array::from_fn(|_| Box::new([0.0; SPECTRAL_BUFFER_SIZE]));
        let mut phases = array::from_fn(|_| Box::new([0.0; SPECTRAL_BUFFER_SIZE]));

        let gain_limit = db_to_gain(DB_LIMIT);

        for (amplitudes, phases, cfg_amplitudes, cfg_phases) in izip!(
            amplitudes.iter_mut(),
            phases.iter_mut(),
            config.amplitudes.iter(),
            config.phases.iter()
        ) {
            for (amp, phase, &cfg_amp, &cfg_phase) in izip!(
                amplitudes.iter_mut(),
                phases.iter_mut(),
                cfg_amplitudes.iter(),
                cfg_phases.iter()
            )
            .skip(DC_OFFSET)
            {
                *amp = cfg_amp.min(gain_limit);
                *phase = cfg_phase;
            }
        }

        let amplitudes_draft = array::from_fn(|_| Box::new([0.0; SPECTRAL_BUFFER_SIZE]));

        let output_harmonics =
            array::from_fn(|_| Box::new([ComplexSample::ZERO; SPECTRAL_BUFFER_SIZE]));

        let mut editor = Self {
            id: config.id,
            audio_end,
            ui_end: Some(ui_end),
            output_slot: usize::MAX,
            amplitudes,
            phases,
            amplitudes_draft,
            draft_enabled: false,
            output_harmonics,
            random: Pcg32::new(0x2b992ddfa23249d6, 0x9e3779b97f4a7c15),
        };

        editor.rebuild_harmonics();
        editor
            .audio_end
            .publish_harmonics(&editor.amplitudes, &editor.phases);
        editor
    }

    pub fn get_config(&self) -> HarmonicEditorConfig {
        HarmonicEditorConfig {
            id: self.id,
            amplitudes: array::from_fn(|c| Vec::from_iter(self.amplitudes[c].iter().copied())),
            phases: array::from_fn(|c| Vec::from_iter(self.phases[c].iter().copied())),
        }
    }

    fn frequency_bin(idx: usize, amp: Sample, phase: Sample) -> ComplexSample {
        ComplexSample::from_polar(
            amp / (idx as Sample * std::f32::consts::PI),
            (phase + 0.25) * std::f32::consts::TAU,
        )
    }

    fn rebuild_harmonics(&mut self) {
        let amplitudes = if self.draft_enabled {
            self.amplitudes_draft.iter()
        } else {
            self.amplitudes.iter()
        };

        for (harmonics, amplitudes, phases) in izip!(
            self.output_harmonics.iter_mut(),
            amplitudes,
            self.phases.iter()
        ) {
            for (idx, (bin, &amp, &phase)) in
                izip!(harmonics.iter_mut(), amplitudes.iter(), phases.iter())
                    .enumerate()
                    .skip(DC_OFFSET)
            {
                *bin = Self::frequency_bin(idx, amp, phase);
            }
        }

        self.audio_end
            .update_display_spectrum(&*self.output_harmonics[LEFT_CHANNEL]);
    }

    fn rebuild_harmonic(&mut self, idx: usize) {
        assert!((DC_OFFSET..SPECTRAL_BUFFER_SIZE).contains(&idx));

        for (harmonics, amplitudes, phases) in izip!(
            self.output_harmonics.iter_mut(),
            self.amplitudes.iter(),
            self.phases.iter()
        ) {
            harmonics[idx] = Self::frequency_bin(idx, amplitudes[idx], phases[idx]);
        }

        self.audio_end
            .update_display_spectrum(&*self.output_harmonics[LEFT_CHANNEL]);
    }

    pub fn set_amplitude(&mut self, idx: usize, amplitude: StereoSample) {
        for (amplitudes, &amplitude) in izip!(self.amplitudes.iter_mut(), amplitude.iter()) {
            amplitudes[idx] = amplitude.min(db_to_gain(DB_LIMIT));
        }

        self.rebuild_harmonic(idx);
    }

    pub fn set_phase(&mut self, idx: usize, phase: StereoSample) {
        for (phases, &phase) in izip!(self.phases.iter_mut(), phase.iter()) {
            phases[idx] = phase.clamp(0.0, 1.0);
        }

        self.rebuild_harmonic(idx);
    }

    pub fn apply_draft(&mut self) {
        for (amplitudes, amplitudes_draft) in
            izip!(self.amplitudes.iter_mut(), self.amplitudes_draft.iter())
        {
            amplitudes.copy_from_slice(amplitudes_draft.as_slice());
        }

        self.draft_enabled = false;
        self.rebuild_harmonics();
        self.audio_end
            .publish_harmonics(&self.amplitudes, &self.phases);
    }

    pub fn discard_draft(&mut self) {
        self.draft_enabled = false;
        self.rebuild_harmonics();
    }

    fn apply_range_set(
        &mut self,
        harmonic_from: usize,
        harmonic_to: usize,
        n_th: Option<NthElement>,
        gain: StereoSample,
    ) {
        let from = harmonic_from.clamp(DC_OFFSET, SPECTRAL_BUFFER_SIZE - 1);
        let to = harmonic_to.clamp(from, SPECTRAL_BUFFER_SIZE - 1);
        let gain_limit = db_to_gain(DB_LIMIT);

        for (amplitudes_draft, &gain) in izip!(self.amplitudes_draft.iter_mut(), gain.iter()) {
            let gain = gain.min(gain_limit);

            for (offset, amp) in amplitudes_draft[from..=to].iter_mut().enumerate() {
                let harmonic_idx = from + offset;

                if n_th.as_ref().is_none_or(|n_th| n_th.matches(harmonic_idx)) {
                    *amp = gain;
                }
            }
        }
    }

    fn reflect_into_range(value: Sample, min: Sample, max: Sample) -> Sample {
        if min >= max {
            return min;
        }

        let range = max - min;
        let period = 2.0 * range;
        let mut t = (value - min).rem_euclid(period);

        if t > range {
            t = period - t;
        }

        min + t
    }

    fn apply_random_amplitudes(
        &mut self,
        level_from: Sample,
        level_to: Sample,
        stereo: Sample,
    ) {
        let level_from = level_from.clamp(MIN_RANDOM_LEVEL_DB, DB_LIMIT);
        let level_to = level_to.clamp(MIN_RANDOM_LEVEL_DB, DB_LIMIT);
        let (level_from, level_to) = if level_from <= level_to {
            (level_from, level_to)
        } else {
            (level_to, level_from)
        };
        let stereo = stereo.clamp(0.0, 1.0);
        let stereo_amount = (level_to - level_from) * stereo;
        let gain_limit = db_to_gain(DB_LIMIT);

        for idx in DC_OFFSET..SPECTRAL_BUFFER_SIZE {
            let center =
                level_from + (level_to - level_from) * self.random.random::<Sample>();
            let left_db = Self::reflect_into_range(
                center + stereo_amount * (self.random.random::<Sample>() - 0.5),
                level_from,
                level_to,
            );
            let right_db = Self::reflect_into_range(
                center + stereo_amount * (self.random.random::<Sample>() - 0.5),
                level_from,
                level_to,
            );

            self.amplitudes_draft[LEFT_CHANNEL][idx] = db_to_gain(left_db).min(gain_limit);
            self.amplitudes_draft[RIGHT_CHANNEL][idx] = db_to_gain(right_db).min(gain_limit);
        }
    }

    pub fn apply_edit_request(&mut self, request: EditRequest) {
        for (amplitudes_draft, amplitudes) in
            izip!(self.amplitudes_draft.iter_mut(), self.amplitudes.iter())
        {
            amplitudes_draft.copy_from_slice(amplitudes.as_slice());
        }
        self.draft_enabled = true;

        match request {
            EditRequest::Range {
                harmonic_from,
                harmonic_to,
                gain,
            } => {
                self.apply_range_set(harmonic_from as usize, harmonic_to as usize, None, gain);
            }
            EditRequest::NthElement {
                harmonic_from,
                harmonic_to,
                mul,
                add,
                gain,
            } => {
                self.apply_range_set(
                    harmonic_from as usize,
                    harmonic_to as usize,
                    Some(NthElement::new(mul as isize, add as isize, false)),
                    gain,
                );
            }
            EditRequest::RandomAmplitudes {
                level_from,
                level_to,
                stereo,
            } => {
                self.apply_random_amplitudes(level_from, level_to, stereo);
            }
        }
    }

    pub fn clear(&mut self) {
        for amplitudes in self.amplitudes.iter_mut() {
            amplitudes.fill(0.0);
        }

        self.rebuild_harmonics();
        self.audio_end
            .publish_harmonics(&self.amplitudes, &self.phases);
    }

    pub fn reset_saw(&mut self) {
        for (amplitudes, phases) in izip!(self.amplitudes.iter_mut(), self.phases.iter_mut()) {
            fill_default_harmonics(amplitudes.iter_mut(), phases.iter_mut());
        }

        self.rebuild_harmonics();
        self.audio_end
            .publish_harmonics(&self.amplitudes, &self.phases);
    }

    fn process_voice(
        &mut self,
        target: &VoiceTarget,
        outputs: &mut VoicesLayout<SpectralOutput>,
        rf: &mut RouterFactory<SpectralRouterType>,
    ) {
        let (_, mut voice_output) = rf.for_voice(target, outputs);
        let out = voice_output.output();

        out.copy_from_slice(&self.output_harmonics[target.channel_idx][..out.len()]);
    }
}

impl SynthModule for HarmonicEditor {
    fn id(&self) -> ModuleId {
        self.id
    }

    fn inputs(&self) -> &'static [InputMeta] {
        &[]
    }

    fn output_type(&self) -> DataType {
        DataType::Spectral
    }

    fn output_slot(&self) -> usize {
        self.output_slot
    }

    fn set_output_slot(&mut self, slot: usize) {
        self.output_slot = slot;
    }

    fn process_ui_events(&mut self) {
        while let Some(event) = self.audio_end.pop_event() {
            match event {
                UiEvent::SetAmplitude { index, gain } => {
                    self.set_amplitude(index as usize, gain);
                }
                UiEvent::SetPhase { index, phase } => {
                    self.set_phase(index as usize, phase);
                }
                UiEvent::Clear => {
                    self.clear();
                }
                UiEvent::ResetSawtooth => {
                    self.reset_saw();
                }
                UiEvent::EditRequest(request) => {
                    self.apply_edit_request(request);
                    self.rebuild_harmonics();
                }
                UiEvent::ApplyDraft => {
                    self.apply_draft();
                }
                UiEvent::DiscardDraft => {
                    self.discard_draft();
                }
            }
        }
    }

    fn process(&mut self, ctx: &mut ProcessContext) {
        ctx.for_spectral(self.id, self.output_slot, |rf, target, outputs| {
            self.process_voice(target, outputs, rf);
        });
    }
}
