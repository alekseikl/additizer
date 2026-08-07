use crate::synth_engine::{
    ModuleId, Sample, SmoothedSampleParams,
    buffer::VoicesLayout,
    routing::{
        AudioRouterType, ControlRouterType, OutputRouterType, OutputsArena, RouterFactory,
        SamplesOutput, SpectralOutput, SpectralRouterType,
    },
    ui_bridge::AudioEnd,
    voices_handler::PlayingVoice,
};

pub struct ProcessParams<'a> {
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
    pub is_last: bool,
}

impl<'c> ProcessContext<'c> {
    pub fn for_audio<'f>(
        &'f mut self,
        module_id: ModuleId,
        output_slot: usize,
        f: impl FnOnce(&mut RouterFactory<'f, 'c, AudioRouterType>, &mut VoicesLayout<SamplesOutput>),
    ) where
        'c: 'f,
    {
        RouterFactory {
            ctx: self,
            module_id,
            data_type: AudioRouterType {
                samples_slot: output_slot,
            },
        }
        .with_output_slot(f);
    }

    pub fn for_audio2<'f>(
        &'f mut self,
        module_id: ModuleId,
        output_slot: usize,
        f: impl FnMut(
            &mut RouterFactory<'f, 'c, AudioRouterType>,
            VoiceTarget,
            &mut VoicesLayout<SamplesOutput>,
        ),
    ) where
        'c: 'f,
    {
        RouterFactory {
            ctx: self,
            module_id,
            data_type: AudioRouterType {
                samples_slot: output_slot,
            },
        }
        .with_output_slot2(f);
    }

    pub fn for_control<'f>(
        &'f mut self,
        module_id: ModuleId,
        output_slot: usize,
        f: impl FnOnce(&mut RouterFactory<'f, 'c, ControlRouterType>, &mut VoicesLayout<SamplesOutput>),
    ) where
        'c: 'f,
    {
        RouterFactory {
            ctx: self,
            module_id,
            data_type: ControlRouterType {
                samples_slot: output_slot,
            },
        }
        .with_output_slot(f);
    }

    pub fn for_control2<'f>(
        &'f mut self,
        module_id: ModuleId,
        output_slot: usize,
        f: impl FnMut(
            &mut RouterFactory<'f, 'c, ControlRouterType>,
            VoiceTarget,
            &mut VoicesLayout<SamplesOutput>,
        ),
    ) where
        'c: 'f,
    {
        RouterFactory {
            ctx: self,
            module_id,
            data_type: ControlRouterType {
                samples_slot: output_slot,
            },
        }
        .with_output_slot2(f);
    }

    pub fn for_spectral<'f>(
        &'f mut self,
        module_id: ModuleId,
        output_slot: usize,
        f: impl FnOnce(
            &mut RouterFactory<'f, 'c, SpectralRouterType>,
            &mut VoicesLayout<SpectralOutput>,
        ),
    ) where
        'c: 'f,
    {
        RouterFactory {
            ctx: self,
            module_id,
            data_type: SpectralRouterType {
                spectral_slot: output_slot,
            },
        }
        .with_output_slot(f);
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
