use triple_buffer::triple_buffer;

use addi_dsp::segments::Segments;
use addi_engine::mod_map::{ModMapAudioEnd, ModMapLinks, ModMapUiEnd, UiEvent};
use addi_engine::{Input, Smp, StereoSample, UI_TO_AUDIO_RING_CAPACITY};

pub struct UiEnd {
    tx: rtrb::Producer<UiEvent>,
    segments: triple_buffer::Input<Segments>,
}

impl UiEnd {
    pub fn set_param(&mut self, input: Input, value: StereoSample) -> bool {
        self.tx.push(UiEvent::InputParam { input, value }).is_ok()
    }

    pub fn set_segments(&mut self, segments: Segments) -> bool {
        self.segments.write(segments);
        self.tx.push(UiEvent::SegmentsUpdated).is_ok()
    }

    pub fn set_bipolar(&mut self, value: bool) -> bool {
        self.tx.push(UiEvent::Bipolar(value)).is_ok()
    }

    pub fn set_smooth(&mut self, value: Smp) -> bool {
        self.tx.push(UiEvent::Smooth(value)).is_ok()
    }
}

pub struct AudioEnd {
    rx: rtrb::Consumer<UiEvent>,
    segments: triple_buffer::Output<Segments>,
}

impl AudioEnd {
    pub fn pop_event(&mut self) -> Option<UiEvent> {
        self.rx.pop().ok()
    }

    pub fn read_segments(&mut self) -> &Segments {
        self.segments.read()
    }
}

pub fn make_link_pair() -> (AudioEnd, UiEnd) {
    let (to_audio_tx, from_ui_rx) = rtrb::RingBuffer::<UiEvent>::new(UI_TO_AUDIO_RING_CAPACITY);
    let (segments_input, segments_output) =
        triple_buffer(&Segments::from_points(&[(0.0, 0.0), (1.0, 1.0)], false));

    (
        AudioEnd {
            rx: from_ui_rx,
            segments: segments_output,
        },
        UiEnd {
            tx: to_audio_tx,
            segments: segments_input,
        },
    )
}

impl ModMapAudioEnd for AudioEnd {
    fn pop_event(&mut self) -> Option<UiEvent> {
        AudioEnd::pop_event(self)
    }

    fn read_segments(&mut self) -> &Segments {
        AudioEnd::read_segments(self)
    }
}

impl ModMapUiEnd for UiEnd {
    fn set_param(&mut self, input: Input, value: StereoSample) -> bool {
        UiEnd::set_param(self, input, value)
    }

    fn set_segments(&mut self, segments: Segments) -> bool {
        UiEnd::set_segments(self, segments)
    }

    fn set_bipolar(&mut self, value: bool) -> bool {
        UiEnd::set_bipolar(self, value)
    }

    fn set_smooth(&mut self, value: Smp) -> bool {
        UiEnd::set_smooth(self, value)
    }
}

pub struct Links;

impl ModMapLinks for Links {
    type AudioEnd = AudioEnd;
    type UiEnd = UiEnd;
    type EngineEnd = crate::links::engine::AudioEnd;

    fn create_link_pair() -> (Self::AudioEnd, Option<Self::UiEnd>) {
        let (audio_end, ui_end) = make_link_pair();
        (audio_end, Some(ui_end))
    }
}
