use crate::synth_engine::{ComplexSample, StereoSample, synth_module::ModuleUiBridge};

use super::link::UiEnd;
use super::{EditRequest, HarmonicEditor, Harmonics, clamp_bandwidth};

pub struct HarmonicEditorUiBridge {
    ui_end: UiEnd,
    bandwidth: i32,
    mono: bool,
}

impl HarmonicEditorUiBridge {
    pub fn try_new(editor: &mut HarmonicEditor) -> Option<Self> {
        Some(Self {
            bandwidth: editor.bandwidth(),
            mono: editor.mono(),
            ui_end: editor.ui_end.take()?,
        })
    }

    pub fn harmonics_mut(&mut self) -> &mut Harmonics {
        self.ui_end.get_harmonics_mut()
    }

    pub fn get_display_spectrum(&mut self) -> &[ComplexSample] {
        self.ui_end.get_display_spectrum()
    }

    pub fn bandwidth(&self) -> i32 {
        self.bandwidth
    }

    pub fn set_bandwidth(&mut self, bandwidth: i32) {
        let bandwidth = clamp_bandwidth(bandwidth);

        if self.ui_end.set_bandwidth(bandwidth) {
            self.bandwidth = bandwidth;
        }
    }

    pub fn mono(&self) -> bool {
        self.mono
    }

    pub fn set_mono(&mut self, mono: bool) {
        if self.ui_end.set_mono(mono) {
            self.mono = mono;
        }
    }

    pub fn set_harmonic(&mut self, harmonic_number: usize, gain: StereoSample) {
        self.ui_end.set_amplitude(harmonic_number, gain);
    }

    pub fn set_phase(&mut self, harmonic_number: usize, phase: StereoSample) {
        self.ui_end.set_phase(harmonic_number, phase);
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
