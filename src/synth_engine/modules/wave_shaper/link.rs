use crate::synth_engine::{Input, StereoSample};

use super::config::ShaperType;

pub enum UiEvent {
    InputParam { input: Input, value: StereoSample },
    ShaperType(ShaperType),
}

pub struct UiEnd {
    tx: rtrb::Producer<UiEvent>,
}

impl UiEnd {
    pub fn new(tx: rtrb::Producer<UiEvent>) -> Self {
        Self { tx }
    }

    pub fn set_param(&mut self, input: Input, value: StereoSample) -> bool {
        self.tx.push(UiEvent::InputParam { input, value }).is_ok()
    }

    pub fn set_shaper_type(&mut self, shaper_type: ShaperType) -> bool {
        self.tx.push(UiEvent::ShaperType(shaper_type)).is_ok()
    }
}

pub struct AudioEnd {
    rx: rtrb::Consumer<UiEvent>,
}

impl AudioEnd {
    pub fn new(rx: rtrb::Consumer<UiEvent>) -> Self {
        Self { rx }
    }

    pub fn pop_event(&mut self) -> Option<UiEvent> {
        self.rx.pop().ok()
    }
}

pub fn create_link_pair() -> (AudioEnd, UiEnd) {
    let (to_audio_tx, from_ui_rx) = rtrb::RingBuffer::<UiEvent>::new(128);

    (AudioEnd::new(from_ui_rx), UiEnd::new(to_audio_tx))
}
