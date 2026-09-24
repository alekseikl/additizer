use std::array;

use smallvec::SmallVec;

mod config;
mod link;
mod ui_bridge;

#[cfg(test)]
mod tests;

pub use config::{
    EqFilter, MAX_CUTOFF_HZ, MAX_EQ_FILTERS, MAX_Q, MIN_CUTOFF_HZ, MIN_Q, SpectralEqConfig,
};
use link::{AudioEnd, UiEnd, UiEvent, create_link_pair};
pub use ui_bridge::SpectralEqUiBridge;

use crate::{
    synth_engine::{
        StereoSample,
        buffer::VoicesLayout,
        filters::spectral_filter::{FilterParams, SpectralFilter as SpectralFilterEngine},
        routing::{
            DataType, Input, InputMeta, InputSlots, ModuleId, NUM_CHANNELS, ProcessContext,
            RouterFactory, SpectralInputSlot, SpectralOutput, SpectralRouterType, VoiceTarget,
        },
        spectral_filter::{MAX_CUTOFF, MAX_DRIVE, MIN_CUTOFF, MIN_DRIVE},
        synth_module::SynthModule,
        types::Sample,
    },
    utils::{C4_PITCH, MAX_LEVEL_DB, MIN_LEVEL_DB, db_to_gain_fast, freq_to_c4_pitch},
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
    q_limit_to: Sample,
    q_limit_slope: Sample,
    output_level: Sample,
}

impl ChannelParams {
    fn from_config(c: &SpectralEqConfig, channel_idx: usize) -> Self {
        Self {
            cutoff: c.cutoff[channel_idx],
            q_limit_to: c.q_limit_to[channel_idx],
            q_limit_slope: c.q_limit_slope[channel_idx],
            output_level: c.output_level[channel_idx],
        }
    }
}

pub struct Inputs {
    spectrum: Option<usize>,
    pitch: Option<usize>,
    cutoff: InputSlots,
    output_level: InputSlots,
}

impl Default for Inputs {
    fn default() -> Self {
        Self {
            spectrum: None,
            pitch: None,
            cutoff: InputSlots::new(Input::Cutoff),
            output_level: InputSlots::new(Input::Level),
        }
    }
}

impl Inputs {
    fn from_slots(inputs: &[InputSlots], spectral_inputs: &[SpectralInputSlot]) -> Self {
        let mut result = Self::default();

        for input in inputs {
            match input.input_type {
                Input::Pitch => result.pitch = input.slots.first().map(|s| s.src_slot),
                Input::Cutoff => result.cutoff = input.clone(),
                Input::Level => result.output_level = input.clone(),
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
            Input::Cutoff => self.cutoff.update_amount(src_slot, amount),
            Input::Level => self.output_level.update_amount(src_slot, amount),
            _ => (),
        }
    }
}

pub struct SpectralEq {
    id: ModuleId,
    params: Params,
    channel_params: [ChannelParams; NUM_CHANNELS],
    audio_end: AudioEnd,
    ui_end: Option<UiEnd>,
    inputs: Inputs,
    output_slot: usize,
}

impl SpectralEq {
    pub fn new(id: ModuleId) -> Self {
        Self::from_config(&SpectralEqConfig {
            id,
            ..SpectralEqConfig::default()
        })
    }

    pub fn from_config(config: &SpectralEqConfig) -> Self {
        let (audio_end, ui_end) = create_link_pair();

        Self {
            id: config.id,
            params: Params::from_config(config),
            channel_params: array::from_fn(|channel_idx| {
                ChannelParams::from_config(config, channel_idx)
            }),
            audio_end,
            ui_end: Some(ui_end),
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
            q_limit_to: get_stereo_param!(self, q_limit_to),
            q_limit_slope: get_stereo_param!(self, q_limit_slope),
            cutoff: get_stereo_param!(self, cutoff),
            output_level: get_stereo_param!(self, output_level),
        }
    }

    set_mono_param!(set_linear_phase, linear_phase, bool);
    set_mono_param!(set_keytrack, keytrack, Sample, keytrack.clamp(0.0, 1.0));

    set_stereo_param!(set_cutoff, cutoff, cutoff.clamp(MIN_CUTOFF, MAX_CUTOFF));
    set_stereo_param!(
        set_q_limit_to,
        q_limit_to,
        q_limit_to.clamp(MIN_CUTOFF, MAX_CUTOFF)
    );
    set_stereo_param!(
        set_q_limit_slope,
        q_limit_slope,
        q_limit_slope.clamp(0.0, 1.0)
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
        rf: &mut RouterFactory<SpectralRouterType>,
    ) {
        let (mut router, mut voice_output) = rf.for_voice(target, outputs);
        let inputs = &self.inputs;
        let channel = &self.channel_params[target.channel_idx];

        let cutoff_offset = router
            .scalar(&inputs.cutoff, channel.cutoff)
            .clamp(MIN_CUTOFF, MAX_CUTOFF);
        let q_limit_to = channel.q_limit_to.clamp(MIN_CUTOFF, MAX_CUTOFF);
        let q_limit_slope = channel.q_limit_slope.clamp(0.0, 1.0);
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
        let q_limit_to = (q_limit_to + keytrack_offset).clamp(MIN_CUTOFF, MAX_CUTOFF);

        output.copy_from_slice(&input[..output.len()]);

        for band in &self.params.filters {
            let cutoff = freq_to_c4_pitch(band.cutoff_hz) + cutoff_offset + keytrack_offset;
            let filter = SpectralFilterEngine::new(
                band.filter_type,
                FilterParams {
                    drive: band.drive.clamp(MIN_DRIVE, MAX_DRIVE),
                    cutoff,
                    q: band.q,
                    q_limit_to,
                    q_limit_slope,
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
}

impl SynthModule for SpectralEq {
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

    fn set_input_slots(&mut self, inputs: &[InputSlots], spectral_inputs: &[SpectralInputSlot]) {
        self.inputs = Inputs::from_slots(inputs, spectral_inputs);
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
                UiEvent::QLimitTo(value) => self.set_q_limit_to(value),
                UiEvent::QLimitSlope(value) => self.set_q_limit_slope(value),
                UiEvent::SetFilter { index, filter } => self.set_filter(index as usize, filter),
                UiEvent::AddFilter(filter) => self.add_filter(filter),
                UiEvent::RemoveFilter(index) => self.remove_filter(index as usize),
                UiEvent::MoveFilter { from, to } => self.move_filter(from as usize, to as usize),
            }
        }
    }

    fn process(&mut self, ctx: &mut ProcessContext) {
        ctx.spectral(self.id, self.output_slot)
            .for_voices(|rf, target, outputs| {
                self.process_voice(target, outputs, rf);
            });
    }
}
