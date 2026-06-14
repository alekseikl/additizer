use std::f32;

use crate::{
    synth_engine::{
        Sample, StereoSample, VoiceEvent,
        biquad_filter::BiquadFilter,
        buffer::{
            HARMONIC_SERIES_BUFFER, SPECTRAL_BUFFER_SIZE, SpectralBuffer, VoicesLayout,
            new_voices_layout,
        },
        routing::{
            DataType, Input, InputSlots, ModuleId, ModuleType, NUM_CHANNELS, ProcessContext,
            SpectralInputSlot,
        },
        synth_module::{ModInput, SynthModule},
        types::ComplexSample,
    },
    utils::NthElement,
};

mod config;
mod link;
mod ui_bridge;

pub use config::{ComplexCfg, HarmonicEditorConfig};
use link::{AudioEnd, UiEnd, UiEvent, create_link_pair};
pub use ui_bridge::HarmonicEditorUiBridge;

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

#[derive(Default)]
struct Voice {
    triggered: bool,
}

pub struct HarmonicEditor {
    id: ModuleId,
    harmonics: [SpectralBuffer; NUM_CHANNELS],
    audio_end: AudioEnd,
    ui_end: Option<UiEnd>,
    output_slot: usize,
    voices: VoicesLayout<Voice>,
}

impl HarmonicEditor {
    pub fn new(id: ModuleId) -> Self {
        Self::from_config(&HarmonicEditorConfig {
            id,
            ..HarmonicEditorConfig::default()
        })
    }

    pub fn from_config(config: &config::HarmonicEditorConfig) -> Self {
        let (audio_end, ui_end) = create_link_pair();
        let mut harmonics = [HARMONIC_SERIES_BUFFER; NUM_CHANNELS];

        for (channel, cfg_channel) in harmonics.iter_mut().zip(&config.spectrum) {
            if cfg_channel.len() == SPECTRAL_BUFFER_SIZE {
                for (out, cfg) in channel.iter_mut().zip(cfg_channel.iter()) {
                    *out = cfg.complex();
                }
            }
        }

        Self {
            id: config.id,
            harmonics,
            audio_end,
            ui_end: Some(ui_end),
            output_slot: 0,
            voices: new_voices_layout(),
        }
    }

    pub fn get_config(&self) -> HarmonicEditorConfig {
        HarmonicEditorConfig {
            id: self.id,
            spectrum: self.harmonics.map(|channel| {
                channel
                    .iter()
                    .map(|complex| ComplexCfg::from_complex(*complex))
                    .collect()
            }),
        }
    }

    pub fn harmonics_from_config(config: &HarmonicEditorConfig) -> Vec<StereoSample> {
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

        for (spectrum, gain) in self.harmonics.iter_mut().zip(gain.iter()) {
            spectrum[idx] = HARMONIC_SERIES_BUFFER[idx] * gain;
        }
    }

    pub fn set_selected(&mut self, params: &SetParams) {
        let idx_from = params.from.clamp(1, SPECTRAL_BUFFER_SIZE - 1);
        let range = idx_from..(params.to + 1).clamp(idx_from, SPECTRAL_BUFFER_SIZE);

        for (spectrum, gain) in self.harmonics.iter_mut().zip(params.gain.iter()) {
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
        for (channel_idx, spectrum) in self.harmonics.iter_mut().enumerate() {
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

    fn module_type(&self) -> ModuleType {
        ModuleType::HarmonicEditor
    }

    fn inputs(&self) -> &'static [ModInput] {
        &[]
    }

    fn output(&self) -> DataType {
        DataType::Spectral
    }

    fn output_slot(&self) -> usize {
        self.output_slot
    }

    fn set_slots(
        &mut self,
        _inputs: &[InputSlots],
        _spectral_inputs: &[SpectralInputSlot],
        output_slot: usize,
    ) {
        self.output_slot = output_slot;
    }

    fn update_input_amount(&mut self, _input_type: Input, _src_slot: usize, _amount: StereoSample) {
    }

    fn handle_events(&mut self, events: &[VoiceEvent]) {
        for channel in self.voices.iter_mut() {
            for event in events {
                if let VoiceEvent::Trigger { voice_idx, .. } = event {
                    channel[*voice_idx].triggered = true;
                }
            }
        }
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

    fn process(&mut self, ctx: &mut ProcessContext) {
        ctx.for_spectral(self.id, self.output_slot, |router, output| {
            let num_active_voices = router.params().active_voices.len();
            let spectrum_channels = router.params().spectrum_channels;

            for channel_idx in 0..spectrum_channels {
                for seq_idx in 0..num_active_voices {
                    let voice_idx = router.params().active_voices[seq_idx];
                    let voice = &mut self.voices[channel_idx][voice_idx];
                    let voice_output = &mut output[channel_idx][voice_idx];

                    if voice.triggered {
                        *voice_output.advance() = self.harmonics[channel_idx];
                        voice.triggered = false;
                    }

                    *voice_output.advance() = self.harmonics[channel_idx];
                }
            }
        });
    }
}
