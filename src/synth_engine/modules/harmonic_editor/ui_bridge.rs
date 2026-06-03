use std::sync::Arc;

use parking_lot::Mutex;

use crate::synth_engine::{ModuleId, SPECTRAL_BUFFER_SIZE, StereoSample, SynthEngine};

use super::link::{UiEnd, UiUpdate};
use super::{FilterParams, HarmonicEditor, SetParams};

#[derive(Clone)]
pub struct ControlsState {
    pub harmonics: Vec<StereoSample>,
}

pub struct UiBridge {
    synth: Arc<Mutex<SynthEngine>>,
    module_id: ModuleId,
    ui_end: Option<UiEnd>,
    controls: ControlsState,
}

impl UiBridge {
    pub fn create(module_id: ModuleId, synth: Arc<Mutex<SynthEngine>>) -> Option<Self> {
        let mut synth_lock = synth.lock();
        let editor = synth_lock.get_typed_module_mut::<HarmonicEditor>(module_id)?;
        let ui_end = editor.take_ui_end()?;
        let controls = editor.get_controls_state();

        drop(synth_lock);

        Some(Self {
            synth,
            module_id,
            ui_end: Some(ui_end),
            controls,
        })
    }

    pub fn module_id(&self) -> ModuleId {
        self.module_id
    }

    pub fn sync(&mut self) {
        let synth_lock = self.synth.lock();

        if let Some(editor) = synth_lock.get_typed_module::<HarmonicEditor>(self.module_id) {
            self.controls = editor.get_controls_state();
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

    pub fn controls(&self) -> &ControlsState {
        &self.controls
    }

    pub fn set_harmonic(&mut self, harmonic_number: usize, gain: StereoSample) {
        if self
            .ui_end
            .as_mut()
            .unwrap()
            .set_harmonic(harmonic_number, gain)
        {
            let idx = harmonic_number.clamp(1, SPECTRAL_BUFFER_SIZE - 1);
            self.controls.harmonics[idx] = gain;
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
