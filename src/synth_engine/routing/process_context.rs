use crate::synth_engine::{
    Input, ModuleId, NUM_CHANNELS, Sample, SmoothedSampleParams, StereoSample,
    engine_io::EngineAudioEnd,
    routing::{
        AudioRouterType, ControlRouterType, OutputRouterType, OutputsArena, RouterFactory,
        SpectralRouterType,
    },
    voices_handler::PlayingVoice,
};

trait TelemetrySink {
    fn update_modulated_input(
        &mut self,
        module_id: ModuleId,
        input: Input,
        channel: u8,
        value: Sample,
        normalized_value: Sample,
    ) -> bool;

    fn update_out_volume(&mut self, volume: StereoSample, clipped: [bool; NUM_CHANNELS]);
}

impl<A: EngineAudioEnd> TelemetrySink for A {
    fn update_modulated_input(
        &mut self,
        module_id: ModuleId,
        input: Input,
        channel: u8,
        value: Sample,
        normalized_value: Sample,
    ) -> bool {
        EngineAudioEnd::update_modulated_input(
            self,
            module_id,
            input,
            channel,
            value,
            normalized_value,
        )
    }

    fn update_out_volume(&mut self, volume: StereoSample, clipped: [bool; NUM_CHANNELS]) {
        EngineAudioEnd::update_out_volume(self, volume, clipped)
    }
}

/// UI telemetry published during `process`. Forwards into the engine audio end
/// without putting that type on [`ProcessContext`].
pub struct Telemetry<'a> {
    sink: &'a mut dyn TelemetrySink,
}

impl<'a> Telemetry<'a> {
    pub(crate) fn from_end<A: EngineAudioEnd + 'a>(end: &'a mut A) -> Self {
        Self { sink: end }
    }

    pub fn update_modulated_input(
        &mut self,
        module_id: ModuleId,
        input: Input,
        channel: u8,
        value: Sample,
        normalized_value: Sample,
    ) -> bool {
        self.sink
            .update_modulated_input(module_id, input, channel, value, normalized_value)
    }

    pub fn update_out_volume(&mut self, volume: StereoSample, clipped: [bool; NUM_CHANNELS]) {
        self.sink.update_out_volume(volume, clipped)
    }
}

pub struct ProcessParams<'a> {
    pub trigger_stage: bool,
    pub has_triggered_voices: bool,
    pub samples: usize,
    pub sample_rate: Sample,
    pub needs_update_ui: bool,
    pub smooth_params: SmoothedSampleParams,
    pub active_voices: &'a [PlayingVoice],
}

pub struct ProcessContext<'c> {
    pub outputs_arena: &'c mut OutputsArena,
    pub telemetry: &'c mut Telemetry<'c>,
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
        crate::utils::note_to_pitch(self.note as Sample)
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
