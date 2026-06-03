use crate::synth_engine::StereoSample;

use super::{FilterParams, SetParams};

pub enum UiEvent {
    SetHarmonic {
        harmonic_number: usize,
        gain: StereoSample,
    },
    SetSelected(SetParams),
    ApplyFilter(FilterParams),
}

pub enum UiUpdate {
    RefreshState,
}

pub struct UiEnd {
    rx: rtrb::Consumer<UiUpdate>,
    tx: rtrb::Producer<UiEvent>,
}

impl UiEnd {
    pub fn new(rx: rtrb::Consumer<UiUpdate>, tx: rtrb::Producer<UiEvent>) -> Self {
        Self { rx, tx }
    }

    pub fn set_harmonic(&mut self, harmonic_number: usize, gain: StereoSample) -> bool {
        self.tx
            .push(UiEvent::SetHarmonic {
                harmonic_number,
                gain,
            })
            .is_ok()
    }

    pub fn set_selected(&mut self, params: SetParams) -> bool {
        self.tx.push(UiEvent::SetSelected(params)).is_ok()
    }

    pub fn apply_filter(&mut self, params: FilterParams) -> bool {
        self.tx.push(UiEvent::ApplyFilter(params)).is_ok()
    }

    pub fn pop_update(&mut self) -> Option<UiUpdate> {
        self.rx.pop().ok()
    }
}

pub struct AudioEnd {
    rx: rtrb::Consumer<UiEvent>,
    tx: rtrb::Producer<UiUpdate>,
}

impl AudioEnd {
    pub fn new(rx: rtrb::Consumer<UiEvent>, tx: rtrb::Producer<UiUpdate>) -> Self {
        Self { rx, tx }
    }

    pub fn pop_event(&mut self) -> Option<UiEvent> {
        self.rx.pop().ok()
    }

    pub fn push_refresh_state(&mut self) -> bool {
        self.tx.push(UiUpdate::RefreshState).is_ok()
    }
}

pub fn create_link_pair() -> (AudioEnd, UiEnd) {
    let (to_audio_tx, from_ui_rx) = rtrb::RingBuffer::<UiEvent>::new(128);
    let (to_ui_tx, from_audio_rx) = rtrb::RingBuffer::<UiUpdate>::new(128);

    (
        AudioEnd::new(from_ui_rx, to_ui_tx),
        UiEnd::new(from_audio_rx, to_audio_tx),
    )
}
