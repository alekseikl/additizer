use crate::synth_engine::{
    Input, InputId, ModuleId, NUM_CHANNELS, Sample, StereoSample, amplifier::AmplifierLinks,
    envelope::EnvelopeLinks, expressions::ExpressionsLinks, external_param::ExternalParamLinks,
    harmonic_editor::HarmonicEditorLinks, lfo::LfoLinks, mixer::MixerLinks,
    oscillator::OscillatorLinks, pitch::PitchLinks, spectral_band_select::SpectralBandSelectLinks,
    spectral_blend::SpectralBlendLinks, spectral_eq::SpectralEqLinks,
    spectral_filter::SpectralFilterLinks, spectral_mixer::SpectralMixerLinks,
    spectral_noise::SpectralNoiseLinks, svf::SvfLinks, wave_shaper::WaveShaperLinks,
};

pub use crate::synth_engine::voices_handler::VoicesHandlerMetrics;

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
    BlockSize(usize),
    VoiceKillTime(Sample),
    Oversampling(bool),
    OutputGain(StereoSample),
}

pub enum UiUpdate {
    ModulatedInput {
        module_id: ModuleId,
        input: Input,
        channel: u8,
        value: Sample,
        normalized_value: Sample,
    },
    VoicesStatus(VoicesStatus),
}

pub trait EngineAudioEnd: Send {
    fn update_modulated_input(
        &mut self,
        module_id: ModuleId,
        input: Input,
        channel: u8,
        value: Sample,
        normalized_value: Sample,
    ) -> bool;

    fn update_voices_status(&mut self, metrics: &VoicesHandlerMetrics) -> bool;

    fn pop_event(&mut self) -> Option<UiEvent>;

    fn update_out_volume(&mut self, volume: StereoSample, clipped: [bool; NUM_CHANNELS]);
}

pub trait EngineUiEnd: Send {
    fn get_out_volume(&mut self) -> OutputMeter;

    fn set_link_amount(&mut self, src: ModuleId, dst: InputId, amount: StereoSample) -> bool;

    fn set_voices(&mut self, voices: usize) -> bool;

    fn set_legato(&mut self, legato: bool) -> bool;

    fn set_block_size(&mut self, block_size: usize) -> bool;

    fn set_voice_kill_time(&mut self, voice_kill_time: Sample) -> bool;

    fn set_oversampling(&mut self, oversampling: bool) -> bool;

    fn set_output_gain(&mut self, output_gain: StereoSample) -> bool;

    fn pop_update(&mut self) -> Option<UiUpdate>;
}

pub trait EngineLinks: Send {
    type AudioEnd: EngineAudioEnd;
    type UiEnd: EngineUiEnd;
    type Amplifier: AmplifierLinks<EngineEnd = Self::AudioEnd>;
    type Envelope: EnvelopeLinks<EngineEnd = Self::AudioEnd>;
    type Expressions: ExpressionsLinks<EngineEnd = Self::AudioEnd>;
    type ExternalParam: ExternalParamLinks<EngineEnd = Self::AudioEnd>;
    type HarmonicEditor: HarmonicEditorLinks<EngineEnd = Self::AudioEnd>;
    type Lfo: LfoLinks<EngineEnd = Self::AudioEnd>;
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

    fn create_link_pair() -> (Self::AudioEnd, Self::UiEnd);
}
