use crate::synth_engine::{
    Input, InputId, ModuleId, ModuleType, NUM_CHANNELS, Smp, StereoSample,
    amplifier::{AmplifierConfig, AmplifierLinks},
    envelope::{EnvelopeConfig, EnvelopeLinks},
    expressions::{ExpressionsConfig, ExpressionsLinks},
    external_param::{ExternalParamConfig, ExternalParamLinks},
    harmonic_editor::{HarmonicEditorConfig, HarmonicEditorLinks},
    lfo::{LfoConfig, LfoLinks},
    mixer::{MixerConfig, MixerLinks},
    mod_map::{ModMapConfig, ModMapLinks},
    multi_envelope::{MultiEnvelopeConfig, MultiEnvelopeLinks},
    oscillator::{OscillatorConfig, OscillatorLinks},
    pitch::{PitchConfig, PitchLinks},
    routing_state::RoutingState,
    segments_envelope::{SegmentsEnvelopeConfig, SegmentsEnvelopeLinks},
    spectral_band_select::{SpectralBandSelectConfig, SpectralBandSelectLinks},
    spectral_blend::{SpectralBlendConfig, SpectralBlendLinks},
    spectral_eq::{SpectralEqConfig, SpectralEqLinks},
    spectral_filter::{SpectralFilterConfig, SpectralFilterLinks},
    spectral_mixer::{SpectralMixerConfig, SpectralMixerLinks},
    spectral_noise::{SpectralNoiseConfig, SpectralNoiseLinks},
    svf::{SvfConfig, SvfLinks},
    wave_shaper::{WaveShaperConfig, WaveShaperLinks},
};

pub use crate::synth_engine::voices_handler::VoicesTelemetry;

#[derive(Clone, Copy, Default)]
pub struct VoicesStatus {
    pub waiting_notes: u8,
    pub playing: u8,
    pub releasing: u8,
    pub killing: u8,
}

#[derive(Clone, Copy)]
pub struct OutputMeter {
    pub volume: StereoSample,
    pub clipped: [bool; NUM_CHANNELS],
}

impl Default for OutputMeter {
    fn default() -> Self {
        Self {
            volume: StereoSample::ZERO,
            clipped: [false; NUM_CHANNELS],
        }
    }
}

pub enum UiEvent {
    LinkAmount {
        src: ModuleId,
        dst: InputId,
        amount: StereoSample,
    },
    Voices(usize),
    Legato(bool),
    KillSameNote(bool),
    VoiceKillTime(Smp),
    OutputGain(StereoSample),
    AddModule {
        tag: i32,
        module_type: ModuleType,
    },
    DuplicateModule {
        src: ModuleId,
        tag: i32,
    },
    RemoveModule(ModuleId),
    SetDirectLink {
        src: ModuleId,
        dst: InputId,
    },
    AddMixedLink {
        src: ModuleId,
        dst: InputId,
        amount: StereoSample,
    },
    RemoveLink {
        src: ModuleId,
        dst: InputId,
    },
    RemoveInputLinks(InputId),
    RemoveOutputLinks(ModuleId),
    RefreshRouting,
}

/// `UiEnd` plus the module state the UI needs to build that module's bridge.
pub enum ModuleUiLink<E: EngineLinks> {
    Oscillator {
        ui_end: <E::Oscillator as OscillatorLinks>::UiEnd,
        config: Box<OscillatorConfig>,
    },
    Envelope {
        ui_end: <E::Envelope as EnvelopeLinks>::UiEnd,
        config: Box<EnvelopeConfig>,
    },
    Lfo {
        ui_end: <E::Lfo as LfoLinks>::UiEnd,
        config: Box<LfoConfig>,
    },
    ModMap {
        ui_end: <E::ModMap as ModMapLinks>::UiEnd,
        config: Box<ModMapConfig>,
    },
    MultiEnvelope {
        ui_end: <E::MultiEnvelope as MultiEnvelopeLinks>::UiEnd,
        config: Box<MultiEnvelopeConfig>,
    },
    SegmentsEnvelope {
        ui_end: <E::SegmentsEnvelope as SegmentsEnvelopeLinks>::UiEnd,
        config: Box<SegmentsEnvelopeConfig>,
    },
    Pitch {
        ui_end: <E::Pitch as PitchLinks>::UiEnd,
        config: Box<PitchConfig>,
    },
    Amplifier {
        ui_end: <E::Amplifier as AmplifierLinks>::UiEnd,
        config: Box<AmplifierConfig>,
    },
    Mixer {
        ui_end: <E::Mixer as MixerLinks>::UiEnd,
        config: Box<MixerConfig>,
    },
    WaveShaper {
        ui_end: <E::WaveShaper as WaveShaperLinks>::UiEnd,
        config: Box<WaveShaperConfig>,
    },
    Svf {
        ui_end: <E::Svf as SvfLinks>::UiEnd,
        config: Box<SvfConfig>,
    },
    SpectralFilter {
        ui_end: <E::SpectralFilter as SpectralFilterLinks>::UiEnd,
        config: Box<SpectralFilterConfig>,
    },
    SpectralEq {
        ui_end: <E::SpectralEq as SpectralEqLinks>::UiEnd,
        config: Box<SpectralEqConfig>,
    },
    SpectralBandSelect {
        ui_end: <E::SpectralBandSelect as SpectralBandSelectLinks>::UiEnd,
        config: Box<SpectralBandSelectConfig>,
    },
    SpectralBlend {
        ui_end: <E::SpectralBlend as SpectralBlendLinks>::UiEnd,
        config: Box<SpectralBlendConfig>,
    },
    SpectralMixer {
        ui_end: <E::SpectralMixer as SpectralMixerLinks>::UiEnd,
        config: Box<SpectralMixerConfig>,
    },
    HarmonicEditor {
        ui_end: <E::HarmonicEditor as HarmonicEditorLinks>::UiEnd,
        config: Box<HarmonicEditorConfig>,
    },
    SpectralNoise {
        ui_end: <E::SpectralNoise as SpectralNoiseLinks>::UiEnd,
        config: Box<SpectralNoiseConfig>,
    },
    Expressions {
        ui_end: <E::Expressions as ExpressionsLinks>::UiEnd,
        config: Box<ExpressionsConfig>,
    },
    ExternalParam {
        ui_end: <E::ExternalParam as ExternalParamLinks>::UiEnd,
        config: Box<ExternalParamConfig>,
    },
}

impl<E: EngineLinks> ModuleUiLink<E> {
    pub fn id(&self) -> ModuleId {
        match self {
            Self::Oscillator { config, .. } => config.id,
            Self::Envelope { config, .. } => config.id,
            Self::Lfo { config, .. } => config.id,
            Self::ModMap { config, .. } => config.id,
            Self::MultiEnvelope { config, .. } => config.id,
            Self::SegmentsEnvelope { config, .. } => config.id,
            Self::Pitch { config, .. } => config.id,
            Self::Amplifier { config, .. } => config.id,
            Self::Mixer { config, .. } => config.id,
            Self::WaveShaper { config, .. } => config.id,
            Self::Svf { config, .. } => config.id,
            Self::SpectralFilter { config, .. } => config.id,
            Self::SpectralEq { config, .. } => config.id,
            Self::SpectralBandSelect { config, .. } => config.id,
            Self::SpectralBlend { config, .. } => config.id,
            Self::SpectralMixer { config, .. } => config.id,
            Self::HarmonicEditor { config, .. } => config.id,
            Self::SpectralNoise { config, .. } => config.id,
            Self::Expressions { config, .. } => config.id,
            Self::ExternalParam { config, .. } => config.id,
        }
    }
}

pub enum UiUpdate<M> {
    ModulatedInput {
        module_id: ModuleId,
        input: Input,
        channel: u8,
        value: Smp,
        normalized_value: Smp,
    },
    VoicesStatus(VoicesStatus),
    /// `RoutingState` was published on the triple buffer before this event.
    /// `tag` is the UI request tag for an added module, so the UI can attach
    /// the grid position it stored for that request.
    Topology {
        added: Option<M>,
        removed: Option<ModuleId>,
        tag: Option<i32>,
    },
}

pub trait EngineAudioEnd: Send {
    type ModuleUi;

    fn update_modulated_input(
        &mut self,
        module_id: ModuleId,
        input: Input,
        channel: u8,
        value: Smp,
        normalized_value: Smp,
    ) -> bool;

    fn update_voices_status(&mut self, telemetry: &VoicesTelemetry) -> bool;

    fn pop_event(&mut self) -> Option<UiEvent>;

    fn update_out_volume(&mut self, volume: StereoSample, clipped: [bool; NUM_CHANNELS]);

    fn publish_routing(&mut self, routing: RoutingState);

    fn publish_topology(
        &mut self,
        added: Option<Self::ModuleUi>,
        removed: Option<ModuleId>,
        tag: Option<i32>,
    ) -> bool;
}

pub trait EngineUiEnd: Send {
    type ModuleUi;

    fn get_out_volume(&mut self) -> OutputMeter;

    fn set_link_amount(&mut self, src: ModuleId, dst: InputId, amount: StereoSample) -> bool;

    fn set_voices(&mut self, voices: usize) -> bool;

    fn set_legato(&mut self, legato: bool) -> bool;

    fn set_kill_same_note(&mut self, kill_same_note: bool) -> bool;

    fn set_voice_kill_time(&mut self, voice_kill_time: Smp) -> bool;

    fn set_output_gain(&mut self, output_gain: StereoSample) -> bool;

    fn send_event(&mut self, event: UiEvent) -> bool;

    fn read_routing(&mut self) -> &RoutingState;

    fn pop_update(&mut self) -> Option<UiUpdate<Self::ModuleUi>>;
}

pub trait EngineLinks: Sized + Send {
    type AudioEnd: EngineAudioEnd<ModuleUi = ModuleUiLink<Self>>;
    type UiEnd: EngineUiEnd<ModuleUi = ModuleUiLink<Self>>;
    type Amplifier: AmplifierLinks<EngineEnd = Self::AudioEnd>;
    type Envelope: EnvelopeLinks<EngineEnd = Self::AudioEnd>;
    type Expressions: ExpressionsLinks<EngineEnd = Self::AudioEnd>;
    type ExternalParam: ExternalParamLinks<EngineEnd = Self::AudioEnd>;
    type HarmonicEditor: HarmonicEditorLinks<EngineEnd = Self::AudioEnd>;
    type Lfo: LfoLinks<EngineEnd = Self::AudioEnd>;
    type ModMap: ModMapLinks<EngineEnd = Self::AudioEnd>;
    type MultiEnvelope: MultiEnvelopeLinks<EngineEnd = Self::AudioEnd>;
    type SegmentsEnvelope: SegmentsEnvelopeLinks<EngineEnd = Self::AudioEnd>;
    type Mixer: MixerLinks<EngineEnd = Self::AudioEnd>;
    type Oscillator: OscillatorLinks<EngineEnd = Self::AudioEnd>;
    type Pitch: PitchLinks<EngineEnd = Self::AudioEnd>;
    type SpectralBandSelect: SpectralBandSelectLinks<EngineEnd = Self::AudioEnd>;
    type SpectralBlend: SpectralBlendLinks<EngineEnd = Self::AudioEnd>;
    type SpectralEq: SpectralEqLinks<EngineEnd = Self::AudioEnd>;
    type SpectralFilter: SpectralFilterLinks<EngineEnd = Self::AudioEnd>;
    type SpectralMixer: SpectralMixerLinks<EngineEnd = Self::AudioEnd>;
    type SpectralNoise: SpectralNoiseLinks<EngineEnd = Self::AudioEnd>;
    type Svf: SvfLinks<EngineEnd = Self::AudioEnd>;
    type WaveShaper: WaveShaperLinks<EngineEnd = Self::AudioEnd>;

    fn create_link_pair() -> (Self::AudioEnd, Option<Self::UiEnd>);
}
