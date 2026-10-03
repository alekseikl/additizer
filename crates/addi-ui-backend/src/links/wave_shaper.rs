use addi_engine::{Input, StereoSample, UI_TO_AUDIO_RING_CAPACITY};

use addi_engine::wave_shaper::ShaperType;

use addi_engine::wave_shaper::{UiEvent, WaveShaperAudioEnd, WaveShaperLinks, WaveShaperUiEnd};

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

pub fn make_link_pair() -> (AudioEnd, UiEnd) {
    let (to_audio_tx, from_ui_rx) = rtrb::RingBuffer::<UiEvent>::new(UI_TO_AUDIO_RING_CAPACITY);

    (AudioEnd::new(from_ui_rx), UiEnd::new(to_audio_tx))
}

impl WaveShaperAudioEnd for AudioEnd {
    fn pop_event(&mut self) -> Option<UiEvent> {
        AudioEnd::pop_event(self)
    }
}

impl WaveShaperUiEnd for UiEnd {
    fn set_param(&mut self, input: Input, value: StereoSample) -> bool {
        UiEnd::set_param(self, input, value)
    }
    fn set_shaper_type(&mut self, shaper_type: ShaperType) -> bool {
        UiEnd::set_shaper_type(self, shaper_type)
    }
}

pub struct Links;

impl WaveShaperLinks for Links {
    type AudioEnd = AudioEnd;
    type UiEnd = UiEnd;
    type EngineEnd = crate::links::engine::AudioEnd;

    fn create_link_pair() -> (Self::AudioEnd, Option<Self::UiEnd>) {
        let (audio_end, ui_end) = make_link_pair();
        (audio_end, Some(ui_end))
    }
}
