use crate::synth_engine::{
    Input, InputId, ModuleId, ModuleType, NUM_CHANNELS, Smp, StereoSample,
    amplifier::{AmplifierLinks, AmplifierUiLink},
    envelope::{EnvelopeLinks, EnvelopeUiLink},
    expressions::{ExpressionsLinks, ExpressionsUiLink},
    external_param::{ExternalParamLinks, ExternalParamUiLink},
    harmonic_editor::{HarmonicEditorLinks, HarmonicEditorUiLink},
    lfo::{LfoLinks, LfoUiLink},
    mixer::{MixerLinks, MixerUiLink},
    mod_map::{ModMapLinks, ModMapUiLink},
    multi_envelope::{MultiEnvelopeLinks, MultiEnvelopeUiLink},
    oscillator::{OscillatorLinks, OscillatorUiLink},
    pitch::{PitchLinks, PitchUiLink},
    routing_state::RoutingState,
    segments_envelope::{SegmentsEnvelopeLinks, SegmentsEnvelopeUiLink},
    spectral_band_select::{SpectralBandSelectLinks, SpectralBandSelectUiLink},
    spectral_blend::{SpectralBlendLinks, SpectralBlendUiLink},
    spectral_eq::{SpectralEqLinks, SpectralEqUiLink},
    spectral_filter::{SpectralFilterLinks, SpectralFilterUiLink},
    spectral_mixer::{SpectralMixerLinks, SpectralMixerUiLink},
    spectral_noise::{SpectralNoiseLinks, SpectralNoiseUiLink},
    svf::{SvfLinks, SvfUiLink},
    wave_shaper::{WaveShaperLinks, WaveShaperUiLink},
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
    Oscillator(Box<OscillatorUiLink<<E::Oscillator as OscillatorLinks>::UiEnd>>),
    Envelope(Box<EnvelopeUiLink<<E::Envelope as EnvelopeLinks>::UiEnd>>),
    Lfo(Box<LfoUiLink<<E::Lfo as LfoLinks>::UiEnd>>),
    ModMap(Box<ModMapUiLink<<E::ModMap as ModMapLinks>::UiEnd>>),
    MultiEnvelope(Box<MultiEnvelopeUiLink<<E::MultiEnvelope as MultiEnvelopeLinks>::UiEnd>>),
    SegmentsEnvelope(
        Box<SegmentsEnvelopeUiLink<<E::SegmentsEnvelope as SegmentsEnvelopeLinks>::UiEnd>>,
    ),
    Pitch(Box<PitchUiLink<<E::Pitch as PitchLinks>::UiEnd>>),
    Amplifier(Box<AmplifierUiLink<<E::Amplifier as AmplifierLinks>::UiEnd>>),
    Mixer(Box<MixerUiLink<<E::Mixer as MixerLinks>::UiEnd>>),
    WaveShaper(Box<WaveShaperUiLink<<E::WaveShaper as WaveShaperLinks>::UiEnd>>),
    Svf(Box<SvfUiLink<<E::Svf as SvfLinks>::UiEnd>>),
    SpectralFilter(Box<SpectralFilterUiLink<<E::SpectralFilter as SpectralFilterLinks>::UiEnd>>),
    SpectralEq(Box<SpectralEqUiLink<<E::SpectralEq as SpectralEqLinks>::UiEnd>>),
    SpectralBandSelect(
        Box<SpectralBandSelectUiLink<<E::SpectralBandSelect as SpectralBandSelectLinks>::UiEnd>>,
    ),
    SpectralBlend(Box<SpectralBlendUiLink<<E::SpectralBlend as SpectralBlendLinks>::UiEnd>>),
    SpectralMixer(Box<SpectralMixerUiLink<<E::SpectralMixer as SpectralMixerLinks>::UiEnd>>),
    HarmonicEditor(Box<HarmonicEditorUiLink<<E::HarmonicEditor as HarmonicEditorLinks>::UiEnd>>),
    SpectralNoise(Box<SpectralNoiseUiLink<<E::SpectralNoise as SpectralNoiseLinks>::UiEnd>>),
    Expressions(Box<ExpressionsUiLink<<E::Expressions as ExpressionsLinks>::UiEnd>>),
    ExternalParam(Box<ExternalParamUiLink<<E::ExternalParam as ExternalParamLinks>::UiEnd>>),
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
    Routing,
    /// `RoutingState` was published on the triple buffer before this event.
    /// `tag` is the UI request tag for the added module, so the UI can attach
    /// the grid position it stored for that request.
    ModuleAdded {
        module: M,
        tag: Option<i32>,
    },
    /// `RoutingState` was published on the triple buffer before this event.
    ModuleRemoved(ModuleId),
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

    fn publish_routing_update(&mut self);

    fn publish_module_added(&mut self, module: Self::ModuleUi, tag: Option<i32>);

    /// Retry events that did not fit in the audio-to-UI ring.
    fn flush_stuck_events(&mut self) {}

    fn publish_module_removed(&mut self, module_id: ModuleId);
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
