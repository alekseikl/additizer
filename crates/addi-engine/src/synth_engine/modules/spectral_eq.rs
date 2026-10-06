use std::array;

use smallvec::SmallVec;

mod config;
mod link;
pub mod stub;

#[cfg(test)]
mod tests;

pub use config::{
    EqFilter, MAX_CUTOFF_HZ, MAX_EQ_FILTERS, MAX_Q, MIN_CUTOFF_HZ, MIN_Q, SpectralEqConfig,
};
pub use link::{SpectralEqAudioEnd, SpectralEqLinks, SpectralEqUiEnd, UiEvent};

use addi_dsp::filters::{
    control::{MAX_DRIVE, MAX_PRE_Q, MIN_DRIVE},
    spectral_filter::{FilterParams, SpectralFilter as SpectralFilterEngine},
};

use crate::{
    synth_engine::{
        C4_PITCH, MAX_CUTOFF, MAX_LEVEL_DB, MIN_CUTOFF, MIN_LEVEL_DB, db_to_gain_fast,
        freq_to_c4_pitch,
    },
    synth_engine::{
        StereoSample,
        buffer::VoicesLayout,
        routing::{
            DataType, Input, InputMeta, InputSlots, MixedSlots, ModuleId, NUM_CHANNELS,
            ProcessContext, RouterFactory, SpectralOutput, SpectralRouterType, VoiceTarget,
        },
        spectral_filter::{MAX_Q_ROLLOFF, MIN_Q_ROLLOFF},
        synth_module::SynthModule,
        types::Sample,
    },
};

struct Params {
    filters: SmallVec<[EqFilter; MAX_EQ_FILTERS]>,
    linear_phase: bool,
    keytrack: Sample,
}

impl Params {
    fn from_config(c: &SpectralEqConfig) -> Self {
        let filters = c
            .filters
            .iter()
            .take(MAX_EQ_FILTERS)
            .map(|filter| filter.sanitized())
            .collect();

        Self {
            filters,
            linear_phase: c.linear_phase,
            keytrack: c.keytrack.clamp(0.0, 1.0),
        }
    }
}

struct ChannelParams {
    cutoff: Sample,
    q_cutoff: Sample,
    q_rolloff: Sample,
    output_level: Sample,
}

impl ChannelParams {
    fn from_config(c: &SpectralEqConfig, channel_idx: usize) -> Self {
        Self {
            cutoff: c.cutoff[channel_idx],
            q_cutoff: c.q_cutoff[channel_idx],
            q_rolloff: c.q_rolloff[channel_idx].clamp(MIN_Q_ROLLOFF, MAX_Q_ROLLOFF),
            output_level: c.output_level[channel_idx],
        }
    }
}

pub struct Inputs {
    spectrum: Option<usize>,
    pitch: Option<usize>,
    cutoff: MixedSlots,
    output_level: MixedSlots,
}

impl Default for Inputs {
    fn default() -> Self {
        Self {
            spectrum: None,
            pitch: None,
            cutoff: MixedSlots::new(Input::Cutoff),
            output_level: MixedSlots::new(Input::Level),
        }
    }
}

impl Inputs {
    fn from_slots(inputs: &[InputSlots]) -> Self {
        let mut result = Self::default();

        for input in inputs {
            match input {
                InputSlots::Direct { input_type, slot } => {
                    if matches!(input_type, Input::Pitch) {
                        result.pitch = Some(*slot);
                    }
                }
                InputSlots::Spectral { input_type, slot } => {
                    if matches!(input_type, Input::Spectrum) {
                        result.spectrum = Some(*slot);
                    }
                }
                InputSlots::Mixed(input) => match input.input_type {
                    Input::Cutoff => result.cutoff = input.clone(),
                    Input::Level => result.output_level = input.clone(),
                    _ => (),
                },
            }
        }

        result
    }

    fn update_amount(&mut self, input_type: Input, src_slot: usize, amount: StereoSample) {
        match input_type {
            Input::Cutoff => self.cutoff.update_amount(src_slot, amount),
            Input::Level => self.output_level.update_amount(src_slot, amount),
            _ => (),
        }
    }
}

pub struct SpectralEq<L: SpectralEqLinks = stub::Links> {
    id: ModuleId,
    params: Params,
    channel_params: [ChannelParams; NUM_CHANNELS],
    audio_end: L::AudioEnd,
    ui_end: Option<L::UiEnd>,
    inputs: Inputs,
    output_slot: usize,
}

impl<L: SpectralEqLinks> SpectralEq<L> {
    pub fn take_ui_end(&mut self) -> Option<L::UiEnd> {
        self.ui_end.take()
    }

    pub fn new(id: ModuleId) -> Self {
        Self::from_config(&SpectralEqConfig {
            id,
            ..SpectralEqConfig::default()
        })
    }

    pub fn from_config(config: &SpectralEqConfig) -> Self {
        let (audio_end, ui_end) = L::create_link_pair();

        Self {
            id: config.id,
            params: Params::from_config(config),
            channel_params: array::from_fn(|channel_idx| {
                ChannelParams::from_config(config, channel_idx)
            }),
            audio_end,
            ui_end,
            inputs: Inputs::default(),
            output_slot: usize::MAX,
        }
    }

    pub fn get_config(&self) -> SpectralEqConfig {
        SpectralEqConfig {
            id: self.id,
            filters: self.params.filters.to_vec(),
            linear_phase: self.params.linear_phase,
            keytrack: self.params.keytrack,
            q_cutoff: get_stereo_param!(self, q_cutoff),
            q_rolloff: get_stereo_param!(self, q_rolloff),
            cutoff: get_stereo_param!(self, cutoff),
            output_level: get_stereo_param!(self, output_level),
        }
    }

    set_mono_param!(set_linear_phase, linear_phase, bool);
    set_mono_param!(set_keytrack, keytrack, Sample, keytrack.clamp(0.0, 1.0));

    set_stereo_param!(set_cutoff, cutoff, cutoff.clamp(MIN_CUTOFF, MAX_CUTOFF));
    set_stereo_param!(
        set_q_cutoff,
        q_cutoff,
        q_cutoff.clamp(MIN_CUTOFF, MAX_CUTOFF)
    );
    set_stereo_param!(
        set_q_rolloff,
        q_rolloff,
        q_rolloff.clamp(MIN_Q_ROLLOFF, MAX_Q_ROLLOFF)
    );
    set_stereo_param!(
        set_output_level,
        output_level,
        output_level.clamp(MIN_LEVEL_DB, MAX_LEVEL_DB)
    );

    fn set_filter(&mut self, index: usize, filter: EqFilter) {
        if let Some(slot) = self.params.filters.get_mut(index) {
            *slot = filter.sanitized();
        }
    }

    fn add_filter(&mut self, filter: EqFilter) {
        if self.params.filters.len() >= MAX_EQ_FILTERS {
            return;
        }

        self.params.filters.push(filter.sanitized());
    }

    fn remove_filter(&mut self, index: usize) {
        if index < self.params.filters.len() {
            self.params.filters.remove(index);
        }
    }

    fn move_filter(&mut self, from: usize, to: usize) {
        let len = self.params.filters.len();

        if from >= len || to >= len || from == to {
            return;
        }

        let filter = self.params.filters.remove(from);
        self.params.filters.insert(to, filter);
    }

    fn process_voice(
        &mut self,
        target: &VoiceTarget,
        outputs: &mut VoicesLayout<SpectralOutput>,
        rf: &mut RouterFactory<SpectralRouterType, L::EngineEnd>,
    ) {
        let (mut router, mut voice_output) = rf.for_voice(target, outputs);
        let inputs = &self.inputs;
        let channel = &self.channel_params[target.channel_idx];

        let cutoff_offset = router
            .scalar(&inputs.cutoff, channel.cutoff)
            .clamp(MIN_CUTOFF, MAX_CUTOFF);
        let q_cutoff = channel.q_cutoff.clamp(MIN_CUTOFF, MAX_CUTOFF);
        let q_rolloff = channel.q_rolloff.clamp(MIN_Q_ROLLOFF, MAX_Q_ROLLOFF);
        let keytrack = self.params.keytrack;
        let output_level = router
            .scalar(&inputs.output_level, channel.output_level)
            .clamp(MIN_LEVEL_DB, MAX_LEVEL_DB);
        let linear_phase = self.params.linear_phase;
        let pitch = router
            .direct_opt(inputs.pitch)
            .unwrap_or_else(|| target.note_pitch());
        let input = router.spectral(inputs.spectrum);
        let output = voice_output.output(input.len());

        if router.need_update_ui_mono() {
            self.audio_end.update_pitch(pitch);
        }

        let keytrack_offset = (C4_PITCH - pitch) * (1.0 - keytrack);
        let q_cutoff = (q_cutoff + keytrack_offset).clamp(MIN_CUTOFF, MAX_CUTOFF);

        output.copy_from_slice(&input[..output.len()]);

        for band in &self.params.filters {
            let cutoff = freq_to_c4_pitch(band.cutoff_hz) + cutoff_offset + keytrack_offset;
            let filter = SpectralFilterEngine::new(
                band.filter_type,
                FilterParams {
                    drive: band.drive.clamp(MIN_DRIVE, MAX_DRIVE),
                    cutoff,
                    q: band.q,
                    pre_q: MAX_PRE_Q,
                    q_cutoff,
                    q_rolloff,
                    linear_phase,
                },
            );

            filter.apply_response_in_place(output);
        }

        if output_level.abs() > 1e-4 {
            let gain = db_to_gain_fast(output_level.min(MAX_LEVEL_DB));

            for out in output.iter_mut() {
                *out *= gain;
            }
        }
    }
    pub(crate) fn process(&mut self, ctx: &mut ProcessContext<L::EngineEnd>) {
        ctx.spectral(self.id, self.output_slot)
            .for_voices(|rf, target, outputs| {
                self.process_voice(target, outputs, rf);
            });
    }
}

impl<L: SpectralEqLinks> SynthModule for SpectralEq<L> {
    fn id(&self) -> ModuleId {
        self.id
    }

    fn inputs(&self) -> &'static [InputMeta] {
        static INPUTS: &[InputMeta] = &[
            InputMeta::spectral(Input::Spectrum),
            InputMeta::direct_control(Input::Pitch),
            InputMeta::control(Input::Cutoff),
            InputMeta::control(Input::Level),
        ];

        INPUTS
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

    fn set_input_slots(&mut self, inputs: &[InputSlots]) {
        self.inputs = Inputs::from_slots(inputs);
    }

    fn update_input_amount(&mut self, input_type: Input, src_slot: usize, amount: StereoSample) {
        self.inputs.update_amount(input_type, src_slot, amount);
    }

    fn process_ui_events(&mut self) {
        while let Some(event) = self.audio_end.pop_event() {
            match event {
                UiEvent::InputParam { input, value } => match input {
                    Input::Cutoff => self.set_cutoff(value),
                    Input::Level => self.set_output_level(value),
                    _ => (),
                },
                UiEvent::LinearPhase(value) => self.set_linear_phase(value),
                UiEvent::Keytrack(value) => self.set_keytrack(value),
                UiEvent::QCutoff(value) => self.set_q_cutoff(value),
                UiEvent::QRolloff(value) => self.set_q_rolloff(value),
                UiEvent::SetFilter { index, filter } => self.set_filter(index as usize, filter),
                UiEvent::AddFilter(filter) => self.add_filter(filter),
                UiEvent::RemoveFilter(index) => self.remove_filter(index as usize),
                UiEvent::MoveFilter { from, to } => self.move_filter(from as usize, to as usize),
            }
        }
    }
}
