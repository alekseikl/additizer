use crate::synth_engine::{
    ModuleId, Sample, SmoothedSampleParams,
    routing::{
        AudioRouterType, ControlRouterType, OutputRouterType, OutputsArena, RouterFactory,
        SpectralRouterType,
    },
    ui_bridge::AudioEnd,
    voices_handler::PlayingVoice,
};

pub struct ProcessParams<'a> {
    pub trigger_stage: bool,
    pub has_triggered_voices: bool,
    pub samples: usize,
    pub sample_rate: Sample,
    pub needs_update_ui: bool,
    pub smooth_params: SmoothedSampleParams,
    pub spectrum_channels: usize,
    // Number of harmonics that set in UI. Without taking DC into account.
    pub bandwidth: usize,
    pub active_voices: &'a [PlayingVoice],
}

pub struct ProcessContext<'c> {
    pub outputs_arena: &'c mut OutputsArena,
    pub audio_end: &'c mut AudioEnd,
    pub params: ProcessParams<'c>,
}

#[derive(Clone, Copy)]
pub struct VoiceTarget {
    pub channel_idx: usize,
    pub voice_idx: usize,
    pub note_bandwidth: usize,
    pub triggered: Option<usize>,
    pub is_last: bool,
}

impl VoiceTarget {
    pub fn new(channel_idx: usize, voice: &PlayingVoice, seq_idx: usize) -> Self {
        VoiceTarget {
            channel_idx,
            voice_idx: voice.voice_idx(),
            note_bandwidth: voice.note_bandwidth(),
            triggered: voice.triggered(),
            is_last: seq_idx == 0,
        }
    }
}

impl<'c> ProcessContext<'c> {
    pub fn audio<'f>(
        &'f mut self,
        module_id: ModuleId,
        output_slot: usize,
    ) -> RouterFactory<'f, 'c, AudioRouterType>
    where
        'c: 'f,
    {
        RouterFactory {
            ctx: self,
            module_id,
            data_type: AudioRouterType {
                samples_slot: output_slot,
            },
        }
    }

    pub fn control<'f>(
        &'f mut self,
        module_id: ModuleId,
        output_slot: usize,
    ) -> RouterFactory<'f, 'c, ControlRouterType>
    where
        'c: 'f,
    {
        RouterFactory {
            ctx: self,
            module_id,
            data_type: ControlRouterType {
                samples_slot: output_slot,
            },
        }
    }

    pub fn spectral<'f>(
        &'f mut self,
        module_id: ModuleId,
        output_slot: usize,
    ) -> RouterFactory<'f, 'c, SpectralRouterType>
    where
        'c: 'f,
    {
        RouterFactory {
            ctx: self,
            module_id,
            data_type: SpectralRouterType {
                spectral_slot: output_slot,
            },
        }
    }

    pub fn for_output<'f>(
        &'f mut self,
        module_id: ModuleId,
    ) -> RouterFactory<'f, 'c, OutputRouterType>
    where
        'c: 'f,
    {
        RouterFactory {
            ctx: self,
            module_id,
            data_type: OutputRouterType,
        }
    }
}
