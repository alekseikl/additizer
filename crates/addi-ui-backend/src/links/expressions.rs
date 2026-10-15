use triple_buffer::triple_buffer;

use addi_engine::{Expression, Smp, UI_TO_AUDIO_RING_CAPACITY};

use addi_engine::expressions::{ExpressionsAudioEnd, ExpressionsLinks, ExpressionsUiEnd, UiEvent};

pub struct UiEnd {
    tx: rtrb::Producer<UiEvent>,
    value: triple_buffer::Output<Smp>,
}

impl UiEnd {
    pub fn new(tx: rtrb::Producer<UiEvent>, value: triple_buffer::Output<Smp>) -> Self {
        Self { tx, value }
    }

    pub fn get_value(&mut self) -> Smp {
        *self.value.read()
    }

    pub fn set_expression(&mut self, expression: Expression) -> bool {
        self.tx.push(UiEvent::Expression(expression)).is_ok()
    }

    pub fn set_use_release_velocity(&mut self, value: bool) -> bool {
        self.tx.push(UiEvent::UseReleaseVelocity(value)).is_ok()
    }

    pub fn set_smooth(&mut self, value: Smp) -> bool {
        self.tx.push(UiEvent::Smooth(value)).is_ok()
    }
}

pub struct AudioEnd {
    rx: rtrb::Consumer<UiEvent>,
    value: triple_buffer::Input<Smp>,
}

impl AudioEnd {
    pub fn new(rx: rtrb::Consumer<UiEvent>, value: triple_buffer::Input<Smp>) -> Self {
        Self { rx, value }
    }

    pub fn pop_event(&mut self) -> Option<UiEvent> {
        self.rx.pop().ok()
    }

    pub fn update_value(&mut self, value: Smp) {
        self.value.write(value);
    }
}

pub fn make_link_pair() -> (AudioEnd, UiEnd) {
    let (to_audio_tx, from_ui_rx) = rtrb::RingBuffer::<UiEvent>::new(UI_TO_AUDIO_RING_CAPACITY);
    let (value_input, value_output) = triple_buffer(&0.0);

    (
        AudioEnd::new(from_ui_rx, value_input),
        UiEnd::new(to_audio_tx, value_output),
    )
}

impl ExpressionsAudioEnd for AudioEnd {
    fn pop_event(&mut self) -> Option<UiEvent> {
        AudioEnd::pop_event(self)
    }
    fn update_value(&mut self, value: Smp) {
        AudioEnd::update_value(self, value)
    }
}

impl ExpressionsUiEnd for UiEnd {
    fn get_value(&mut self) -> Smp {
        UiEnd::get_value(self)
    }
    fn set_expression(&mut self, expression: Expression) -> bool {
        UiEnd::set_expression(self, expression)
    }
    fn set_use_release_velocity(&mut self, value: bool) -> bool {
        UiEnd::set_use_release_velocity(self, value)
    }
    fn set_smooth(&mut self, value: Smp) -> bool {
        UiEnd::set_smooth(self, value)
    }
}

pub struct Links;

impl ExpressionsLinks for Links {
    type AudioEnd = AudioEnd;
    type UiEnd = UiEnd;
    type EngineEnd = crate::links::engine::AudioEnd;

    fn create_link_pair() -> (Self::AudioEnd, Option<Self::UiEnd>) {
        let (audio_end, ui_end) = make_link_pair();
        (audio_end, Some(ui_end))
    }
}
