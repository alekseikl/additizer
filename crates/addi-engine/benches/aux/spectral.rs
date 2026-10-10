use addi_engine::{
    ComplexSample, DataType, ModuleId, SPECTRAL_BUFFER_SIZE,
    routing::{EngineAudioEnd, InputMeta, ProcessContext},
    synth_module::SynthModule,
};

/// Writes one spectrum into every active voice.
pub struct Spectral {
    id: ModuleId,
    output_slot: usize,
    spectrum: Vec<ComplexSample>,
}

impl Spectral {
    pub fn new(id: ModuleId, spectrum: &[ComplexSample]) -> Self {
        let mut module = Self {
            id,
            output_slot: usize::MAX,
            spectrum: Vec::new(),
        };

        module.set_spectrum(spectrum);
        module
    }

    pub fn set_spectrum(&mut self, spectrum: &[ComplexSample]) {
        assert!(
            spectrum.len() <= SPECTRAL_BUFFER_SIZE,
            "spectrum is longer than a spectral buffer"
        );

        self.spectrum.clear();
        self.spectrum.extend_from_slice(spectrum);
    }

    pub fn process<A: EngineAudioEnd>(&mut self, ctx: &mut ProcessContext<A>) {
        let id = self.id;
        let slot = self.output_slot;

        ctx.spectral(id, slot).for_voices(|rf, target, outputs| {
            let (_router, mut voice_output) = rf.for_voice(target, outputs);
            let out = voice_output.output(self.spectrum.len());

            out.copy_from_slice(&self.spectrum);
        });
    }
}

impl SynthModule for Spectral {
    fn id(&self) -> ModuleId {
        self.id
    }

    fn inputs(&self) -> &'static [InputMeta] {
        &[]
    }

    fn output_type(&self) -> DataType {
        DataType::Spectral
    }

    fn output_slot(&self) -> usize {
        self.output_slot
    }

    fn set_output_slot(&mut self, slot: usize) {
        self.output_slot = slot;
    }

    fn process_ui_events(&mut self) {}
}
