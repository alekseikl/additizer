use crate::synth_engine::{
    Input, ModuleId, InputId, Sample, StereoSample, ui_bridge::VoicesStatus,
    voices_handler::VoicesHandlerUiState,
};

pub enum UiEvent {
    LinkAmount {
        src: ModuleId,
        dst: InputId,
        amount: StereoSample,
    },
    Voices(usize),
    Legato(bool),
    BlockSize(usize),
    VoiceKillTime(Sample),
    Oversampling(bool),
    StereoSpectrum(bool),
    OutputGain(StereoSample),
}

pub enum UiUpdate {
    ModulatedInput {
        module_id: ModuleId,
        input: Input,
        channel: u8,
        value: Sample,
    },
    VoicesStatus(VoicesStatus),
}

pub struct AudioEnd {
    rx: rtrb::Consumer<UiEvent>,
    tx: rtrb::Producer<UiUpdate>,
}

impl AudioEnd {
    pub fn new(rx: rtrb::Consumer<UiEvent>, tx: rtrb::Producer<UiUpdate>) -> Self {
        Self { rx, tx }
    }

    pub fn update_modulated_input(
        &mut self,
        module_id: ModuleId,
        input: Input,
        channel: u8,
        value: Sample,
    ) -> bool {
        self.tx
            .push(UiUpdate::ModulatedInput {
                module_id,
                input,
                channel,
                value,
            })
            .is_ok()
    }

    pub fn update_voices_status(&mut self, d: &VoicesHandlerUiState) -> bool {
        self.tx
            .push(UiUpdate::VoicesStatus(VoicesStatus {
                waiting_notes: d.waiting as u8,
                playing: d.playing as u8,
                releasing: d.releasing as u8,
                killing: d.killing as u8,
            }))
            .is_ok()
    }

    pub fn pop_event(&mut self) -> Option<UiEvent> {
        self.rx.pop().ok()
    }
}

pub struct UiEnd {
    rx: rtrb::Consumer<UiUpdate>,
    tx: rtrb::Producer<UiEvent>,
}

impl UiEnd {
    pub fn new(rx: rtrb::Consumer<UiUpdate>, tx: rtrb::Producer<UiEvent>) -> Self {
        Self { rx, tx }
    }

    pub fn set_link_amount(
        &mut self,
        src: ModuleId,
        dst: InputId,
        amount: StereoSample,
    ) -> bool {
        self.tx
            .push(UiEvent::LinkAmount { src, dst, amount })
            .is_ok()
    }

    pub fn set_voices(&mut self, voices: usize) -> bool {
        self.tx.push(UiEvent::Voices(voices)).is_ok()
    }

    pub fn set_legato(&mut self, legato: bool) -> bool {
        self.tx.push(UiEvent::Legato(legato)).is_ok()
    }

    pub fn set_block_size(&mut self, block_size: usize) -> bool {
        self.tx.push(UiEvent::BlockSize(block_size)).is_ok()
    }

    pub fn set_voice_kill_time(&mut self, voice_kill_time: Sample) -> bool {
        self.tx
            .push(UiEvent::VoiceKillTime(voice_kill_time))
            .is_ok()
    }

    pub fn set_oversampling(&mut self, oversampling: bool) -> bool {
        self.tx.push(UiEvent::Oversampling(oversampling)).is_ok()
    }

    pub fn set_stereo_spectrum(&mut self, stereo_spectrum: bool) -> bool {
        self.tx
            .push(UiEvent::StereoSpectrum(stereo_spectrum))
            .is_ok()
    }

    pub fn set_output_gain(&mut self, output_gain: StereoSample) -> bool {
        self.tx.push(UiEvent::OutputGain(output_gain)).is_ok()
    }

    pub fn pop_update(&mut self) -> Option<UiUpdate> {
        self.rx.pop().ok()
    }
}

pub fn create_link_pair() -> (AudioEnd, UiEnd) {
    let (to_audio_tx, from_ui_rx) = rtrb::RingBuffer::<UiEvent>::new(512);
    let (to_ui_tx, from_audio_rx) = rtrb::RingBuffer::<UiUpdate>::new(128);

    (
        AudioEnd::new(from_ui_rx, to_ui_tx),
        UiEnd::new(from_audio_rx, to_audio_tx),
    )
}
