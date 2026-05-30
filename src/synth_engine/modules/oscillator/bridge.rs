use crate::synth_engine::{
    Input, Sample, StereoSample,
    oscillator::{MAX_UNISON_VOICES, PhasesDst},
    synth_module::ModuleToUiBridge,
};

pub struct UnisonUiState {
    pub initial_phase: StereoSample,
    pub phase_shift: StereoSample,
    pub phase_shift_to: StereoSample,
    pub gain: StereoSample,
    pub gain_to: StereoSample,
}
pub struct UiState {
    pub gain: StereoSample,
    pub pitch_shift: StereoSample,
    pub detune: StereoSample,
    pub detune_power: StereoSample,
    pub glide: StereoSample,
    pub glide_slope: StereoSample,
    pub phase_shift: StereoSample,
    pub frequency_shift: StereoSample,
    pub unison: usize,
    pub steal_phase: bool,
    pub phases_blend: StereoSample,
    pub gains_blend: StereoSample,
    pub unison_params: [UnisonUiState; MAX_UNISON_VOICES],
}

pub enum UiEvent {
    InputParam {
        input: Input,
        value: StereoSample,
    },
    Unison(usize),
    UnisonInitialPhase {
        idx: usize,
        value: StereoSample,
    },
    UnisonPhaseShift {
        idx: usize,
        value: StereoSample,
    },
    UnisonPhaseShiftTo {
        idx: usize,
        value: StereoSample,
    },
    UnisonGain {
        idx: usize,
        value: StereoSample,
    },
    UnisonGainTo {
        idx: usize,
        value: StereoSample,
    },
    StealPhase(bool),
    ApplyUnisonLevelShape {
        center: StereoSample,
        level: StereoSample,
        to: bool,
    },
    RandomizePhases {
        from: Sample,
        to: Sample,
        stereo_spread: Sample,
        dst: PhasesDst,
    },
}

pub enum UiUpdate {
    ModulatedInput {
        input: Input,
        channel: u8,
        value: Sample,
    },
    Output {
        channel: u8,
        value: Sample,
    },
    RefreshState,
}

pub struct AudioBridge {
    rx: rtrb::Consumer<UiUpdate>,
    tx: rtrb::Producer<UiEvent>,
}

impl AudioBridge {
    pub fn new(rx: rtrb::Consumer<UiUpdate>, tx: rtrb::Producer<UiEvent>) -> Self {
        Self { rx, tx }
    }

    pub fn set_param(&mut self, input: Input, value: StereoSample) {
        let _ = self.tx.push(UiEvent::InputParam { input, value });
    }

    pub fn set_unison(&mut self, unison: usize) {
        let _ = self.tx.push(UiEvent::Unison(unison));
    }

    pub fn set_steal_phase(&mut self, steal_phase: bool) {
        let _ = self.tx.push(UiEvent::StealPhase(steal_phase));
    }

    pub fn set_unison_initial_phase(&mut self, idx: usize, value: StereoSample) {
        let _ = self.tx.push(UiEvent::UnisonInitialPhase { idx, value });
    }

    pub fn set_unison_phase_shift(&mut self, idx: usize, value: StereoSample) {
        let _ = self.tx.push(UiEvent::UnisonPhaseShift { idx, value });
    }

    pub fn set_unison_phase_shift_to(&mut self, idx: usize, value: StereoSample) {
        let _ = self.tx.push(UiEvent::UnisonPhaseShiftTo { idx, value });
    }

    pub fn set_unison_gain(&mut self, idx: usize, value: StereoSample) {
        let _ = self.tx.push(UiEvent::UnisonGain { idx, value });
    }

    pub fn set_unison_gain_to(&mut self, idx: usize, value: StereoSample) {
        let _ = self.tx.push(UiEvent::UnisonGainTo { idx, value });
    }

    pub fn apply_unison_level_shape(
        &mut self,
        center: StereoSample,
        level: StereoSample,
        to: bool,
    ) {
        let _ = self
            .tx
            .push(UiEvent::ApplyUnisonLevelShape { center, level, to });
    }

    pub fn randomize_phases(
        &mut self,
        from: Sample,
        to: Sample,
        stereo_spread: Sample,
        dst: PhasesDst,
    ) {
        let _ = self.tx.push(UiEvent::RandomizePhases {
            from,
            to,
            stereo_spread,
            dst,
        });
    }

    pub fn pop_update(&mut self) -> Option<UiUpdate> {
        self.rx.pop().ok()
    }
}

pub struct UiBridge {
    rx: rtrb::Consumer<UiEvent>,
    tx: rtrb::Producer<UiUpdate>,
}

impl UiBridge {
    pub fn new(rx: rtrb::Consumer<UiEvent>, tx: rtrb::Producer<UiUpdate>) -> Self {
        Self { rx, tx }
    }

    pub fn pop_event(&mut self) -> Option<UiEvent> {
        self.rx.pop().ok()
    }

    pub fn push_refresh_state(&mut self) {
        let _ = self.tx.push(UiUpdate::RefreshState);
    }
}

impl ModuleToUiBridge for UiBridge {
    fn update_modulated_input(&mut self, input: Input, channel_idx: usize, value: Sample) {
        let _ = self.tx.push(UiUpdate::ModulatedInput {
            input,
            channel: channel_idx as u8,
            value,
        });
    }

    fn update_output(&mut self, channel_idx: usize, value: Sample) {
        let _ = self.tx.push(UiUpdate::Output {
            channel: channel_idx as u8,
            value,
        });
    }
}
