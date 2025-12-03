use std::{any::Any, f64};

use itertools::izip;
use serde::{Deserialize, Serialize};

use crate::synth_engine::{
    StereoSample,
    buffer::{
        HARMONIC_SERIES_BUFFER, SPECTRAL_BUFFER_SIZE, SpectralBuffer, ZEROES_SPECTRAL_BUFFER,
    },
    routing::{DataType, ModuleId, ModuleType, NUM_CHANNELS, Router},
    synth_module::{InputInfo, ModuleConfigBox, ProcessParams, SynthModule},
};

const NUM_EDITABLE_HARMONICS: usize = SPECTRAL_BUFFER_SIZE - 2;

#[derive(Clone, Serialize, Deserialize)]
pub struct HarmonicEditorConfig {
    label: Option<String>,
    harmonics: Vec<StereoSample>,
}

impl Default for HarmonicEditorConfig {
    fn default() -> Self {
        Self {
            label: None,
            harmonics: vec![StereoSample::splat(1.0); NUM_EDITABLE_HARMONICS],
        }
    }
}

pub struct HarmonicEditor {
    id: ModuleId,
    label: String,
    config: ModuleConfigBox<HarmonicEditorConfig>,
    harmonics: Vec<StereoSample>,
    outputs: [SpectralBuffer; NUM_CHANNELS],
}

impl HarmonicEditor {
    pub fn new(id: ModuleId, config: ModuleConfigBox<HarmonicEditorConfig>) -> Self {
        let mut editor = Self {
            id,
            label: format!("Harmonic Editor {id}"),
            config,
            harmonics: vec![StereoSample::splat(1.0); NUM_EDITABLE_HARMONICS],
            outputs: [ZEROES_SPECTRAL_BUFFER; NUM_CHANNELS],
        };

        {
            let config = editor.config.lock();

            if let Some(label) = config.label.as_ref() {
                editor.label = label.clone();
            }

            if config.harmonics.len() == NUM_EDITABLE_HARMONICS {
                editor.harmonics = config.harmonics.clone();
            }
        }

        editor.update_buffers();
        editor
    }

    gen_downcast_methods!();

    pub fn set_all_to_zero(&mut self) {
        self.harmonics.fill(StereoSample::splat(0.0));
        self.apply_harmonics();
    }

    pub fn set_all_to_one(&mut self) {
        self.harmonics.fill(StereoSample::splat(1.0));
        self.apply_harmonics();
    }

    pub fn keep_selected(&mut self, a: isize, b: isize) {
        let matches = |idx: usize| -> bool {
            let i = idx as isize + 1;

            if a == 0 {
                i == b
            } else {
                let result = (i - b) as f64 / a as f64;

                result >= 0.0 && result.fract().abs() < f32::EPSILON as f64
            }
        };

        for (idx, harmonic) in self.harmonics.iter_mut().enumerate() {
            if !matches(idx) {
                *harmonic = StereoSample::splat(0.0);
            }
        }

        self.apply_harmonics();
    }

    pub fn apply_harmonics(&mut self) {
        self.update_buffers();
        self.config.lock().harmonics = self.harmonics.clone();
    }

    fn update_buffers(&mut self) {
        let (channel_l, channel_r) = self.outputs.split_at_mut(1);
        let buff_l = &mut channel_l[0];
        let buff_r = &mut channel_r[0];
        let range = 1..(self.harmonics.len() + 1);

        for ((out_l, out_r), series_factor, harmonic) in izip!(
            buff_l[range.clone()]
                .iter_mut()
                .zip(buff_r[range.clone()].iter_mut()),
            &HARMONIC_SERIES_BUFFER[range],
            &self.harmonics
        ) {
            *out_l = series_factor * harmonic.left();
            *out_r = series_factor * harmonic.right();
        }
    }

    pub fn harmonics_ref_mut(&mut self) -> &mut [StereoSample] {
        &mut self.harmonics
    }
}

impl SynthModule for HarmonicEditor {
    fn id(&self) -> ModuleId {
        self.id
    }

    fn label(&self) -> String {
        self.label.clone()
    }

    fn set_label(&mut self, label: String) {
        self.label = label.clone();
        self.config.lock().label = Some(label);
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::HarmonicEditor
    }

    fn inputs(&self) -> &'static [InputInfo] {
        &[]
    }

    fn output_type(&self) -> DataType {
        DataType::Spectral
    }

    fn process(&mut self, _params: &ProcessParams, _router: &dyn Router) {}

    fn get_spectral_output(
        &self,
        _current: bool,
        _voice_idx: usize,
        channel_idx: usize,
    ) -> &SpectralBuffer {
        &self.outputs[channel_idx]
    }
}
