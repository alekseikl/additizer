use triple_buffer::triple_buffer;

use crate::synth_engine::{
    Input, Sample, StereoSample,
    oscillator::{PhasesDst, WAVEFORM_SIZE},
};

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
    RefreshState,
}

pub const DISPLAY_WAVEFORM_SIZE: usize = WAVEFORM_SIZE + 1;

pub type DisplayWaveform = [Sample; DISPLAY_WAVEFORM_SIZE];

pub struct UiEnd {
    rx: rtrb::Consumer<UiUpdate>,
    tx: rtrb::Producer<UiEvent>,
    waveform: triple_buffer::Output<Box<DisplayWaveform>>,
}

impl UiEnd {
    pub fn get_waveform(&mut self) -> &DisplayWaveform {
        self.waveform.update();
        self.waveform.output_buffer()
    }

    pub fn set_param(&mut self, input: Input, value: StereoSample) -> bool {
        self.tx.push(UiEvent::InputParam { input, value }).is_ok()
    }

    pub fn set_unison(&mut self, unison: usize) -> bool {
        self.tx.push(UiEvent::Unison(unison)).is_ok()
    }

    pub fn set_steal_phase(&mut self, steal_phase: bool) -> bool {
        self.tx.push(UiEvent::StealPhase(steal_phase)).is_ok()
    }

    pub fn set_unison_initial_phase(&mut self, idx: usize, value: StereoSample) -> bool {
        self.tx
            .push(UiEvent::UnisonInitialPhase { idx, value })
            .is_ok()
    }

    pub fn set_unison_phase_shift(&mut self, idx: usize, value: StereoSample) -> bool {
        self.tx
            .push(UiEvent::UnisonPhaseShift { idx, value })
            .is_ok()
    }

    pub fn set_unison_phase_shift_to(&mut self, idx: usize, value: StereoSample) -> bool {
        self.tx
            .push(UiEvent::UnisonPhaseShiftTo { idx, value })
            .is_ok()
    }

    pub fn set_unison_gain(&mut self, idx: usize, value: StereoSample) -> bool {
        self.tx.push(UiEvent::UnisonGain { idx, value }).is_ok()
    }

    pub fn set_unison_gain_to(&mut self, idx: usize, value: StereoSample) -> bool {
        self.tx.push(UiEvent::UnisonGainTo { idx, value }).is_ok()
    }

    pub fn apply_unison_level_shape(
        &mut self,
        center: StereoSample,
        level: StereoSample,
        to: bool,
    ) -> bool {
        self.tx
            .push(UiEvent::ApplyUnisonLevelShape { center, level, to })
            .is_ok()
    }

    pub fn randomize_phases(
        &mut self,
        from: Sample,
        to: Sample,
        stereo_spread: Sample,
        dst: PhasesDst,
    ) -> bool {
        self.tx
            .push(UiEvent::RandomizePhases {
                from,
                to,
                stereo_spread,
                dst,
            })
            .is_ok()
    }

    pub fn pop_update(&mut self) -> Option<UiUpdate> {
        self.rx.pop().ok()
    }
}

pub struct AudioEnd {
    rx: rtrb::Consumer<UiEvent>,
    tx: rtrb::Producer<UiUpdate>,
    waveform: triple_buffer::Input<Box<DisplayWaveform>>,
}

impl AudioEnd {
    pub fn pop_event(&mut self) -> Option<UiEvent> {
        self.rx.pop().ok()
    }

    pub fn push_refresh_state(&mut self) -> bool {
        self.tx.push(UiUpdate::RefreshState).is_ok()
    }

    pub fn update_waveform(&mut self, wf: &[Sample]) {
        self.waveform.input_buffer_mut().copy_from_slice(wf);
        self.waveform.publish();
    }
}

pub fn create_link_pair() -> (AudioEnd, UiEnd) {
    let (to_audio_tx, from_ui_rx) = rtrb::RingBuffer::<UiEvent>::new(512);
    let (to_ui_tx, from_audio_rx) = rtrb::RingBuffer::<UiUpdate>::new(128);
    let (waveform_input, waveform_output) = triple_buffer(&Box::new([0.0; DISPLAY_WAVEFORM_SIZE]));

    (
        AudioEnd {
            rx: from_ui_rx,
            tx: to_ui_tx,
            waveform: waveform_input,
        },
        UiEnd {
            rx: from_audio_rx,
            tx: to_audio_tx,
            waveform: waveform_output,
        },
    )
}
