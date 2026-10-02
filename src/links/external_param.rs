use triple_buffer::triple_buffer;

use crate::synth_engine::{Sample, UI_TO_AUDIO_RING_CAPACITY};

use crate::synth_engine::external_param::{
    ExternalParamAudioEnd, ExternalParamLinks, ExternalParamUiEnd, UiEvent,
};

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

    pub fn set_make_bipolar(&mut self, value: bool) -> bool {
        self.tx.push(UiEvent::MakeBipolar(value)).is_ok()
    }

    pub fn set_polyphonic(&mut self, value: bool) -> bool {
        self.tx.push(UiEvent::Polyphonic(value)).is_ok()
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

pub fn make_link_pair() -> (AudioEnd, UiEnd) {
    let (to_audio_tx, from_ui_rx) = rtrb::RingBuffer::<UiEvent>::new(UI_TO_AUDIO_RING_CAPACITY);
    let (value_input, value_output) = triple_buffer(&0.0);

    (
        AudioEnd::new(from_ui_rx, value_input),
        UiEnd::new(to_audio_tx, value_output),
    )
}

impl ExternalParamAudioEnd for AudioEnd {
    fn pop_event(&mut self) -> Option<UiEvent> {
        AudioEnd::pop_event(self)
    }
    fn update_value(&mut self, value: Sample) {
        AudioEnd::update_value(self, value)
    }
}

impl ExternalParamUiEnd for UiEnd {
    fn get_value(&mut self) -> Sample {
        UiEnd::get_value(self)
    }
    fn select_param(&mut self, index: usize) -> bool {
        UiEnd::select_param(self, index)
    }
    fn set_smooth(&mut self, value: Sample) -> bool {
        UiEnd::set_smooth(self, value)
    }
    fn set_sample_on_trigger(&mut self, value: bool) -> bool {
        UiEnd::set_sample_on_trigger(self, value)
    }
    fn set_make_bipolar(&mut self, value: bool) -> bool {
        UiEnd::set_make_bipolar(self, value)
    }
    fn set_polyphonic(&mut self, value: bool) -> bool {
        UiEnd::set_polyphonic(self, value)
    }
}

pub struct Links;

impl ExternalParamLinks for Links {
    type AudioEnd = AudioEnd;
    type UiEnd = UiEnd;
    type EngineEnd = crate::links::engine::AudioEnd;

    fn create_link_pair() -> (Self::AudioEnd, Self::UiEnd) {
        make_link_pair()
    }
}
