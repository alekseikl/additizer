use std::sync::Arc;

use parking_lot::Mutex;

use crate::synth_engine::{
    ModuleId, SPECTRAL_BUFFER_SIZE, StereoSample, SynthEngine, buffer::HARMONIC_SERIES_BUFFER,
};

use super::link::{UiEnd, UiUpdate};
use super::{FilterParams, HarmonicEditor, HarmonicEditorConfig, SetParams};

pub struct UiBridge {
    synth: Arc<Mutex<SynthEngine>>,
    module_id: ModuleId,
    ui_end: Option<UiEnd>,
    config: HarmonicEditorConfig,
}

impl UiBridge {
    pub fn create(module_id: ModuleId, synth: Arc<Mutex<SynthEngine>>) -> Option<Self> {
        let mut synth_lock = synth.lock();
        let editor = synth_lock.get_typed_module_mut::<HarmonicEditor>(module_id)?;
        let ui_end = editor.take_ui_end()?;
        let config = editor.get_config();

        drop(synth_lock);

        Some(Self {
            synth,
            module_id,
            ui_end: Some(ui_end),
            config,
        })
    }

    pub fn module_id(&self) -> ModuleId {
        self.module_id
    }

    pub fn sync(&mut self) {
        let synth_lock = self.synth.lock();

        if let Some(editor) = synth_lock.get_typed_module::<HarmonicEditor>(self.module_id) {
            self.config = editor.get_config();
        }
    }

    pub fn update(&mut self) {
        while let Some(update) = self.ui_end.as_mut().unwrap().pop_update() {
            match update {
                UiUpdate::RefreshState => {
                    self.sync();
                }
            }
        }
    }

    // pub fn config(&self) -> &Config {
    //     &self.config
    // }

    pub fn harmonics(&self) -> Vec<StereoSample> {
        HarmonicEditor::harmonics_from_config(&self.config)
    }

    pub fn set_harmonic(&mut self, harmonic_number: usize, gain: StereoSample) {
        if self
            .ui_end
            .as_mut()
            .unwrap()
            .set_harmonic(harmonic_number, gain)
        {
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
        self.ui_end.as_mut().unwrap().set_selected(params);
    }

    pub fn apply_filter(&mut self, params: FilterParams) {
        self.ui_end.as_mut().unwrap().apply_filter(params);
    }
}

impl Drop for UiBridge {
    fn drop(&mut self) {
        let mut synth_lock = self.synth.lock();

        if let Some(editor) = synth_lock.get_typed_module_mut::<HarmonicEditor>(self.module_id) {
            editor.return_ui_end(self.ui_end.take().unwrap());
        }
    }
}
