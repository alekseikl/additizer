use crate::synth_engine::{
    ModuleId, Sample, SmoothedSampleParams,
    engine_io::EngineAudioEnd,
    routing::{
        AudioRouterType, ControlRouterType, OutputRouterType, OutputsArena, RouterFactory,
        SpectralRouterType,
    },
    voices_handler::PlayingVoice,
};

pub struct ProcessParams<'a> {
    pub trigger_stage: bool,
    pub has_triggered_voices: bool,
    pub samples: usize,
    pub sample_rate: Sample,
    pub needs_update_ui: bool,
    pub smooth_params: SmoothedSampleParams,
    pub active_voices: &'a [PlayingVoice],
}

pub struct ProcessContext<'c, A: EngineAudioEnd> {
    pub outputs_arena: &'c mut OutputsArena,
    pub audio_end: &'c mut A,
    pub params: ProcessParams<'c>,
}

#[derive(Clone, Copy)]
pub struct VoiceTarget {
    pub channel_idx: usize,
    pub voice_idx: usize,
    pub note: u8,
    pub triggered: Option<usize>,
    pub is_last: bool,
}

impl VoiceTarget {
    pub fn new(channel_idx: usize, voice: &PlayingVoice, seq_idx: usize) -> Self {
        VoiceTarget {
            channel_idx,
            voice_idx: voice.voice_idx(),
            note: voice.note(),
            triggered: voice.triggered(),
            is_last: seq_idx == 0,
        }
    }

    pub fn note_pitch(&self) -> Sample {
        crate::synth_engine::note_to_pitch(self.note as Sample)
    }
}

impl<'c, A: EngineAudioEnd> ProcessContext<'c, A> {
    pub fn audio<'f>(
        &'f mut self,
        module_id: ModuleId,
        output_slot: usize,
    ) -> RouterFactory<'f, 'c, AudioRouterType, A>
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
    ) -> RouterFactory<'f, 'c, ControlRouterType, A>
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
    ) -> RouterFactory<'f, 'c, SpectralRouterType, A>
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
    ) -> RouterFactory<'f, 'c, OutputRouterType, A>
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
