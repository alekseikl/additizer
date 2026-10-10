#![allow(dead_code)]

use addi_engine::{
    DataType, ModuleId, Sample,
    routing::{EngineAudioEnd, InputMeta, ProcessContext},
    synth_module::SynthModule,
};

use super::fill::Fill;

/// Writes a fixed audio buffer for every active voice.
pub struct Audio {
    id: ModuleId,
    output_slot: usize,
    fill: Fill,
}

impl Audio {
    pub fn constant(id: ModuleId, value: Sample) -> Self {
        Self {
            id,
            output_slot: usize::MAX,
            fill: Fill::Constant(value),
        }
    }

    pub fn set_constant(&mut self, value: Sample) {
        self.fill = Fill::Constant(value);
    }

    pub fn set_samples(&mut self, samples: &[Sample]) {
        assert!(!samples.is_empty(), "audio fill needs at least one sample");

        self.fill = Fill::Samples(samples.to_vec());
    }

    pub fn process<A: EngineAudioEnd>(&mut self, ctx: &mut ProcessContext<A>) {
        let id = self.id;
        let slot = self.output_slot;

        ctx.audio(id, slot).for_voices(|rf, target, outputs| {
            let (_router, mut voice_output) = rf.for_voice(target, outputs);

            self.fill.write(voice_output.output());
        });
    }
}

impl SynthModule for Audio {
    fn id(&self) -> ModuleId {
        self.id
    }

    fn inputs(&self) -> &'static [InputMeta] {
        &[]
    }

    fn output_type(&self) -> DataType {
        DataType::Audio
    }

    fn output_slot(&self) -> usize {
        self.output_slot
    }

    fn set_output_slot(&mut self, slot: usize) {
        self.output_slot = slot;
    }

    fn process_ui_events(&mut self) {}
}
