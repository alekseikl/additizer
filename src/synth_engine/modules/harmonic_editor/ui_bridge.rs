use crate::synth_engine::{ComplexSample, StereoSample, synth_module::ModuleUiBridge};

use super::link::UiEnd;
use super::{EditRequest, HarmonicEditor, Harmonics};

pub struct HarmonicEditorUiBridge {
    ui_end: UiEnd,
}

impl HarmonicEditorUiBridge {
    pub fn try_new(editor: &mut HarmonicEditor) -> Option<Self> {
        Some(Self {
            ui_end: editor.ui_end.take()?,
        })
    }

    pub fn harmonics_mut(&mut self) -> &mut Harmonics {
        self.ui_end.get_harmonics_mut()
    }

    pub fn get_display_spectrum(&mut self) -> &[ComplexSample] {
        self.ui_end.get_display_spectrum()
    }

    pub fn set_harmonic(&mut self, harmonic_number: usize, gain: StereoSample) {
        self.ui_end.set_amplitude(harmonic_number, gain);
    }

    pub fn clear(&mut self) {
        self.ui_end.clear();
    }

    pub fn reset_sawtooth(&mut self) {
        self.ui_end.reset_sawtooth();
    }

    pub fn edit_request(&mut self, request: EditRequest) {
        self.ui_end.edit_request(request);
    }

    pub fn apply_draft(&mut self) {
        self.ui_end.apply_draft();
    }

    pub fn discard_draft(&mut self) {
        self.ui_end.discard_draft();
    }
}

impl ModuleUiBridge for HarmonicEditorUiBridge {
    fn update(&mut self) -> bool {
        false
    }
}
