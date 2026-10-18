use crate::synth_engine::{
    StereoSample,
    routing::{Input, InputSlots, MixedSlots},
};

pub struct Inputs {
    pub spectrum: Option<usize>,
    pub pitch: Option<usize>,
    pub phase_shift: MixedSlots,
    pub freq_shift: MixedSlots,
    pub detune: MixedSlots,
    pub detune_focus: MixedSlots,
    pub phase_steal: MixedSlots,
    pub phases_blend: MixedSlots,
    pub gains_blend: MixedSlots,
}

impl Default for Inputs {
    fn default() -> Self {
        Self {
            spectrum: None,
            pitch: None,
            phase_shift: MixedSlots::new(Input::PhaseShift),
            freq_shift: MixedSlots::new(Input::FrequencyShift),
            detune: MixedSlots::new(Input::Detune),
            detune_focus: MixedSlots::new(Input::DetuneFocus),
            phase_steal: MixedSlots::new(Input::PhaseSteal),
            phases_blend: MixedSlots::new(Input::PhasesBlend),
            gains_blend: MixedSlots::new(Input::LevelsBlend),
        }
    }
}

impl Inputs {
    pub fn from_slots(inputs: &[InputSlots]) -> Self {
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
                    Input::PhaseShift => result.phase_shift = input.clone(),
                    Input::FrequencyShift => result.freq_shift = input.clone(),
                    Input::Detune => result.detune = input.clone(),
                    Input::DetuneFocus => result.detune_focus = input.clone(),
                    Input::PhaseSteal => result.phase_steal = input.clone(),
                    Input::PhasesBlend => result.phases_blend = input.clone(),
                    Input::LevelsBlend => result.gains_blend = input.clone(),
                    _ => (),
                },
            }
        }

        result
    }

    pub fn update_amount(&mut self, input_type: Input, src_slot: usize, amount: StereoSample) {
        match input_type {
            Input::PhaseShift => self.phase_shift.update_amount(src_slot, amount),
            Input::FrequencyShift => self.freq_shift.update_amount(src_slot, amount),
            Input::Detune => self.detune.update_amount(src_slot, amount),
            Input::DetuneFocus => self.detune_focus.update_amount(src_slot, amount),
            Input::PhaseSteal => self.phase_steal.update_amount(src_slot, amount),
            Input::PhasesBlend => self.phases_blend.update_amount(src_slot, amount),
            Input::LevelsBlend => self.gains_blend.update_amount(src_slot, amount),
            _ => (),
        }
    }
}
