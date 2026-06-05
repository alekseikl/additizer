use std::f32;

use crate::{
    synth_engine::{
        Sample, StereoSample,
        biquad_filter::BiquadFilter,
        buffer::{HARMONIC_SERIES_BUFFER, SPECTRAL_BUFFER_SIZE, SpectralBuffer},
        routing::{DataType, ModuleId, ModuleType, NUM_CHANNELS, Router},
        synth_module::{ModInput, ProcessParams, SynthModule},
        types::ComplexSample,
    },
    utils::NthElement,
};

mod config;
mod link;
mod ui_bridge;

pub use config::{ComplexCfg, Config};
use link::{AudioEnd, UiEnd, UiEvent, create_link_pair};
pub use ui_bridge::UiBridge;

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

#[derive(Clone, Copy, PartialEq)]
pub enum FilterType {
    LowPass,
    HighPass,
    BandPass,
    BandStop,
    Peaking,
}

#[derive(Clone, Copy)]
pub struct FilterParams {
    pub filter_type: FilterType,
    pub filter_order: StereoSample,
    pub cutoff: StereoSample,
    pub q: StereoSample,
    pub gain: StereoSample,
}

impl BiquadFilter {
    fn iter_for_type(
        &self,
        filter_type: FilterType,
        order: Sample,
    ) -> impl Iterator<Item = ComplexSample> {
        let order = order.clamp(1.0, 8.0);
        let power = order / 2.0;

        let iter: Box<dyn Iterator<Item = ComplexSample>> = match filter_type {
            FilterType::LowPass => Box::new(self.low_pass()),
            FilterType::HighPass => Box::new(self.high_pass()),
            FilterType::BandPass => Box::new(self.band_pass()),
            FilterType::BandStop => Box::new(self.band_stop()),
            FilterType::Peaking => Box::new(self.peaking()),
        };

        iter.map(move |response| response.powf(power))
    }
}

pub struct HarmonicEditor {
    id: ModuleId,
    outputs: [SpectralBuffer; NUM_CHANNELS],
    audio_end: AudioEnd,
    ui_end: Option<UiEnd>,
}

impl HarmonicEditor {
    pub fn new(id: ModuleId) -> Self {
        Self::from_config(&Config {
            id,
            ..Config::default()
        })
    }

    pub fn from_config(config: &config::Config) -> Self {
        let (audio_end, ui_end) = create_link_pair();
        let mut outputs = [HARMONIC_SERIES_BUFFER; NUM_CHANNELS];

        for (channel, cfg_channel) in outputs.iter_mut().zip(&config.spectrum) {
            if cfg_channel.len() == SPECTRAL_BUFFER_SIZE {
                for (out, cfg) in channel.iter_mut().zip(cfg_channel.iter()) {
                    *out = cfg.complex();
                }
            }
        }

        Self {
            id: config.id,
            outputs,
            audio_end,
            ui_end: Some(ui_end),
        }
    }

    pub fn take_ui_end(&mut self) -> Option<UiEnd> {
        self.ui_end.take()
    }

    pub fn return_ui_end(&mut self, ui_end: UiEnd) {
        assert!(self.ui_end.is_none(), "ui_end not taken");
        self.ui_end = Some(ui_end);
    }

    pub fn get_config(&self) -> Config {
        Config {
            id: self.id,
            spectrum: self.outputs.map(|channel| {
                channel
                    .iter()
                    .map(|complex| ComplexCfg::from_complex(*complex))
                    .collect()
            }),
        }
    }

    pub fn harmonics_from_config(config: &Config) -> Vec<StereoSample> {
        let mut magnitudes = vec![StereoSample::ZERO; SPECTRAL_BUFFER_SIZE];

        for (channel_idx, channel) in config.spectrum.iter().enumerate() {
            for (harmonic_idx, (magnitude, harmonic)) in
                magnitudes.iter_mut().zip(channel.iter()).enumerate()
            {
                let value = harmonic_idx as Sample * f32::consts::PI * harmonic.complex().norm();
                let almost_one = (value - 1.0).abs() < Sample::EPSILON;

                magnitude[channel_idx] =
                    Sample::from(almost_one) * 1.0 + Sample::from(!almost_one) * value;
            }
        }

        magnitudes
    }

    pub fn set_harmonic(&mut self, harmonic_number: usize, gain: StereoSample) {
        let idx = harmonic_number.clamp(1, SPECTRAL_BUFFER_SIZE - 1);

        for (spectrum, gain) in self.outputs.iter_mut().zip(gain.iter()) {
            spectrum[idx] = HARMONIC_SERIES_BUFFER[idx] * gain;
        }
    }

    pub fn set_selected(&mut self, params: &SetParams) {
        let idx_from = params.from.clamp(1, SPECTRAL_BUFFER_SIZE - 1);
        let range = idx_from..(params.to + 1).clamp(idx_from, SPECTRAL_BUFFER_SIZE);

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
    }

    pub fn apply_filter(&mut self, params: &FilterParams) {
        for (channel_idx, spectrum) in self.outputs.iter_mut().enumerate() {
            let filter = BiquadFilter::new(
                params.gain[channel_idx],
                params.cutoff[channel_idx],
                params.q[channel_idx],
            );

            let filter_iter =
                filter.iter_for_type(params.filter_type, params.filter_order[channel_idx]);

            for (out, response) in spectrum.iter_mut().zip(filter_iter) {
                *out *= response;
            }
        }
    }
}

impl SynthModule for HarmonicEditor {
    fn id(&self) -> ModuleId {
        self.id
    }

    fn label(&self) -> String {
        "Harmonics".into()
    }

    fn set_label(&mut self, _label: String) {}

    fn module_type(&self) -> ModuleType {
        ModuleType::HarmonicEditor
    }

    fn inputs(&self) -> &'static [ModInput] {
        &[]
    }

    fn output(&self) -> DataType {
        DataType::Spectral
    }

    fn handle_ui_events(&mut self) {
        let mut refresh = false;

        while let Some(event) = self.audio_end.pop_event() {
            match event {
                UiEvent::SetHarmonic {
                    harmonic_number,
                    gain,
                } => self.set_harmonic(harmonic_number, gain),
                UiEvent::SetSelected(params) => {
                    self.set_selected(&params);
                    refresh = true;
                }
                UiEvent::ApplyFilter(params) => {
                    self.apply_filter(&params);
                    refresh = true;
                }
            }
        }

        if refresh {
            self.audio_end.push_refresh_state();
        }
    }

    fn process(&mut self, _params: &ProcessParams, _router: &mut dyn Router) {}

    fn get_spectral_output(
        &self,
        _current: bool,
        _voice_idx: usize,
        channel_idx: usize,
    ) -> &SpectralBuffer {
        &self.outputs[channel_idx]
    }
}
