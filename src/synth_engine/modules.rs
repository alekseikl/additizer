mod amplifier;
mod envelope;
mod external_param;
pub mod harmonic_editor;
mod lfo;
mod modulation_filter;
mod oscillator;
mod spectral_blend;
mod spectral_filter;

pub use amplifier::{Amplifier, AmplifierConfig};
pub use envelope::{Envelope, EnvelopeConfig, EnvelopeCurve};
pub use external_param::{ExternalParam, ExternalParamConfig, ExternalParamsBlock};
pub use lfo::{Lfo, LfoConfig, LfoShape};
pub use modulation_filter::{ModulationFilter, ModulationFilterConfig};
pub use oscillator::{Oscillator, OscillatorConfig};
pub use spectral_blend::{SpectralBlend, SpectralBlendConfig};
pub use spectral_filter::{SpectralFilter, SpectralFilterConfig};
