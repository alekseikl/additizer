use crate::synth_engine::{Input, ModuleId, ModuleInput, Sample, StereoSample};

pub struct UiState {
    pub voices: usize,
    pub legato: bool,
    pub block_size: usize,
    pub voice_kill_time: Sample,
    pub oversampling: bool,
    pub stereo_spectrum: bool,
}

pub enum UIEvent {
    InputParam {
        module_id: ModuleId,
        input: Input,
        value: StereoSample,
    },
    LinkAmount {
        src: ModuleId,
        dst: ModuleInput,
        amount: StereoSample,
    },
    Voices(usize),
    Legato(bool),
    BlockSize(usize),
    VoiceKillTime(Sample),
    Oversampling(bool),
    StereoSpectrum(bool),
}

pub enum UiUpdate {
    ModulatedInput {
        module_id: ModuleId,
        input: Input,
        channel: u8,
        value: Sample,
    },
    Output {
        module_id: ModuleId,
        channel: u8,
        value: Sample,
    },
    VoicesStatus {
        has_active_voices: bool,
        waiting_notes: u8,
        playing: u8,
        releasing: u8,
        killing: u8,
    },
}
