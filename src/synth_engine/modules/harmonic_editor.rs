use std::any::Any;

use itertools::izip;
use serde::{Deserialize, Serialize};

use crate::synth_engine::{
    StereoSample,
    buffer::{
        HARMONIC_SERIES_BUFFER, SPECTRAL_BUFFER_SIZE, SpectralBuffer, ZEROES_SPECTRAL_BUFFER,
    },
    routing::{InputType, ModuleId, ModuleType, NUM_CHANNELS, OutputType, Router},
    synth_module::{ModuleConfigBox, ProcessParams, SynthModule},
};

const NUM_EDITABLE_HARMONICS: usize = SPECTRAL_BUFFER_SIZE - 2;

#[derive(Clone, Serialize, Deserialize)]
pub struct HarmonicEditorConfig {
    label: Option<String>,
    harmonics: Vec<StereoSample>,
    tail: StereoSample,
}

impl Default for HarmonicEditorConfig {
    fn default() -> Self {
        Self {
            label: None,
            harmonics: vec![StereoSample::splat(1.0); NUM_EDITABLE_HARMONICS],
            tail: StereoSample::splat(1.0),
        }
    }
}

pub struct HarmonicEditor {
    id: ModuleId,
    label: String,
    config: ModuleConfigBox<HarmonicEditorConfig>,
    harmonics: Vec<StereoSample>,
    tail: StereoSample,
    outputs: [SpectralBuffer; NUM_CHANNELS],
}

impl HarmonicEditor {
    pub fn new(id: ModuleId, config: ModuleConfigBox<HarmonicEditorConfig>) -> Self {
        let mut editor = Self {
            id,
            label: format!("Harmonic Editor {id}"),
            config,
            harmonics: vec![StereoSample::splat(1.0); NUM_EDITABLE_HARMONICS],
            tail: StereoSample::splat(1.0),
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

            editor.tail = config.tail;
        }

        editor.update_buffers();
        editor
    }

    gen_downcast_methods!(HarmonicEditor);

    pub fn set_harmonics(&mut self, harmonics: &[StereoSample], tail: StereoSample) {
        self.harmonics = harmonics.to_vec();
        self.tail = tail;

        self.apply_harmonics();
    }

    pub fn apply_harmonics(&mut self) {
        self.update_buffers();

        {
            let mut config = self.config.lock();
            config.harmonics = self.harmonics.clone();
            config.tail = self.tail;
        }
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

        let range = (self.harmonics.len() + 1)..buff_l.len();

        for ((out_l, out_r), series_factor) in buff_l[range.clone()]
            .iter_mut()
            .zip(buff_r[range.clone()].iter_mut())
            .zip(HARMONIC_SERIES_BUFFER[range].iter())
        {
            *out_l = series_factor * self.tail.left();
            *out_r = series_factor * self.tail.right();
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

    fn is_spectral_rate(&self) -> bool {
        true
    }

    fn inputs(&self) -> &'static [InputType] {
        &[]
    }

    fn output_type(&self) -> OutputType {
        OutputType::Spectrum
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
