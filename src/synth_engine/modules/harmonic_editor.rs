use std::any::Any;

use itertools::izip;
use serde::{Deserialize, Serialize};

use crate::{
    synth_engine::{
        Sample, StereoSample,
        buffer::{
            HARMONIC_SERIES_BUFFER, SPECTRAL_BUFFER_SIZE, SpectralBuffer, ZEROES_SPECTRAL_BUFFER,
        },
        routing::{DataType, ModuleId, ModuleType, NUM_CHANNELS, Router},
        synth_module::{InputInfo, ModuleConfigBox, ProcessParams, SynthModule},
    },
    utils::NthElement,
};

#[derive(Clone, Copy, PartialEq)]
pub enum SetAction {
    Set,
    Multiple,
}

pub struct SetParams {
    pub from: usize, // One based index
    pub to: usize,
    pub n_th: Option<NthElement>,
    pub action: SetAction,
    pub gain: StereoSample,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct HarmonicEditorConfig {
    label: Option<String>,
    harmonics: Vec<StereoSample>,
}

impl Default for HarmonicEditorConfig {
    fn default() -> Self {
        Self {
            label: None,
            harmonics: vec![StereoSample::splat(1.0); HarmonicEditor::NUM_EDITABLE_HARMONICS],
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
    pub const NUM_EDITABLE_HARMONICS: usize = SPECTRAL_BUFFER_SIZE - 2;

    pub fn new(id: ModuleId, config: ModuleConfigBox<HarmonicEditorConfig>) -> Self {
        let mut editor = Self {
            id,
            label: format!("Harmonic Editor {id}"),
            config,
            harmonics: vec![StereoSample::splat(1.0); Self::NUM_EDITABLE_HARMONICS],
            outputs: [ZEROES_SPECTRAL_BUFFER; NUM_CHANNELS],
        };

        {
            let config = editor.config.lock();

            if let Some(label) = config.label.as_ref() {
                editor.label = label.clone();
            }

            if config.harmonics.len() == Self::NUM_EDITABLE_HARMONICS {
                editor.harmonics = config.harmonics.clone();
            }
        }

        editor.update_buffers();
        editor
    }

    gen_downcast_methods!();

    pub fn set_selected(&mut self, params: &SetParams) {
        assert!(!self.harmonics.is_empty());

        const ZERO_THRESHOLD: Sample = 0.000011; //Slightly above -100dB

        let max_idx = self.harmonics.len() - 1;
        let idx_from = params.from.wrapping_sub(1).clamp(0, max_idx);
        let range = idx_from..params.to.clamp(idx_from, self.harmonics.len());
        let gain: StereoSample = params
            .gain
            .iter()
            .map(|gain| Sample::from(*gain > ZERO_THRESHOLD) * gain)
            .collect();

        for (idx, harmonic) in self.harmonics[range].iter_mut().enumerate() {
            let matches = params
                .n_th
                .as_ref()
                .is_none_or(|n_th| n_th.matches(idx_from + idx));

            if !matches {
                continue;
            }

            match params.action {
                SetAction::Set => *harmonic = gain,
                SetAction::Multiple => *harmonic = *harmonic * gain,
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

    fn output(&self) -> DataType {
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
