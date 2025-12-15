use std::{any::Any, f32};

use serde::{Deserialize, Serialize};

use crate::{
    synth_engine::{
        Sample, StereoSample,
        buffer::{HARMONIC_SERIES_BUFFER, SPECTRAL_BUFFER_SIZE, SpectralBuffer},
        routing::{DataType, ModuleId, ModuleType, NUM_CHANNELS, Router},
        synth_module::{InputInfo, ModuleConfigBox, ProcessParams, SynthModule},
        types::ComplexSample,
    },
    utils::NthElement,
};

const NUM_HARMONICS: usize = SPECTRAL_BUFFER_SIZE - 1;

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

#[derive(Default, Clone, Copy, Serialize, Deserialize)]
pub struct ComplexCfg {
    re: Sample,
    im: Sample,
}

impl ComplexCfg {
    fn from_complex(complex: ComplexSample) -> Self {
        Self {
            re: complex.re,
            im: complex.im,
        }
    }

    fn complex(&self) -> ComplexSample {
        ComplexSample::new(self.re, self.im)
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct HarmonicEditorConfig {
    label: Option<String>,
    spectrum: [Vec<ComplexCfg>; NUM_CHANNELS],
}

impl Default for HarmonicEditorConfig {
    fn default() -> Self {
        let mut cfg = Self {
            label: None,
            spectrum: Default::default(),
        };

        let harmonic_series = &HARMONIC_SERIES_BUFFER;

        for channel in &mut cfg.spectrum {
            channel.extend(
                harmonic_series
                    .iter()
                    .take(NUM_HARMONICS)
                    .map(|c| ComplexCfg::from_complex(*c)),
            );
        }

        cfg
    }
}

pub struct HarmonicEditor {
    id: ModuleId,
    label: String,
    config: ModuleConfigBox<HarmonicEditorConfig>,
    outputs: [SpectralBuffer; NUM_CHANNELS],
}

impl HarmonicEditor {
    pub fn new(id: ModuleId, config: ModuleConfigBox<HarmonicEditorConfig>) -> Self {
        let mut editor = Self {
            id,
            label: format!("Harmonic Editor {id}"),
            config,
            outputs: [HARMONIC_SERIES_BUFFER; NUM_CHANNELS],
        };

        {
            let config = editor.config.lock();

            if let Some(label) = config.label.as_ref() {
                editor.label = label.clone();
            }

            for (channel, cfg_channel) in editor.outputs.iter_mut().zip(&config.spectrum) {
                if cfg_channel.len() == NUM_HARMONICS {
                    for (out, cfg) in channel.iter_mut().zip(cfg_channel.iter()) {
                        *out = cfg.complex();
                    }
                }
            }
        }

        editor
    }

    gen_downcast_methods!();

    pub fn get_harmonics(&self) -> Vec<StereoSample> {
        let mut magnitudes = vec![StereoSample::ZERO; NUM_HARMONICS];

        for (channel_idx, channel) in self.outputs.iter().enumerate() {
            for (harmonic_idx, (magnitude, harmonic)) in
                magnitudes.iter_mut().zip(channel.iter()).enumerate()
            {
                let norm = harmonic.norm();
                let mg = harmonic_idx as Sample * norm * f32::consts::PI;

                magnitude[channel_idx] = mg;
            }
        }

        magnitudes
    }

    pub fn set_harmonic(&mut self, harmonic_number: usize, gain: StereoSample) {
        let idx = harmonic_number.clamp(1, NUM_HARMONICS - 1);

        for (spectrum, gain) in self.outputs.iter_mut().zip(gain.iter()) {
            spectrum[idx] = HARMONIC_SERIES_BUFFER[idx] * gain;
        }

        let mut config = self.config.lock();

        for (cfg_spectrum, spectrum) in config.spectrum.iter_mut().zip(self.outputs) {
            cfg_spectrum[idx] = ComplexCfg::from_complex(spectrum[idx]);
        }
    }

    pub fn set_selected(&mut self, params: &SetParams) {
        let idx_from = params.from.clamp(1, NUM_HARMONICS - 1);
        let range = idx_from..params.to.clamp(idx_from, NUM_HARMONICS);

        for (spectrum, gain) in self.outputs.iter_mut().zip(params.gain.iter()) {
            for (idx, (harmonic, initial_harmonic)) in spectrum[range.clone()]
                .iter_mut()
                .zip(HARMONIC_SERIES_BUFFER[range.clone()].iter())
                .enumerate()
            {
                let matches = params
                    .n_th
                    .as_ref()
                    .is_none_or(|n_th| n_th.matches(idx_from - 1 + idx));

                if !matches {
                    continue;
                }

                match params.action {
                    SetAction::Set => *harmonic = *initial_harmonic * gain,
                    SetAction::Multiple => *harmonic *= gain,
                }
            }
        }

        self.save_harmonics();
    }

    fn save_harmonics(&self) {
        let mut config = self.config.lock();

        for (cfg_channel, channel) in config.spectrum.iter_mut().zip(self.outputs.iter()) {
            for (cfg, out) in cfg_channel.iter_mut().zip(channel.iter()) {
                *cfg = ComplexCfg::from_complex(*out);
            }
        }
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
