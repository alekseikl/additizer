use std::sync::Arc;

use parking_lot::Mutex;

use crate::synth_engine::{
    ModuleId, SPECTRAL_BUFFER_SIZE, StereoSample, SynthEngine, buffer::HARMONIC_SERIES_BUFFER,
    synth_module::ModuleUiBridge,
};

use super::link::{UiEnd, UiUpdate};
use super::{FilterParams, HarmonicEditor, HarmonicEditorConfig, SetParams};

pub struct HarmonicEditorUiBridge {
    synth: Arc<Mutex<SynthEngine>>,
    module_id: ModuleId,
    ui_end: UiEnd,
    config: HarmonicEditorConfig,
}

impl HarmonicEditorUiBridge {
    pub fn try_new(
        module_id: ModuleId,
        synth: Arc<Mutex<SynthEngine>>,
        editor: &mut HarmonicEditor,
    ) -> Option<Self> {
        Some(Self {
            synth,
            module_id,
            ui_end: editor.ui_end.take()?,
            config: editor.get_config(),
        })
    }

    pub fn sync(&mut self) {
        let synth_lock = self.synth.lock();

        if let Some(editor) = synth_lock.get_typed_module::<HarmonicEditor>(self.module_id) {
            self.config = editor.get_config();
        }
    }

    // pub fn config(&self) -> &Config {
    //     &self.config
    // }

    pub fn harmonics(&self) -> Vec<StereoSample> {
        HarmonicEditor::harmonics_from_config(&self.config)
    }

    pub fn set_harmonic(&mut self, harmonic_number: usize, gain: StereoSample) {
        if self.ui_end.set_harmonic(harmonic_number, gain) {
            let idx = harmonic_number.clamp(1, SPECTRAL_BUFFER_SIZE - 1);

            for (channel, gain) in self.config.spectrum.iter_mut().zip(gain.iter()) {
                if idx < channel.len() {
                    channel[idx] =
                        super::config::ComplexCfg::from_complex(HARMONIC_SERIES_BUFFER[idx] * gain);
                }
            }
        }
    }

    pub fn set_selected(&mut self, params: SetParams) {
        self.ui_end.set_selected(params);
    }

    pub fn apply_filter(&mut self, params: FilterParams) {
        self.ui_end.apply_filter(params);
    }
}

impl ModuleUiBridge for HarmonicEditorUiBridge {
    fn update(&mut self) {
        while let Some(update) = self.ui_end.pop_update() {
            match update {
                UiUpdate::RefreshState => {
                    self.sync();
                }
            }
        }
    }
}

