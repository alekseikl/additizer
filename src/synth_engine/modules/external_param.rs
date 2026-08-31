mod config;
mod link;
mod ui_bridge;

pub use config::ExternalParamConfig;
use link::{AudioEnd, UiEnd, UiEvent, create_link_pair};
pub use ui_bridge::ExternalParamUiBridge;

use crate::params::ExtParam;
use crate::synth_engine::{
    Buffer, ModuleId, Sample,
    buffer::{MonoVoicesLayout, ValueBuffer, VoicesLayout, new_mono_voices_layout, zero_buffer},
    routing::{
        ControlRouterType, DataType, InputMeta, ProcessContext, RouterFactory, SamplesOutput,
        VoiceEvent, VoiceTarget,
    },
    smooth::Smoother,
    synth_module::SynthModule,
};

pub const NUM_EXT_PARAMS: usize = 4;

struct Params {
    selected_param_index: usize,
    smooth: Sample,
    sample_on_trigger: bool,
    make_bipolar: bool,
    polyphonic: bool,
}

impl Params {
    fn from_config(c: &config::ExternalParamConfig) -> Self {
        Self {
            selected_param_index: c.selected_param_index.min(NUM_EXT_PARAMS - 1),
            smooth: c.smooth,
            sample_on_trigger: c.sample_on_trigger,
            make_bipolar: c.make_bipolar,
            polyphonic: c.polyphonic,
        }
    }
}

struct Voice {
    values: ValueBuffer,
    value_at_trigger: Sample,
    buffer: Buffer,
    smoother: Smoother,
}

impl Default for Voice {
    fn default() -> Self {
        Self {
            values: ValueBuffer::default(),
            value_at_trigger: 0.0,
            buffer: zero_buffer(),
            smoother: Smoother::default(),
        }
    }
}

pub struct ExternalParam {
    id: ModuleId,
    params: Params,
    audio_end: AudioEnd,
    ui_end: Option<UiEnd>,
    output_slot: usize,
    values: ValueBuffer,
    mono_buff: Buffer,
    voices: MonoVoicesLayout<Voice>,
}

impl ExternalParam {
    pub fn new(id: ModuleId) -> Self {
        Self::from_config(&ExternalParamConfig {
            id,
            ..ExternalParamConfig::default()
        })
    }

    pub fn from_config(config: &config::ExternalParamConfig) -> Self {
        let (audio_end, ui_end) = create_link_pair();

        Self {
            id: config.id,
            params: Params::from_config(config),
            audio_end,
            ui_end: Some(ui_end),
            output_slot: usize::MAX,
            values: ValueBuffer::default(),
            mono_buff: zero_buffer(),
            voices: new_mono_voices_layout(),
        }
    }

    pub fn get_config(&self) -> ExternalParamConfig {
        ExternalParamConfig {
            id: self.id,
            selected_param_index: self.params.selected_param_index,
            smooth: self.params.smooth,
            sample_on_trigger: self.params.sample_on_trigger,
            make_bipolar: self.params.make_bipolar,
            polyphonic: self.params.polyphonic,
        }
    }

    set_mono_param!(
        select_param,
        selected_param_index,
        usize,
        selected_param_index.min(NUM_EXT_PARAMS - 1)
    );
    set_mono_param!(set_smooth, smooth, Sample);
    set_mono_param!(set_sample_on_trigger, sample_on_trigger, bool);
    set_mono_param!(set_make_bipolar, make_bipolar, bool);
    set_mono_param!(set_polyphonic, polyphonic, bool);

    // Set parameters values at the beginning of a block
    pub fn set_values(&mut self, values: &[ExtParam; NUM_EXT_PARAMS]) {
        let param = &values[self.params.selected_param_index];
        let value = if self.params.polyphonic {
            param.unmodulated()
        } else {
            param.modulated()
        };

        self.values.set(value, 0);
    }

    pub fn handle_mono_automation(
        &mut self,
        param_idx: usize,
        offset: usize,
        value: Sample,
        params: &[ExtParam; NUM_EXT_PARAMS],
    ) {
        if param_idx == self.params.selected_param_index {
            if self.params.polyphonic {
                self.values.set(value, offset);
            } else {
                let param = &params[self.params.selected_param_index];

                self.values
                    .set(value + (param.modulated() - param.unmodulated()), offset);
            }
        }
    }

    pub fn handle_poly_modulation(
        &mut self,
        param_idx: usize,
        voice_idx: usize,
        offset: usize,
        value_offset: Sample,
    ) {
        if self.params.polyphonic && param_idx == self.params.selected_param_index {
            self.voices[voice_idx].values.set(value_offset, offset);
        }
    }

    fn process_voice(
        &mut self,
        target: &VoiceTarget,
        outputs: &mut VoicesLayout<SamplesOutput>,
        rf: &mut RouterFactory<ControlRouterType>,
    ) {
        let block_samples = rf.params().samples;
        let sample_rate = rf.params().sample_rate;
        let voice = &mut self.voices[target.voice_idx];
        let (router, mut voice_output) = rf.for_voice(target, outputs);

        // Mono voice state is shared across channels; prepare it once.
        if target.channel_idx == 0 {
            let buff = &mut voice.buffer[..block_samples];
            let mono = &self.mono_buff[..block_samples];

            voice.values.read_and_reset(buff);

            for (out, &mono_value) in buff.iter_mut().zip(mono) {
                *out = (mono_value + *out).clamp(0.0, 1.0);
            }

            if self.params.sample_on_trigger {
                if let Some(offset) = target.triggered {
                    voice.value_at_trigger = buff[offset];
                }

                buff.fill(voice.value_at_trigger);
            }

            let offset = if let Some(offset) = target.triggered {
                voice.smoother.reset(buff[offset]);
                offset
            } else {
                0
            };

            voice
                .smoother
                .apply_if_needed(sample_rate, self.params.smooth, &mut buff[offset..]);

            if router.need_update_ui() {
                self.audio_end.update_value(buff[offset]);
            }

            if self.params.make_bipolar {
                for sample in &mut buff[offset..] {
                    *sample = *sample * 2.0 - 1.0;
                }
            }
        }

        voice_output.fill_with_ext_control(&voice.buffer[..block_samples]);
    }
}

impl SynthModule for ExternalParam {
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
                self.voices[*voice_idx].values.set(0.0, 0);
            }
        }
    }

    fn process_ui_events(&mut self) {
        while let Some(event) = self.audio_end.pop_event() {
            match event {
                UiEvent::SelectedParamIndex(index) => self.select_param(index),
                UiEvent::Smooth(value) => self.set_smooth(value),
                UiEvent::SampleOnTrigger(value) => self.set_sample_on_trigger(value),
                UiEvent::MakeBipolar(value) => self.set_make_bipolar(value),
                UiEvent::Polyphonic(value) => self.set_polyphonic(value),
            }
        }
    }

    fn process(&mut self, ctx: &mut ProcessContext) {
        let samples = ctx.params.samples;
        let read_values = ctx.params.trigger_stage || !ctx.params.has_triggered_voices;

        if read_values {
            self.values.read_and_reset(&mut self.mono_buff[..samples]);
        }

        ctx.control(self.id, self.output_slot)
            .for_voices(|rf, target, outputs| {
                self.process_voice(target, outputs, rf);
            });

        if ctx.params.needs_update_ui && ctx.params.active_voices.is_empty() {
            self.audio_end.update_value(self.mono_buff[0]);
        }
    }
}
