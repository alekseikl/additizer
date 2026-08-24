use crate::synth_engine::{
    Input, StereoSample, UI_TO_AUDIO_RING_CAPACITY, filters::spectral_filter::FilterType,
};

pub enum UiEvent {
    InputParam { input: Input, value: StereoSample },
    FilterType(FilterType),
    LinearPhase(bool),
    QLimitTo(StereoSample),
    QLimitCurve(StereoSample),
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

    pub fn set_filter_type(&mut self, filter_type: FilterType) -> bool {
        self.tx.push(UiEvent::FilterType(filter_type)).is_ok()
    }

    pub fn set_linear_phase(&mut self, value: bool) -> bool {
        self.tx.push(UiEvent::LinearPhase(value)).is_ok()
    }

    pub fn set_q_limit_to(&mut self, value: StereoSample) -> bool {
        self.tx.push(UiEvent::QLimitTo(value)).is_ok()
    }

    pub fn set_q_limit_curve(&mut self, value: StereoSample) -> bool {
        self.tx.push(UiEvent::QLimitCurve(value)).is_ok()
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
    let (to_audio_tx, from_ui_rx) = rtrb::RingBuffer::<UiEvent>::new(UI_TO_AUDIO_RING_CAPACITY);

    (AudioEnd::new(from_ui_rx), UiEnd::new(to_audio_tx))
}
