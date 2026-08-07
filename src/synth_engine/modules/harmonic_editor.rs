use std::f32;

use crate::{
    synth_engine::{
        Sample, StereoSample, VoiceEvent,
        biquad_filter::{BandPass, BandStop, FilterImpl, HighPass, LowPass, Peaking},
        buffer::{
            HARMONIC_SERIES_BUFFER, SPECTRAL_BUFFER_SIZE, SpectralBuffer, VoicesLayout,
            new_voices_layout,
        },
        routing::{
            DataType, Input, InputMeta, InputSlots, ModuleId, NUM_CHANNELS, ProcessContext,
            RouterFactory, SpectralInputSlot, SpectralOutput, SpectralRouterType, VoiceTarget,
        },
        synth_module::SynthModule,
    },
    utils::NthElement,
};

mod config;
mod link;
mod ui_bridge;

pub use config::{ComplexCfg, HarmonicEditorConfig};
use link::{AudioEnd, UiEnd, UiEvent, create_link_pair};
pub use ui_bridge::{DISPLAY_SPECTRUM_SIZE, HarmonicEditorUiBridge};

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

fn apply_filter_response(spectrum: &mut SpectralBuffer, filter: impl FilterImpl, power: Sample) {
    for (out, response) in spectrum
        .iter_mut()
        .zip(filter.into_iter(SPECTRAL_BUFFER_SIZE))
    {
        *out *= response.powf(power);
    }
}

#[derive(Clone, Copy)]
struct Voice {
    needs_update: bool,
}

impl Default for Voice {
    fn default() -> Self {
        Self { needs_update: true }
    }
}

pub struct HarmonicEditor {
    id: ModuleId,
    harmonics: [SpectralBuffer; NUM_CHANNELS],
    audio_end: AudioEnd,
    ui_end: Option<UiEnd>,
    output_slot: usize,
    voices: VoicesLayout<Voice>,
    triggers: VoicesLayout<Option<usize>>,
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
            output_slot: usize::MAX,
            voices: new_voices_layout(),
            triggers: new_voices_layout(),
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

    pub fn set_needs_update(&mut self) {
        for channel in self.voices.iter_mut() {
            for voice in channel.iter_mut() {
                voice.needs_update = true;
            }
        }
    }

    pub fn set_harmonic(&mut self, harmonic_number: usize, gain: StereoSample) {
        let idx = harmonic_number.clamp(1, SPECTRAL_BUFFER_SIZE - 1);

        for (spectrum, gain) in self.harmonics.iter_mut().zip(gain.iter()) {
            spectrum[idx] = HARMONIC_SERIES_BUFFER[idx] * gain;
        }

        self.set_needs_update();
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

        self.set_needs_update();
    }

    pub fn apply_filter(&mut self, params: &FilterParams) {
        for (channel_idx, spectrum) in self.harmonics.iter_mut().enumerate() {
            let gain = params.gain[channel_idx];
            let cutoff = params.cutoff[channel_idx];
            let q = params.q[channel_idx];
            let power = params.filter_order[channel_idx].clamp(1.0, 8.0) / 2.0;

            match params.filter_type {
                FilterType::LowPass => {
                    apply_filter_response(spectrum, LowPass::new(gain, cutoff, q), power)
                }
                FilterType::HighPass => {
                    apply_filter_response(spectrum, HighPass::new(gain, cutoff, q), power)
                }
                FilterType::BandPass => {
                    apply_filter_response(spectrum, BandPass::new(gain, cutoff, q), power)
                }
                FilterType::BandStop => {
                    apply_filter_response(spectrum, BandStop::new(gain, cutoff, q), power)
                }
                FilterType::Peaking => {
                    apply_filter_response(spectrum, Peaking::new(gain, cutoff, q), power)
                }
            }
        }

        self.set_needs_update();
    }

    fn process_voice(
        &mut self,
        target: VoiceTarget,
        outputs: &mut VoicesLayout<SpectralOutput>,
        rf: &mut RouterFactory<SpectralRouterType>,
    ) -> bool {
        let voice = &mut self.voices[target.channel_idx][target.voice_idx];

        if !voice.needs_update {
            return false;
        }

        let (router, mut voice_output) = rf.for_voice2(&target, &mut self.triggers, outputs);
        let out = voice_output.output();
        let triggered = router.triggered();

        out.copy_from_slice(&self.harmonics[target.channel_idx][..out.len()]);

        if !triggered {
            voice.needs_update = false;
        }

        triggered
    }
}

impl SynthModule for HarmonicEditor {
    fn id(&self) -> ModuleId {
        self.id
    }

    fn inputs(&self) -> &'static [InputMeta] {
        &[]
    }

    fn output_type(&self) -> DataType {
        DataType::Spectral
    }

    fn output_slot(&self) -> usize {
        self.output_slot
    }

    fn set_output_slot(&mut self, slot: usize) {
        self.output_slot = slot;
    }

    fn set_input_slots(&mut self, _inputs: &[InputSlots], _spectral_inputs: &[SpectralInputSlot]) {}

    fn update_input_amount(&mut self, _input_type: Input, _src_slot: usize, _amount: StereoSample) {
    }

    fn process_events(&mut self, events: &[VoiceEvent]) {
        for trigger_channel in self.triggers.iter_mut() {
            for event in events {
                if let VoiceEvent::Trigger {
                    voice_idx, offset, ..
                } = event
                {
                    trigger_channel[*voice_idx] = Some(*offset);
                }
            }
        }
    }

    fn process_ui_events(&mut self) {
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
        ctx.for_spectral2(self.id, self.output_slot, |rf, target, outputs| {
            self.process_voice(target, outputs, rf)
        });
    }
}
