use crate::synth_engine::{
    Amplifier, Envelope, Expressions, ExternalParam, HarmonicEditor, Input, Lfo, Mixer,
    ModuleConfig, ModuleId, Oscillator, Pitch, SpectralBandSelect, SpectralBlend, SpectralEq,
    SpectralFilter, SpectralMixer, SpectralNoise, StereoSample, Svf, VoiceEvent, WaveShaper,
    engine_io::EngineLinks,
    modules::Output,
    routing::{DataType, InputMeta, InputSlots, ProcessContext, SpectralInputSlot},
    synth_module::SynthModule,
    voices_handler::DecayingVoice,
};
use enum_dispatch::enum_dispatch;

#[derive(Debug, Clone, Copy)]
pub enum ModuleType {
    Output,
    Envelope,
    Amplifier,
    Mixer,
    Oscillator,
    SpectralFilter,
    SpectralEq,
    SpectralBandSelect,
    SpectralBlend,
    SpectralMixer,
    HarmonicEditor,
    SpectralNoise,
    ExternalParam,
    Lfo,
    Pitch,
    WaveShaper,
    Svf,
    Expressions,
}

impl ModuleType {
    pub fn label(self) -> &'static str {
        match self {
            Self::Output => "Output",
            Self::Envelope => "Envelope",
            Self::Amplifier => "Amplifier",
            Self::Mixer => "Mixer",
            Self::Oscillator => "Oscillator",
            Self::SpectralFilter => "Spectral Filter",
            Self::SpectralEq => "Spectral EQ",
            Self::SpectralBandSelect => "Band Select",
            Self::SpectralBlend => "Spectral Blend",
            Self::SpectralMixer => "Spectral Mixer",
            Self::HarmonicEditor => "Harmonic Editor",
            Self::SpectralNoise => "Spectral Noise",
            Self::ExternalParam => "Ext Parameter",
            Self::Lfo => "LFO",
            Self::Pitch => "Pitch",
            Self::WaveShaper => "Waveshaper",
            Self::Svf => "SVF",
            Self::Expressions => "Expressions",
        }
    }
}

#[enum_dispatch(SynthModule)]
pub enum ModuleHandle<E: EngineLinks = crate::synth_engine::stub::StubLinks> {
    Oscillator(Box<Oscillator<E::Oscillator>>),
    Envelope(Box<Envelope<E::Envelope>>),
    Lfo(Box<Lfo<E::Lfo>>),
    Pitch(Box<Pitch<E::Pitch>>),
    Amplifier(Box<Amplifier<E::Amplifier>>),
    WaveShaper(Box<WaveShaper<E::WaveShaper>>),
    Svf(Box<Svf<E::Svf>>),
    Mixer(Box<Mixer<E::Mixer>>),
    SpectralFilter(Box<SpectralFilter<E::SpectralFilter>>),
    SpectralEq(Box<SpectralEq<E::SpectralEq>>),
    SpectralBandSelect(Box<SpectralBandSelect<E::SpectralBandSelect>>),
    SpectralBlend(Box<SpectralBlend<E::SpectralBlend>>),
    SpectralMixer(Box<SpectralMixer<E::SpectralMixer>>),
    HarmonicEditor(Box<HarmonicEditor<E::HarmonicEditor>>),
    SpectralNoise(Box<SpectralNoise<E::SpectralNoise>>),
    Expressions(Box<Expressions<E::Expressions>>),
    ExternalParam(Box<ExternalParam<E::ExternalParam>>),
    Output(Box<Output>),
}

impl<E: EngineLinks> ModuleHandle<E> {
    pub(super) fn process(&mut self, ctx: &mut ProcessContext<E::AudioEnd>) {
        match self {
            Self::Oscillator(m) => m.process(ctx),
            Self::Envelope(m) => m.process(ctx),
            Self::Lfo(m) => m.process(ctx),
            Self::Pitch(m) => m.process(ctx),
            Self::Amplifier(m) => m.process(ctx),
            Self::WaveShaper(m) => m.process(ctx),
            Self::Svf(m) => m.process(ctx),
            Self::Mixer(m) => m.process(ctx),
            Self::SpectralFilter(m) => m.process(ctx),
            Self::SpectralEq(m) => m.process(ctx),
            Self::SpectralBandSelect(m) => m.process(ctx),
            Self::SpectralBlend(m) => m.process(ctx),
            Self::SpectralMixer(m) => m.process(ctx),
            Self::HarmonicEditor(m) => m.process(ctx),
            Self::SpectralNoise(m) => m.process(ctx),
            Self::Expressions(m) => m.process(ctx),
            Self::ExternalParam(m) => m.process(ctx),
            Self::Output(m) => m.process(ctx),
        }
    }

    pub(crate) fn module_type(&self) -> ModuleType {
        match self {
            Self::Output(_) => ModuleType::Output,
            Self::Oscillator(_) => ModuleType::Oscillator,
            Self::Envelope(_) => ModuleType::Envelope,
            Self::Lfo(_) => ModuleType::Lfo,
            Self::Pitch(_) => ModuleType::Pitch,
            Self::Amplifier(_) => ModuleType::Amplifier,
            Self::Mixer(_) => ModuleType::Mixer,
            Self::WaveShaper(_) => ModuleType::WaveShaper,
            Self::Svf(_) => ModuleType::Svf,
            Self::SpectralFilter(_) => ModuleType::SpectralFilter,
            Self::SpectralEq(_) => ModuleType::SpectralEq,
            Self::SpectralBandSelect(_) => ModuleType::SpectralBandSelect,
            Self::SpectralBlend(_) => ModuleType::SpectralBlend,
            Self::SpectralMixer(_) => ModuleType::SpectralMixer,
            Self::HarmonicEditor(_) => ModuleType::HarmonicEditor,
            Self::SpectralNoise(_) => ModuleType::SpectralNoise,
            Self::Expressions(_) => ModuleType::Expressions,
            Self::ExternalParam(_) => ModuleType::ExternalParam,
        }
    }

    pub(super) fn new(module_type: ModuleType, id: ModuleId) -> Self {
        match module_type {
            ModuleType::Output => unreachable!("output is created with the engine"),
            ModuleType::Oscillator => Self::Oscillator(Box::new(Oscillator::new(id))),
            ModuleType::Envelope => Self::Envelope(Box::new(Envelope::new(id))),
            ModuleType::Lfo => Self::Lfo(Box::new(Lfo::new(id))),
            ModuleType::Pitch => Self::Pitch(Box::new(Pitch::new(id))),
            ModuleType::Amplifier => Self::Amplifier(Box::new(Amplifier::new(id))),
            ModuleType::Mixer => Self::Mixer(Box::new(Mixer::new(id))),
            ModuleType::WaveShaper => Self::WaveShaper(Box::new(WaveShaper::new(id))),
            ModuleType::Svf => Self::Svf(Box::new(Svf::new(id))),
            ModuleType::SpectralFilter => Self::SpectralFilter(Box::new(SpectralFilter::new(id))),
            ModuleType::SpectralEq => Self::SpectralEq(Box::new(SpectralEq::new(id))),
            ModuleType::SpectralBandSelect => {
                Self::SpectralBandSelect(Box::new(SpectralBandSelect::new(id)))
            }
            ModuleType::SpectralBlend => Self::SpectralBlend(Box::new(SpectralBlend::new(id))),
            ModuleType::SpectralMixer => Self::SpectralMixer(Box::new(SpectralMixer::new(id))),
            ModuleType::HarmonicEditor => Self::HarmonicEditor(Box::new(HarmonicEditor::new(id))),
            ModuleType::SpectralNoise => Self::SpectralNoise(Box::new(SpectralNoise::new(id))),
            ModuleType::Expressions => Self::Expressions(Box::new(Expressions::new(id))),
            ModuleType::ExternalParam => Self::ExternalParam(Box::new(ExternalParam::new(id))),
        }
    }

    pub(super) fn from_config(module_cfg: &ModuleConfig) -> Self {
        match module_cfg {
            ModuleConfig::Oscillator(cfg) => {
                Self::Oscillator(Box::new(Oscillator::from_config(cfg)))
            }
            ModuleConfig::Envelope(cfg) => Self::Envelope(Box::new(Envelope::from_config(cfg))),
            ModuleConfig::Lfo(cfg) => Self::Lfo(Box::new(Lfo::from_config(cfg))),
            ModuleConfig::Pitch(cfg) => Self::Pitch(Box::new(Pitch::from_config(cfg))),
            ModuleConfig::Amplifier(cfg) => Self::Amplifier(Box::new(Amplifier::from_config(cfg))),
            ModuleConfig::Mixer(cfg) => Self::Mixer(Box::new(Mixer::from_config(cfg))),
            ModuleConfig::WaveShaper(cfg) => {
                Self::WaveShaper(Box::new(WaveShaper::from_config(cfg)))
            }
            ModuleConfig::Svf(cfg) => Self::Svf(Box::new(Svf::from_config(cfg))),
            ModuleConfig::SpectralFilter(cfg) => {
                Self::SpectralFilter(Box::new(SpectralFilter::from_config(cfg)))
            }
            ModuleConfig::SpectralEq(cfg) => {
                Self::SpectralEq(Box::new(SpectralEq::from_config(cfg)))
            }
            ModuleConfig::SpectralBandSelect(cfg) => {
                Self::SpectralBandSelect(Box::new(SpectralBandSelect::from_config(cfg)))
            }
            ModuleConfig::SpectralBlend(cfg) => {
                Self::SpectralBlend(Box::new(SpectralBlend::from_config(cfg)))
            }
            ModuleConfig::SpectralMixer(cfg) => {
                Self::SpectralMixer(Box::new(SpectralMixer::from_config(cfg)))
            }
            ModuleConfig::HarmonicEditor(cfg) => {
                Self::HarmonicEditor(Box::new(HarmonicEditor::from_config(cfg)))
            }
            ModuleConfig::SpectralNoise(cfg) => {
                Self::SpectralNoise(Box::new(SpectralNoise::from_config(cfg)))
            }
            ModuleConfig::Expressions(cfg) => {
                Self::Expressions(Box::new(Expressions::from_config(cfg)))
            }
            ModuleConfig::ExternalParam(cfg) => {
                Self::ExternalParam(Box::new(ExternalParam::from_config(cfg)))
            }
        }
    }

    pub(super) fn config(&self) -> Option<ModuleConfig> {
        match self {
            Self::Output(_) => None,
            Self::Oscillator(m) => Some(ModuleConfig::Oscillator(Box::new(m.get_config()))),
            Self::Envelope(m) => Some(ModuleConfig::Envelope(Box::new(m.get_config()))),
            Self::Lfo(m) => Some(ModuleConfig::Lfo(Box::new(m.get_config()))),
            Self::Pitch(m) => Some(ModuleConfig::Pitch(Box::new(m.get_config()))),
            Self::Amplifier(m) => Some(ModuleConfig::Amplifier(Box::new(m.get_config()))),
            Self::Mixer(m) => Some(ModuleConfig::Mixer(Box::new(m.get_config()))),
            Self::WaveShaper(m) => Some(ModuleConfig::WaveShaper(Box::new(m.get_config()))),
            Self::Svf(m) => Some(ModuleConfig::Svf(Box::new(m.get_config()))),
            Self::SpectralFilter(m) => Some(ModuleConfig::SpectralFilter(Box::new(m.get_config()))),
            Self::SpectralEq(m) => Some(ModuleConfig::SpectralEq(Box::new(m.get_config()))),
            Self::SpectralBandSelect(m) => {
                Some(ModuleConfig::SpectralBandSelect(Box::new(m.get_config())))
            }
            Self::SpectralBlend(m) => Some(ModuleConfig::SpectralBlend(Box::new(m.get_config()))),
            Self::SpectralMixer(m) => Some(ModuleConfig::SpectralMixer(Box::new(m.get_config()))),
            Self::HarmonicEditor(m) => Some(ModuleConfig::HarmonicEditor(Box::new(m.get_config()))),
            Self::SpectralNoise(m) => Some(ModuleConfig::SpectralNoise(Box::new(m.get_config()))),
            Self::Expressions(m) => Some(ModuleConfig::Expressions(Box::new(m.get_config()))),
            Self::ExternalParam(m) => Some(ModuleConfig::ExternalParam(Box::new(m.get_config()))),
        }
    }
}
