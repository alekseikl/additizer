//! No-op engine links. [`super::SynthEngine`] defaults to [`StubLinks`].
//! Module link stubs live next to each module.

use crate::synth_engine::{
    Input, InputId, ModuleId, NUM_CHANNELS, Sample, StereoSample, amplifier, engine_io, envelope,
    expressions, external_param, harmonic_editor, lfo, mixer, oscillator, pitch,
    spectral_band_select, spectral_blend, spectral_eq, spectral_filter, spectral_mixer,
    spectral_noise, svf, wave_shaper,
};

/// Audio-thread end that drops every UI event and telemetry update.
pub struct AudioEnd;

/// UI-thread end that is never constructed.
pub enum NoUi {}

impl NoUi {
    fn diverge(&self) -> ! {
        match *self {}
    }
}

/// Stub [`crate::synth_engine::EngineLinks`].
pub struct StubLinks;

impl engine_io::EngineLinks for StubLinks {
    type AudioEnd = AudioEnd;
    type UiEnd = NoUi;
    type Amplifier = amplifier::stub::Links;
    type Envelope = envelope::stub::Links;
    type Expressions = expressions::stub::Links;
    type ExternalParam = external_param::stub::Links;
    type HarmonicEditor = harmonic_editor::stub::Links;
    type Lfo = lfo::stub::Links;
    type Mixer = mixer::stub::Links;
    type Oscillator = oscillator::stub::Links;
    type Pitch = pitch::stub::Links;
    type SpectralBandSelect = spectral_band_select::stub::Links;
    type SpectralBlend = spectral_blend::stub::Links;
    type SpectralEq = spectral_eq::stub::Links;
    type SpectralFilter = spectral_filter::stub::Links;
    type SpectralMixer = spectral_mixer::stub::Links;
    type SpectralNoise = spectral_noise::stub::Links;
    type Svf = svf::stub::Links;
    type WaveShaper = wave_shaper::stub::Links;

    fn create_link_pair() -> (Self::AudioEnd, Option<Self::UiEnd>) {
        (AudioEnd, None)
    }
}

impl engine_io::EngineAudioEnd for AudioEnd {
    fn update_modulated_input(
        &mut self,
        _module_id: ModuleId,
        _input: Input,
        _channel: u8,
        _value: Sample,
        _normalized_value: Sample,
    ) -> bool {
        false
    }

    fn update_voices_status(&mut self, _metrics: &engine_io::VoicesHandlerMetrics) -> bool {
        false
    }

    fn pop_event(&mut self) -> Option<engine_io::UiEvent> {
        None
    }

    fn update_out_volume(&mut self, _volume: StereoSample, _clipped: [bool; NUM_CHANNELS]) {}
}

impl engine_io::EngineUiEnd for NoUi {
    fn get_out_volume(&mut self) -> engine_io::OutputMeter {
        self.diverge()
    }

    fn set_link_amount(&mut self, _src: ModuleId, _dst: InputId, _amount: StereoSample) -> bool {
        self.diverge()
    }

    fn set_voices(&mut self, _voices: usize) -> bool {
        self.diverge()
    }

    fn set_legato(&mut self, _legato: bool) -> bool {
        self.diverge()
    }

    fn set_block_size(&mut self, _block_size: usize) -> bool {
        self.diverge()
    }

    fn set_voice_kill_time(&mut self, _voice_kill_time: Sample) -> bool {
        self.diverge()
    }

    fn set_oversampling(&mut self, _oversampling: bool) -> bool {
        self.diverge()
    }

    fn set_output_gain(&mut self, _output_gain: StereoSample) -> bool {
        self.diverge()
    }

    fn pop_update(&mut self) -> Option<engine_io::UiUpdate> {
        self.diverge()
    }
}

#[cfg(test)]
mod tests;
