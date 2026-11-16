mod config;
mod link;
pub mod stub;

pub use config::ExternalParamConfig;
pub use link::{ExternalParamAudioEnd, ExternalParamLinks, ExternalParamUiEnd, UiEvent};

use crate::synth_engine::{
    Buffer, ModuleId, Smp,
    buffer::{
        MonoVoicesLayout, ValueBuffer, VoicesLayout, new_mono_voices_layout_with, zero_buffer,
    },
    routing::{
        ControlRouterType, DataType, InputMeta, ProcessContext, RouterFactory, SamplesOutput,
        VoiceEvent, VoiceTarget,
    },
    synth_module::SynthModule,
};

pub const NUM_EXT_PARAMS: usize = 8;

struct Params {
    selected_param_index: usize,
    sample_on_trigger: bool,
}

impl Params {
    fn from_config(c: &config::ExternalParamConfig) -> Self {
        Self {
            selected_param_index: c.selected_param_index.min(NUM_EXT_PARAMS - 1),
            sample_on_trigger: c.sample_on_trigger,
        }
    }
}

struct Voice {
    values: ValueBuffer,
    value_at_trigger: Smp,
    buffer: Buffer,
}

impl Voice {
    fn new(buffer_size: usize) -> Self {
        Self {
            values: ValueBuffer::new(buffer_size),
            value_at_trigger: 0.0,
            buffer: zero_buffer(buffer_size),
        }
    }
}

pub struct ExternalParam<L: ExternalParamLinks = stub::Links> {
    id: ModuleId,
    params: Params,
    audio_end: L::AudioEnd,
    ui_end: Option<L::UiEnd>,
    output_slot: usize,
    values: ValueBuffer,
    /// Monophonic modulation offsets. Added to `values` once per block.
    mod_values: ValueBuffer,
    mono_buff: Buffer,
    voices: MonoVoicesLayout<Voice>,
}

impl<L: ExternalParamLinks> ExternalParam<L> {
    pub fn take_ui_end(&mut self) -> Option<L::UiEnd> {
        self.ui_end.take()
    }

    pub fn new(id: ModuleId, buffer_size: usize) -> Self {
        Self::from_config(
            &ExternalParamConfig {
                id,
                ..ExternalParamConfig::default()
            },
            buffer_size,
        )
    }

    pub fn from_config(config: &config::ExternalParamConfig, buffer_size: usize) -> Self {
        let (audio_end, ui_end) = L::create_link_pair();

        Self {
            id: config.id,
            params: Params::from_config(config),
            audio_end,
            ui_end,
            output_slot: usize::MAX,
            values: ValueBuffer::new(buffer_size),
            mod_values: ValueBuffer::new(buffer_size),
            mono_buff: zero_buffer(buffer_size),
            voices: new_mono_voices_layout_with(|| Voice::new(buffer_size)),
        }
    }

    pub fn get_config(&self) -> ExternalParamConfig {
        ExternalParamConfig {
            id: self.id,
            selected_param_index: self.params.selected_param_index,
            sample_on_trigger: self.params.sample_on_trigger,
        }
    }

    set_mono_param!(
        select_param,
        selected_param_index,
        usize,
        selected_param_index.min(NUM_EXT_PARAMS - 1)
    );
    set_mono_param!(set_sample_on_trigger, sample_on_trigger, bool);

    pub fn handle_mono_automation(&mut self, param_idx: usize, offset: usize, value: Smp) {
        if param_idx == self.params.selected_param_index {
            self.values.set(value, offset);
        }
    }

    pub fn handle_mono_modulation(&mut self, param_idx: usize, offset: usize, value_offset: Smp) {
        if param_idx == self.params.selected_param_index {
            self.mod_values.set(value_offset, offset);
        }
    }

    pub fn handle_poly_modulation(
        &mut self,
        param_idx: usize,
        voice_idx: usize,
        offset: usize,
        value_offset: Smp,
    ) {
        if param_idx == self.params.selected_param_index {
            self.voices[voice_idx].values.set(value_offset, offset);
        }
    }

    fn process_voice(
        &mut self,
        target: &VoiceTarget,
        outputs: &mut VoicesLayout<SamplesOutput>,
        rf: &mut RouterFactory<ControlRouterType, L::EngineEnd>,
    ) {
        let block_samples = rf.params().samples;
        let voice = &mut self.voices[target.voice_idx];
        let (router, mut voice_output) = rf.for_voice(target, outputs);

        // Mono voice state is shared across channels; prepare it once.
        if target.channel_idx == 0 {
            let buff = &mut voice.buffer[..block_samples];
            let mono = &self.mono_buff[..block_samples];

            if voice.values.is_empty() {
                for (out, &mono_value) in buff.iter_mut().zip(mono) {
                    *out = mono_value.clamp(0.0, 1.0);
                }
            } else {
                voice.values.read_and_reset(buff);

                for (out, &mono_value) in buff.iter_mut().zip(mono) {
                    *out = (mono_value + *out).clamp(0.0, 1.0);
                }
            }

            if self.params.sample_on_trigger {
                if let Some(offset) = target.triggered {
                    voice.value_at_trigger = buff[offset];
                }

                buff.fill(voice.value_at_trigger);
            }

            if router.need_update_ui() {
                let shown = target
                    .triggered
                    .map(|offset| buff[offset])
                    .unwrap_or(buff[0]);

                self.audio_end.update_value(shown);
            }
        }

        voice_output.fill_with_ext_control(&voice.buffer[..block_samples]);
    }

    pub(crate) fn process(&mut self, ctx: &mut ProcessContext<L::EngineEnd>) {
        ctx.control(self.id, self.output_slot)
            .for_module(|rf| {
                let buff = &mut self.mono_buff[..rf.params().samples];

                self.values.read_and_reset(buff);

                if !self.mod_values.is_empty() {
                    self.mod_values.add_and_reset(buff);
                }
            })
            .for_voices(|rf, target, outputs| {
                self.process_voice(target, outputs, rf);
            })
            .advance(|rf| {
                if rf.params().needs_update_ui && rf.params().active_voices.is_empty() {
                    self.audio_end.update_value(self.mono_buff[0]);
                }
            });
    }
}

impl<L: ExternalParamLinks> SynthModule for ExternalParam<L> {
    fn id(&self) -> ModuleId {
        self.id
    }

    fn inputs(&self) -> &'static [InputMeta] {
        &[]
    }

    fn output_type(&self) -> DataType {
        DataType::Control
    }

    fn output_slot(&self) -> usize {
        self.output_slot
    }

    fn set_output_slot(&mut self, slot: usize) {
        self.output_slot = slot;
    }

    fn process_events(&mut self, events: &[VoiceEvent]) {
        for event in events {
            if let VoiceEvent::Reset { voice_idx, .. } = event {
                self.voices[*voice_idx].values.clear();
            }
        }
    }

    fn process_ui_events(&mut self) {
        while let Some(event) = self.audio_end.pop_event() {
            match event {
                UiEvent::SelectedParamIndex(index) => self.select_param(index),
                UiEvent::SampleOnTrigger(value) => self.set_sample_on_trigger(value),
            }
        }
    }
}
