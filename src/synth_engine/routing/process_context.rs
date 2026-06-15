use crate::synth_engine::{
    ModuleId, Sample, SmoothedSampleParams,
    buffer::VoicesLayout,
    routing::{
        AudioRouterType, ControlRouterType, OutputRouterType, OutputsArena, RouterFactory,
        SamplesOutput, SpectralOutput, SpectralRouterType,
    },
    ui_bridge::AudioEnd,
};

pub struct ProcessParams<'a> {
    pub samples: usize,
    pub sample_rate: Sample,
    // pub buffer_t_step: Sample,
    pub needs_update_ui: bool,
    pub smooth_params: SmoothedSampleParams,
    pub spectrum_channels: usize,
    pub active_voices: &'a [usize],
}

pub struct ProcessContext<'c> {
    pub outputs_arena: &'c mut OutputsArena,
    pub audio_end: &'c mut AudioEnd,
    pub params: ProcessParams<'c>,
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
