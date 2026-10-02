use triple_buffer::triple_buffer;

use crate::synth_engine::{Input, Sample, StereoSample, UI_TO_AUDIO_RING_CAPACITY};

use crate::synth_engine::lfo::LfoShape;

use crate::synth_engine::lfo::{LfoAudioEnd, LfoLinks, LfoUiEnd, UiEvent};

pub struct UiEnd {
    tx: rtrb::Producer<UiEvent>,
    phase: triple_buffer::Output<Sample>,
}

impl UiEnd {
    pub fn get_phase(&mut self) -> Sample {
        *self.phase.read()
    }

    pub fn set_param(&mut self, input: Input, value: StereoSample) -> bool {
        self.tx.push(UiEvent::InputParam { input, value }).is_ok()
    }

    pub fn set_shape(&mut self, shape: LfoShape) -> bool {
        self.tx.push(UiEvent::Shape(shape)).is_ok()
    }

    pub fn set_bipolar(&mut self, value: bool) -> bool {
        self.tx.push(UiEvent::Bipolar(value)).is_ok()
    }

    pub fn set_steal_phase(&mut self, value: bool) -> bool {
        self.tx.push(UiEvent::StealPhase(value)).is_ok()
    }

    pub fn set_smooth_time(&mut self, value: StereoSample) -> bool {
        self.tx.push(UiEvent::SmoothTime(value)).is_ok()
    }
}

pub struct AudioEnd {
    rx: rtrb::Consumer<UiEvent>,
    phase: triple_buffer::Input<Sample>,
}

impl AudioEnd {
    pub fn pop_event(&mut self) -> Option<UiEvent> {
        self.rx.pop().ok()
    }

    pub fn update_phase(&mut self, phase: Sample) {
        self.phase.write(phase);
    }
}

pub fn make_link_pair() -> (AudioEnd, UiEnd) {
    let (to_audio_tx, from_ui_rx) = rtrb::RingBuffer::<UiEvent>::new(UI_TO_AUDIO_RING_CAPACITY);
    let (phase_input, phase_output) = triple_buffer(&0.0);

    (
        AudioEnd {
            rx: from_ui_rx,
            phase: phase_input,
        },
        UiEnd {
            tx: to_audio_tx,
            phase: phase_output,
        },
    )
}

impl LfoAudioEnd for AudioEnd {
    fn pop_event(&mut self) -> Option<UiEvent> {
        AudioEnd::pop_event(self)
    }
    fn update_phase(&mut self, phase: Sample) {
        AudioEnd::update_phase(self, phase)
    }
}

impl LfoUiEnd for UiEnd {
    fn get_phase(&mut self) -> Sample {
        UiEnd::get_phase(self)
    }
    fn set_param(&mut self, input: Input, value: StereoSample) -> bool {
        UiEnd::set_param(self, input, value)
    }
    fn set_shape(&mut self, shape: LfoShape) -> bool {
        UiEnd::set_shape(self, shape)
    }
    fn set_bipolar(&mut self, value: bool) -> bool {
        UiEnd::set_bipolar(self, value)
    }
    fn set_steal_phase(&mut self, value: bool) -> bool {
        UiEnd::set_steal_phase(self, value)
    }
    fn set_smooth_time(&mut self, value: StereoSample) -> bool {
        UiEnd::set_smooth_time(self, value)
    }
}

pub struct Links;

impl LfoLinks for Links {
    type AudioEnd = AudioEnd;
    type UiEnd = UiEnd;
    type EngineEnd = crate::links::engine::AudioEnd;

    fn create_link_pair() -> (Self::AudioEnd, Self::UiEnd) {
        make_link_pair()
    }
}
