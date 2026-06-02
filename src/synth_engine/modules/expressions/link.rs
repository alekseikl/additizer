use crate::synth_engine::{Expression, Sample};

pub enum UiEvent {
    Expression(Expression),
    UseReleaseVelocity(bool),
    Smooth(Sample),
}

pub struct UiEnd {
    tx: rtrb::Producer<UiEvent>,
}

impl UiEnd {
    pub fn new(tx: rtrb::Producer<UiEvent>) -> Self {
        Self { tx }
    }

    pub fn set_expression(&mut self, expression: Expression) -> bool {
        self.tx.push(UiEvent::Expression(expression)).is_ok()
    }

    pub fn set_use_release_velocity(&mut self, value: bool) -> bool {
        self.tx.push(UiEvent::UseReleaseVelocity(value)).is_ok()
    }

    pub fn set_smooth(&mut self, value: Sample) -> bool {
        self.tx.push(UiEvent::Smooth(value)).is_ok()
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
