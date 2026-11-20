pub mod amplifier;
pub mod envelope;
pub mod expressions;
pub mod external_param;
pub mod harmonic_editor;
pub mod lfo;
pub mod mixer;
pub mod mod_map;
pub mod multi_envelope;
pub mod oscillator;
pub mod pitch;
pub mod segments_envelope;
pub mod spectral_band_select;
pub mod spectral_blend;
pub mod spectral_eq;
pub mod spectral_filter;
pub mod spectral_mixer;
pub mod spectral_noise;
pub mod svf;
pub mod wave_shaper;

use enum_dispatch::enum_dispatch;

use addi_engine::{ModuleId, ModuleType, ModuleUiLink, engine_io::EngineLinks};

use amplifier::AmplifierUiBridge;
use envelope::EnvelopeUiBridge;
use expressions::ExpressionsUiBridge;
use external_param::ExternalParamUiBridge;
use harmonic_editor::HarmonicEditorUiBridge;
use lfo::LfoUiBridge;
use mixer::MixerUiBridge;
use mod_map::ModMapUiBridge;
use multi_envelope::MultiEnvelopeUiBridge;
use oscillator::OscillatorUiBridge;
use pitch::PitchUiBridge;
use segments_envelope::SegmentsEnvelopeUiBridge;
use spectral_band_select::SpectralBandSelectUiBridge;
use spectral_blend::SpectralBlendUiBridge;
use spectral_eq::SpectralEqUiBridge;
use spectral_filter::SpectralFilterUiBridge;
use spectral_mixer::SpectralMixerUiBridge;
use spectral_noise::SpectralNoiseUiBridge;
use svf::SvfUiBridge;
use wave_shaper::WaveShaperUiBridge;

pub fn module_default_label(module_type: ModuleType) -> &'static str {
    match module_type {
        ModuleType::Output => "Output",
        ModuleType::Amplifier => "Amplifier",
        ModuleType::Envelope => "Envelope",
        ModuleType::Mixer => "Mixer",
        ModuleType::Oscillator => "Oscillator",
        ModuleType::SpectralFilter => "Spectral Filter",
        ModuleType::SpectralEq => "Spectral EQ",
        ModuleType::SpectralBandSelect => "Band Select",
        ModuleType::SpectralBlend => "Spectral Blend",
        ModuleType::SpectralMixer => "Spectral Mixer",
        ModuleType::HarmonicEditor => "Harmonics",
        ModuleType::SpectralNoise => "Spectral Noise",
        ModuleType::ExternalParam => "Ext Param",
        ModuleType::Lfo => "LFO",
        ModuleType::ModMap => "Mod Map",
        ModuleType::MultiEnvelope => "Multi Envelope",
        ModuleType::SegmentsEnvelope => "Seg Envelope",
        ModuleType::Pitch => "Pitch",
        ModuleType::WaveShaper => "Waveshaper",
        ModuleType::Svf => "SVF",
        ModuleType::Expressions => "Expressions",
    }
}

#[enum_dispatch(ModuleUiBridge)]
pub enum ModuleBridge<E: EngineLinks = crate::links::PluginLinks> {
    Oscillator(Box<OscillatorUiBridge<E::Oscillator>>),
    Envelope(Box<EnvelopeUiBridge<E::Envelope>>),
    Amplifier(Box<AmplifierUiBridge<E::Amplifier>>),
    Lfo(Box<LfoUiBridge<E::Lfo>>),
    ModMap(Box<ModMapUiBridge<E::ModMap>>),
    MultiEnvelope(Box<MultiEnvelopeUiBridge<E::MultiEnvelope>>),
    SegmentsEnvelope(Box<SegmentsEnvelopeUiBridge<E::SegmentsEnvelope>>),
    Pitch(Box<PitchUiBridge<E::Pitch>>),
    Mixer(Box<MixerUiBridge<E::Mixer>>),
    WaveShaper(Box<WaveShaperUiBridge<E::WaveShaper>>),
    Svf(Box<SvfUiBridge<E::Svf>>),
    SpectralFilter(Box<SpectralFilterUiBridge<E::SpectralFilter>>),
    SpectralEq(Box<SpectralEqUiBridge<E::SpectralEq>>),
    SpectralBandSelect(Box<SpectralBandSelectUiBridge<E::SpectralBandSelect>>),
    SpectralBlend(Box<SpectralBlendUiBridge<E::SpectralBlend>>),
    SpectralMixer(Box<SpectralMixerUiBridge<E::SpectralMixer>>),
    HarmonicEditor(Box<HarmonicEditorUiBridge<E::HarmonicEditor>>),
    SpectralNoise(Box<SpectralNoiseUiBridge<E::SpectralNoise>>),
    Expressions(Box<ExpressionsUiBridge<E::Expressions>>),
    ExternalParam(Box<ExternalParamUiBridge<E::ExternalParam>>),
}

impl<E: EngineLinks> ModuleBridge<E> {
    pub(super) fn new(link: ModuleUiLink<E>) -> Self {
        match link {
            ModuleUiLink::Oscillator(link) => {
                let link = *link;
                Self::Oscillator(Box::new(OscillatorUiBridge::new(link.ui_end, link.config)))
            }
            ModuleUiLink::Envelope(link) => {
                let link = *link;
                Self::Envelope(Box::new(EnvelopeUiBridge::new(link.ui_end, link.config)))
            }
            ModuleUiLink::Lfo(link) => {
                let link = *link;
                Self::Lfo(Box::new(LfoUiBridge::new(link.ui_end, link.config)))
            }
            ModuleUiLink::ModMap(link) => {
                let link = *link;
                Self::ModMap(Box::new(ModMapUiBridge::new(link.ui_end, link.config)))
            }
            ModuleUiLink::MultiEnvelope(link) => {
                let link = *link;
                Self::MultiEnvelope(Box::new(MultiEnvelopeUiBridge::new(
                    link.ui_end,
                    link.config,
                )))
            }
            ModuleUiLink::SegmentsEnvelope(link) => {
                let link = *link;
                Self::SegmentsEnvelope(Box::new(SegmentsEnvelopeUiBridge::new(
                    link.ui_end,
                    link.config,
                )))
            }
            ModuleUiLink::Pitch(link) => {
                let link = *link;
                Self::Pitch(Box::new(PitchUiBridge::new(link.ui_end, link.config)))
            }
            ModuleUiLink::Amplifier(link) => {
                let link = *link;
                Self::Amplifier(Box::new(AmplifierUiBridge::new(link.ui_end, link.config)))
            }
            ModuleUiLink::Mixer(link) => {
                let link = *link;
                Self::Mixer(Box::new(MixerUiBridge::new(link.ui_end, link.config)))
            }
            ModuleUiLink::WaveShaper(link) => {
                let link = *link;
                Self::WaveShaper(Box::new(WaveShaperUiBridge::new(link.ui_end, link.config)))
            }
            ModuleUiLink::Svf(link) => {
                let link = *link;
                Self::Svf(Box::new(SvfUiBridge::new(link.ui_end, link.config)))
            }
            ModuleUiLink::SpectralFilter(link) => {
                let link = *link;
                Self::SpectralFilter(Box::new(SpectralFilterUiBridge::new(
                    link.ui_end,
                    link.config,
                )))
            }
            ModuleUiLink::SpectralEq(link) => {
                let link = *link;
                Self::SpectralEq(Box::new(SpectralEqUiBridge::new(link.ui_end, link.config)))
            }
            ModuleUiLink::SpectralBandSelect(link) => {
                let link = *link;
                Self::SpectralBandSelect(Box::new(SpectralBandSelectUiBridge::new(
                    link.ui_end,
                    link.config,
                )))
            }
            ModuleUiLink::SpectralBlend(link) => {
                let link = *link;
                Self::SpectralBlend(Box::new(SpectralBlendUiBridge::new(
                    link.ui_end,
                    link.config,
                )))
            }
            ModuleUiLink::SpectralMixer(link) => {
                let link = *link;
                Self::SpectralMixer(Box::new(SpectralMixerUiBridge::new(
                    link.ui_end,
                    link.config,
                )))
            }
            ModuleUiLink::HarmonicEditor(link) => {
                let link = *link;
                Self::HarmonicEditor(Box::new(HarmonicEditorUiBridge::new(
                    link.ui_end,
                    link.config,
                )))
            }
            ModuleUiLink::SpectralNoise(link) => {
                let link = *link;
                Self::SpectralNoise(Box::new(SpectralNoiseUiBridge::new(
                    link.ui_end,
                    link.config,
                )))
            }
            ModuleUiLink::Expressions(link) => {
                let link = *link;
                Self::Expressions(Box::new(ExpressionsUiBridge::new(link.ui_end, link.config)))
            }
            ModuleUiLink::ExternalParam(link) => {
                let link = *link;
                Self::ExternalParam(Box::new(ExternalParamUiBridge::new(
                    link.ui_end,
                    link.config,
                )))
            }
        }
    }
}

#[enum_dispatch]
#[auto_impl::auto_impl(Box)]
pub trait ModuleUiBridge: Send {
    fn id(&self) -> ModuleId;

    // Return true when routing needs to be update
    fn update(&mut self) -> bool;
}
