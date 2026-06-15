use crate::synth_engine::{Input, Sample, StereoSample};

pub enum UiEvent {
    InputParam { input: Input, value: StereoSample },
    Smooth(StereoSample),
    AttackCurvature(Sample),
    DecayCurvature(Sample),
    ReleaseCurvature(Sample),
    KeepVoiceAlive(bool),
}

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

    pub fn set_smooth(&mut self, value: StereoSample) -> bool {
        self.tx.push(UiEvent::Smooth(value)).is_ok()
    }

    pub fn set_attack_curvature(&mut self, value: Sample) -> bool {
        self.tx.push(UiEvent::AttackCurvature(value)).is_ok()
    }

    pub fn set_decay_curvature(&mut self, value: Sample) -> bool {
        self.tx.push(UiEvent::DecayCurvature(value)).is_ok()
    }

    pub fn set_release_curvature(&mut self, value: Sample) -> bool {
        self.tx.push(UiEvent::ReleaseCurvature(value)).is_ok()
    }

    pub fn set_keep_voice_alive(&mut self, value: bool) -> bool {
        self.tx.push(UiEvent::KeepVoiceAlive(value)).is_ok()
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

pub fn create_link_pair() -> (AudioEnd, UiEnd) {
    let (to_audio_tx, from_ui_rx) = rtrb::RingBuffer::<UiEvent>::new(128);

    (AudioEnd::new(from_ui_rx), UiEnd::new(to_audio_tx))
}
