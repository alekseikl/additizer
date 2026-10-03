use triple_buffer::triple_buffer;

use addi_engine::{Input, Sample, StereoSample, UI_TO_AUDIO_RING_CAPACITY};

use addi_engine::pitch::{PitchAudioEnd, PitchLinks, PitchUiEnd, UiEvent};

pub struct UiEnd {
    tx: rtrb::Producer<UiEvent>,
    pitch: triple_buffer::Output<Sample>,
}

impl UiEnd {
    pub fn get_pitch(&mut self) -> Sample {
        *self.pitch.read()
    }

    pub fn set_param(&mut self, input: Input, value: StereoSample) -> bool {
        self.tx.push(UiEvent::InputParam { input, value }).is_ok()
    }

    pub fn set_keytrack(&mut self, value: bool) -> bool {
        self.tx.push(UiEvent::Keytrack(value)).is_ok()
    }

    pub fn set_glide_always(&mut self, value: bool) -> bool {
        self.tx.push(UiEvent::GlideAlways(value)).is_ok()
    }

    pub fn set_glide_per_octave(&mut self, value: bool) -> bool {
        self.tx.push(UiEvent::GlidePerOctave(value)).is_ok()
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
    let (pitch_input, pitch_output) = triple_buffer(&0.0);

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

impl PitchAudioEnd for AudioEnd {
    fn pop_event(&mut self) -> Option<UiEvent> {
        AudioEnd::pop_event(self)
    }
    fn update_pitch(&mut self, pitch: Sample) {
        AudioEnd::update_pitch(self, pitch)
    }
}

impl PitchUiEnd for UiEnd {
    fn get_pitch(&mut self) -> Sample {
        UiEnd::get_pitch(self)
    }
    fn set_param(&mut self, input: Input, value: StereoSample) -> bool {
        UiEnd::set_param(self, input, value)
    }
    fn set_keytrack(&mut self, value: bool) -> bool {
        UiEnd::set_keytrack(self, value)
    }
    fn set_glide_always(&mut self, value: bool) -> bool {
        UiEnd::set_glide_always(self, value)
    }
    fn set_glide_per_octave(&mut self, value: bool) -> bool {
        UiEnd::set_glide_per_octave(self, value)
    }
}

pub struct Links;

impl PitchLinks for Links {
    type AudioEnd = AudioEnd;
    type UiEnd = UiEnd;
    type EngineEnd = crate::links::engine::AudioEnd;

    fn create_link_pair() -> (Self::AudioEnd, Option<Self::UiEnd>) {
        let (audio_end, ui_end) = make_link_pair();
        (audio_end, Some(ui_end))
    }
}
