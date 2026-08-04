use triple_buffer::triple_buffer;

use crate::synth_engine::Sample;

pub enum UiEvent {
    SelectedParamIndex(usize),
    Smooth(Sample),
    SampleOnTrigger(bool),
}

pub struct UiEnd {
    tx: rtrb::Producer<UiEvent>,
    value: triple_buffer::Output<Sample>,
}

impl UiEnd {
    pub fn new(tx: rtrb::Producer<UiEvent>, value: triple_buffer::Output<Sample>) -> Self {
        Self { tx, value }
    }

    pub fn get_value(&mut self) -> Sample {
        *self.value.read()
    }

    pub fn select_param(&mut self, index: usize) -> bool {
        self.tx.push(UiEvent::SelectedParamIndex(index)).is_ok()
    }

    pub fn set_smooth(&mut self, value: Sample) -> bool {
        self.tx.push(UiEvent::Smooth(value)).is_ok()
    }

    pub fn set_sample_on_trigger(&mut self, value: bool) -> bool {
        self.tx.push(UiEvent::SampleOnTrigger(value)).is_ok()
    }
}

pub struct AudioEnd {
    rx: rtrb::Consumer<UiEvent>,
    value: triple_buffer::Input<Sample>,
}

impl AudioEnd {
    pub fn new(rx: rtrb::Consumer<UiEvent>, value: triple_buffer::Input<Sample>) -> Self {
        Self { rx, value }
    }

    pub fn pop_event(&mut self) -> Option<UiEvent> {
        self.rx.pop().ok()
    }

    pub fn update_value(&mut self, value: Sample) {
        self.value.write(value);
    }
}

pub fn create_link_pair() -> (AudioEnd, UiEnd) {
    let (to_audio_tx, from_ui_rx) = rtrb::RingBuffer::<UiEvent>::new(128);
    let (value_input, value_output) = triple_buffer(&0.0);

    (
        AudioEnd::new(from_ui_rx, value_input),
        UiEnd::new(to_audio_tx, value_output),
    )
}
