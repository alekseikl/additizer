use triple_buffer::triple_buffer;

use addi_dsp::filters::svf::SvfType;
use addi_engine::{Input, Sample, StereoSample, UI_TO_AUDIO_RING_CAPACITY};

use addi_engine::svf::{SvfAudioEnd, SvfLinks, SvfUiEnd, UiEvent};

pub struct UiEnd {
    tx: rtrb::Producer<UiEvent>,
    pitch: triple_buffer::Output<Sample>,
}

impl UiEnd {
    pub fn pitch(&mut self) -> Sample {
        *self.pitch.read()
    }

    pub fn set_param(&mut self, input: Input, value: StereoSample) -> bool {
        self.tx.push(UiEvent::InputParam { input, value }).is_ok()
    }

    pub fn set_filter_type(&mut self, filter_type: SvfType) -> bool {
        self.tx.push(UiEvent::FilterType(filter_type)).is_ok()
    }

    pub fn set_keytrack(&mut self, value: Sample) -> bool {
        self.tx.push(UiEvent::Keytrack(value)).is_ok()
    }
}

pub struct AudioEnd {
    rx: rtrb::Consumer<UiEvent>,
    pitch: triple_buffer::Input<Sample>,
}

impl AudioEnd {
    pub fn pop_event(&mut self) -> Option<UiEvent> {
        self.rx.pop().ok()
    }

    pub fn update_pitch(&mut self, pitch: Sample) {
        self.pitch.write(pitch);
    }
}

pub fn make_link_pair() -> (AudioEnd, UiEnd) {
    let (to_audio_tx, from_ui_rx) = rtrb::RingBuffer::<UiEvent>::new(UI_TO_AUDIO_RING_CAPACITY);
    let (pitch_input, pitch_output) = triple_buffer(&addi_dsp::C4_PITCH);

    (
        AudioEnd {
            rx: from_ui_rx,
            pitch: pitch_input,
        },
        UiEnd {
            tx: to_audio_tx,
            pitch: pitch_output,
        },
    )
}

impl SvfAudioEnd for AudioEnd {
    fn pop_event(&mut self) -> Option<UiEvent> {
        AudioEnd::pop_event(self)
    }
    fn update_pitch(&mut self, pitch: Sample) {
        AudioEnd::update_pitch(self, pitch)
    }
}

impl SvfUiEnd for UiEnd {
    fn pitch(&mut self) -> Sample {
        UiEnd::pitch(self)
    }
    fn set_param(&mut self, input: Input, value: StereoSample) -> bool {
        UiEnd::set_param(self, input, value)
    }
    fn set_filter_type(&mut self, filter_type: SvfType) -> bool {
        UiEnd::set_filter_type(self, filter_type)
    }
    fn set_keytrack(&mut self, value: Sample) -> bool {
        UiEnd::set_keytrack(self, value)
    }
}

pub struct Links;

impl SvfLinks for Links {
    type AudioEnd = AudioEnd;
    type UiEnd = UiEnd;
    type EngineEnd = crate::links::engine::AudioEnd;

    fn create_link_pair() -> (Self::AudioEnd, Option<Self::UiEnd>) {
        let (audio_end, ui_end) = make_link_pair();
        (audio_end, Some(ui_end))
    }
}
