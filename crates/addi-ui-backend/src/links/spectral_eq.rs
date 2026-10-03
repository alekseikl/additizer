use triple_buffer::triple_buffer;

use addi_engine::{Input, Sample, StereoSample, UI_TO_AUDIO_RING_CAPACITY};

use addi_engine::spectral_eq::EqFilter;

use addi_engine::spectral_eq::{SpectralEqAudioEnd, SpectralEqLinks, SpectralEqUiEnd, UiEvent};

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

    pub fn set_linear_phase(&mut self, value: bool) -> bool {
        self.tx.push(UiEvent::LinearPhase(value)).is_ok()
    }

    pub fn set_keytrack(&mut self, value: Sample) -> bool {
        self.tx.push(UiEvent::Keytrack(value)).is_ok()
    }

    pub fn set_q_cutoff(&mut self, value: StereoSample) -> bool {
        self.tx.push(UiEvent::QCutoff(value)).is_ok()
    }

    pub fn set_q_rolloff(&mut self, value: StereoSample) -> bool {
        self.tx.push(UiEvent::QRolloff(value)).is_ok()
    }

    pub fn set_filter(&mut self, index: u8, filter: EqFilter) -> bool {
        self.tx.push(UiEvent::SetFilter { index, filter }).is_ok()
    }

    pub fn add_filter(&mut self, filter: EqFilter) -> bool {
        self.tx.push(UiEvent::AddFilter(filter)).is_ok()
    }

    pub fn remove_filter(&mut self, index: u8) -> bool {
        self.tx.push(UiEvent::RemoveFilter(index)).is_ok()
    }

    pub fn move_filter(&mut self, from: u8, to: u8) -> bool {
        self.tx.push(UiEvent::MoveFilter { from, to }).is_ok()
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

impl SpectralEqAudioEnd for AudioEnd {
    fn pop_event(&mut self) -> Option<UiEvent> {
        AudioEnd::pop_event(self)
    }
    fn update_pitch(&mut self, pitch: Sample) {
        AudioEnd::update_pitch(self, pitch)
    }
}

impl SpectralEqUiEnd for UiEnd {
    fn pitch(&mut self) -> Sample {
        UiEnd::pitch(self)
    }
    fn set_param(&mut self, input: Input, value: StereoSample) -> bool {
        UiEnd::set_param(self, input, value)
    }
    fn set_linear_phase(&mut self, value: bool) -> bool {
        UiEnd::set_linear_phase(self, value)
    }
    fn set_keytrack(&mut self, value: Sample) -> bool {
        UiEnd::set_keytrack(self, value)
    }
    fn set_q_cutoff(&mut self, value: StereoSample) -> bool {
        UiEnd::set_q_cutoff(self, value)
    }
    fn set_q_rolloff(&mut self, value: StereoSample) -> bool {
        UiEnd::set_q_rolloff(self, value)
    }
    fn set_filter(&mut self, index: u8, filter: EqFilter) -> bool {
        UiEnd::set_filter(self, index, filter)
    }
    fn add_filter(&mut self, filter: EqFilter) -> bool {
        UiEnd::add_filter(self, filter)
    }
    fn remove_filter(&mut self, index: u8) -> bool {
        UiEnd::remove_filter(self, index)
    }
    fn move_filter(&mut self, from: u8, to: u8) -> bool {
        UiEnd::move_filter(self, from, to)
    }
}

pub struct Links;

impl SpectralEqLinks for Links {
    type AudioEnd = AudioEnd;
    type UiEnd = UiEnd;
    type EngineEnd = crate::links::engine::AudioEnd;

    fn create_link_pair() -> (Self::AudioEnd, Option<Self::UiEnd>) {
        let (audio_end, ui_end) = make_link_pair();
        (audio_end, Some(ui_end))
    }
}
