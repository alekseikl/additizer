use triple_buffer::triple_buffer;

use crate::synth_engine::{Input, Sample, StereoSample, UI_TO_AUDIO_RING_CAPACITY};
use additizer_dsp::filters::spectral_filter::FilterType;

use crate::synth_engine::spectral_filter::{SpectralFilterAudioEnd, SpectralFilterLinks, SpectralFilterUiEnd, UiEvent};

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

    pub fn set_filter_type(&mut self, filter_type: FilterType) -> bool {
        self.tx.push(UiEvent::FilterType(filter_type)).is_ok()
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
    let (pitch_input, pitch_output) = triple_buffer(&crate::utils::C4_PITCH);

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

impl SpectralFilterAudioEnd for AudioEnd {
    fn pop_event(&mut self) -> Option<UiEvent> {
        AudioEnd::pop_event(self)
    }
    fn update_pitch(&mut self, pitch: Sample) {
        AudioEnd::update_pitch(self, pitch)
    }
}

impl SpectralFilterUiEnd for UiEnd {
    fn pitch(&mut self) -> Sample {
        UiEnd::pitch(self)
    }
    fn set_param(&mut self, input: Input, value: StereoSample) -> bool {
        UiEnd::set_param(self, input, value)
    }
    fn set_filter_type(&mut self, filter_type: FilterType) -> bool {
        UiEnd::set_filter_type(self, filter_type)
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
}

pub struct Links;

impl SpectralFilterLinks for Links {
    type AudioEnd = AudioEnd;
    type UiEnd = UiEnd;

    fn create_link_pair() -> (Self::AudioEnd, Self::UiEnd) {
        make_link_pair()
    }
}
