mod config;
mod link;

#[cfg(test)]
mod tests;

pub use crate::ui_bridge::modules::spectral_band_select::SpectralBandSelectUiBridge;
pub use config::{
    BandSelectMode, MAX_BAND_HZ, MAX_HARMONIC, MAX_HARMONIC_END, MIN_BAND_HZ, MIN_HARMONIC,
    SpectralBandSelectConfig,
};
pub use link::{
    SpectralBandSelectAudioEnd, SpectralBandSelectLinks, SpectralBandSelectUiEnd, UiEvent,
};

use crate::{
    synth_engine::{
        ComplexSample,
        buffer::{DC_OFFSET, VoicesLayout},
        routing::{
            DataType, Input, InputMeta, InputSlots, ModuleId, ProcessContext, RouterFactory,
            SpectralInputSlot, SpectralOutput, SpectralRouterType, VoiceTarget,
        },
        synth_module::SynthModule,
        types::Sample,
    },
    utils::pitch_to_freq,
};

/// Pulls a frequency ratio that landed a hair off an integer back onto that bin.
const HARMONIC_TOLERANCE: Sample = 1e-3;

struct Params {
    mode: BandSelectMode,
    harmonic_from: u16,
    harmonic_to: u16,
    freq_from: Sample,
    freq_to: Sample,
}

impl Params {
    fn from_config(c: &SpectralBandSelectConfig) -> Self {
        let c = c.clone().sanitized();

        Self {
            mode: c.mode,
            harmonic_from: c.harmonic_from,
            harmonic_to: c.harmonic_to,
            freq_from: c.freq_from,
            freq_to: c.freq_to,
        }
    }
}

#[derive(Default)]
pub struct Inputs {
    spectrum: Option<usize>,
    pitch: Option<usize>,
}

impl Inputs {
    fn from_slots(inputs: &[InputSlots], spectral_inputs: &[SpectralInputSlot]) -> Self {
        let mut result = Self::default();

        for input in inputs {
            if matches!(input.input_type, Input::Pitch) {
                result.pitch = input.slots.first().map(|s| s.src_slot);
            }
        }

        for input in spectral_inputs {
            if matches!(input.input_type, Input::Spectrum) {
                result.spectrum = Some(input.slot);
            }
        }

        result
    }
}

pub struct SpectralBandSelect<
    L: SpectralBandSelectLinks = crate::links::spectral_band_select::Links,
> {
    id: ModuleId,
    params: Params,
    audio_end: L::AudioEnd,
    ui_end: Option<L::UiEnd>,
    inputs: Inputs,
    output_slot: usize,
}

impl<L: SpectralBandSelectLinks> SpectralBandSelect<L> {
    pub fn take_ui_end(&mut self) -> Option<L::UiEnd> {
        self.ui_end.take()
    }

    pub fn new(id: ModuleId) -> Self {
        Self::from_config(&SpectralBandSelectConfig {
            id,
            ..SpectralBandSelectConfig::default()
        })
    }

    pub fn from_config(config: &SpectralBandSelectConfig) -> Self {
        let (audio_end, ui_end) = L::create_link_pair();
        let params = Params::from_config(config);

        Self {
            id: config.id,
            params,
            audio_end,
            ui_end: Some(ui_end),
            inputs: Inputs::default(),
            output_slot: usize::MAX,
        }
    }

    pub fn get_config(&self) -> SpectralBandSelectConfig {
        SpectralBandSelectConfig {
            id: self.id,
            mode: self.params.mode,
            harmonic_from: self.params.harmonic_from,
            harmonic_to: self.params.harmonic_to,
            freq_from: self.params.freq_from,
            freq_to: self.params.freq_to,
        }
    }

    set_mono_param!(set_mode, mode, BandSelectMode);
    set_mono_param!(
        set_harmonic_from,
        harmonic_from,
        u16,
        harmonic_from.clamp(MIN_HARMONIC, MAX_HARMONIC)
    );
    set_mono_param!(
        set_harmonic_to,
        harmonic_to,
        u16,
        harmonic_to.clamp(MIN_HARMONIC, MAX_HARMONIC_END)
    );
    set_mono_param!(
        set_freq_from,
        freq_from,
        Sample,
        freq_from.clamp(MIN_BAND_HZ, MAX_BAND_HZ)
    );
    set_mono_param!(
        set_freq_to,
        freq_to,
        Sample,
        freq_to.clamp(MIN_BAND_HZ, MAX_BAND_HZ)
    );

    fn quantize_harmonic(ratio: Sample) -> u16 {
        let snapped = (ratio - HARMONIC_TOLERANCE).ceil();

        if !snapped.is_finite() || snapped <= 0.0 {
            0
        } else if snapped >= u16::MAX as Sample {
            u16::MAX
        } else {
            snapped as u16
        }
    }

    fn process_voice(
        &mut self,
        target: &VoiceTarget,
        outputs: &mut VoicesLayout<SpectralOutput>,
        rf: &mut RouterFactory<SpectralRouterType, L::EngineEnd>,
    ) {
        let (router, mut voice_output) = rf.for_voice(target, outputs);
        let pitch = router
            .direct_opt(self.inputs.pitch)
            .unwrap_or_else(|| target.note_pitch());
        let input = router.spectral(self.inputs.spectrum);
        let output = voice_output.output(input.len());

        let (from, to) = match self.params.mode {
            BandSelectMode::Harmonic => (self.params.harmonic_from, self.params.harmonic_to),
            BandSelectMode::Frequency => {
                let fundamental = pitch_to_freq(pitch).max(1.0);

                (
                    Self::quantize_harmonic(self.params.freq_from / fundamental),
                    Self::quantize_harmonic(self.params.freq_to / fundamental),
                )
            }
        };

        if to <= from {
            output.fill(ComplexSample::ZERO);
        } else {
            let range = (from as usize)..(to as usize);

            for (i, (out, &inp)) in output.iter_mut().zip(input).enumerate().skip(DC_OFFSET) {
                *out = if range.contains(&i) {
                    inp
                } else {
                    ComplexSample::ZERO
                };
            }
        }

        if router.need_update_ui_mono() {
            self.audio_end.update_spectrum(output);
        }
    }
    pub(crate) fn process(&mut self, ctx: &mut ProcessContext<L::EngineEnd>) {
        ctx.spectral(self.id, self.output_slot)
            .for_voices(|rf, target, outputs| {
                self.process_voice(target, outputs, rf);
            });
    }
}

impl<L: SpectralBandSelectLinks> SynthModule for SpectralBandSelect<L> {
    fn id(&self) -> ModuleId {
        self.id
    }

    fn inputs(&self) -> &'static [InputMeta] {
        static INPUTS: &[InputMeta] = &[
            InputMeta::spectral(Input::Spectrum),
            InputMeta::direct_control(Input::Pitch),
        ];

        INPUTS
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

    fn set_input_slots(&mut self, inputs: &[InputSlots], spectral_inputs: &[SpectralInputSlot]) {
        self.inputs = Inputs::from_slots(inputs, spectral_inputs);
    }

    fn process_ui_events(&mut self) {
        while let Some(event) = self.audio_end.pop_event() {
            match event {
                UiEvent::Mode(mode) => self.set_mode(mode),
                UiEvent::HarmonicFrom(value) => self.set_harmonic_from(value),
                UiEvent::HarmonicTo(value) => self.set_harmonic_to(value),
                UiEvent::FreqFrom(value) => self.set_freq_from(value),
                UiEvent::FreqTo(value) => self.set_freq_to(value),
            }
        }
    }
}
