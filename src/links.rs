pub mod amplifier;
pub mod engine;
pub mod envelope;
pub mod expressions;
pub mod external_param;
pub mod harmonic_editor;
pub mod lfo;
pub mod mixer;
pub mod oscillator;
pub mod pitch;
pub mod spectral_band_select;
pub mod spectral_blend;
pub mod spectral_eq;
pub mod spectral_filter;
pub mod spectral_mixer;
pub mod spectral_noise;
pub mod svf;
pub mod wave_shaper;

use crate::synth_engine::engine_io::EngineLinks;

pub struct PluginLinks;

impl EngineLinks for PluginLinks {
    type AudioEnd = engine::AudioEnd;
    type UiEnd = engine::UiEnd;
    type Amplifier = amplifier::Links;
    type Envelope = envelope::Links;
    type Expressions = expressions::Links;
    type ExternalParam = external_param::Links;
    type HarmonicEditor = harmonic_editor::Links;
    type Lfo = lfo::Links;
    type Mixer = mixer::Links;
    type Oscillator = oscillator::Links;
    type Pitch = pitch::Links;
    type SpectralBandSelect = spectral_band_select::Links;
    type SpectralBlend = spectral_blend::Links;
    type SpectralEq = spectral_eq::Links;
    type SpectralFilter = spectral_filter::Links;
    type SpectralMixer = spectral_mixer::Links;
    type SpectralNoise = spectral_noise::Links;
    type Svf = svf::Links;
    type WaveShaper = wave_shaper::Links;

    fn create_link_pair() -> (Self::AudioEnd, Self::UiEnd) {
        engine::make_link_pair()
    }
}
