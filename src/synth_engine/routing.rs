use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

use crate::synth_engine::{Sample, StereoSample, config::LinkConfig};

mod outputs;
mod outputs_arena;
mod process_context;
mod voice_router;

pub use outputs::{SamplesOutput, SpectralOutput};
pub use outputs_arena::OutputsArena;
pub use process_context::{ProcessContext, ProcessParams, VoiceTarget};
pub use voice_router::{
    AudioRouterType, ControlRouterType, OutputRouterType, RouterFactory, SpectralRouterType,
    VoiceRouter,
};

pub type ModuleId = i32;

pub const MAX_VOICES: usize = 20;
pub const NUM_CHANNELS: usize = 2;
pub const OUTPUT_MODULE_ID: ModuleId = 0;
pub const MIN_MODULE_ID: ModuleId = 1;

pub const LEFT_CHANNEL: usize = 0;
pub const RIGHT_CHANNEL: usize = 1;

pub const UI_TO_AUDIO_RING_CAPACITY: usize = 64;
pub const AUDIO_TO_UI_RING_CAPACITY: usize = 64;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum DataType {
    Audio,
    Control,
    Spectral,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash, Serialize, Deserialize)]
pub enum Input {
    Audio,
    AudioMix(u8),
    Gain, // 0.0 - 1.0
    GainMix(u8),
    Level,         // dB
    LevelMix(u8),  // dB
    Pan,           // [-1.0, 1.0]
    Distortion,    // dB
    ClippingLevel, // dB
    PitchShift,
    Detune,
    DetunePower,
    Glide,
    GlideSlope,
    PhaseShift,
    PhaseSteal, // [0.0, 1.0]
    FrequencyShift,
    Spectrum,
    SpectrumMix(u8),
    SpectrumTo,
    Blend,
    PhasesBlend,
    GainsBlend,
    LowFrequency,
    Cutoff,
    Resonance,
    Drive, // dB
    Skew,
    Delay,
    Attack,
    Hold,
    Decay,
    Sustain,
    Release,
}

#[derive(Clone, Copy)]
pub struct InputMeta {
    pub input_type: Input,
    pub data_type: DataType,
    pub is_direct: bool,
}

impl InputMeta {
    pub const fn direct_audio(input: Input) -> Self {
        Self {
            input_type: input,
            data_type: DataType::Audio,
            is_direct: true,
        }
    }

    pub const fn audio(input: Input) -> Self {
        Self {
            input_type: input,
            data_type: DataType::Audio,
            is_direct: false,
        }
    }

    pub const fn control(input: Input) -> Self {
        Self {
            input_type: input,
            data_type: DataType::Control,
            is_direct: false,
        }
    }

    pub const fn spectral(input: Input) -> Self {
        Self {
            input_type: input,
            data_type: DataType::Spectral,
            is_direct: true,
        }
    }
}

#[derive(Debug, Default, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
pub enum Expression {
    #[default]
    Velocity, // [0, 1]
    Gain,     // voltage gain ratio, 1.0 = unity
    Pan,      // [-1, 1]
    Pitch,    // semitones, [-128, 128]
    Timbre,   // [0, 1]
    Pressure, // [0, 1]
}

#[derive(Debug)]
pub enum VoiceEvent {
    Reset {
        voice_idx: usize,
        prev_voice_idx: Option<usize>,
        pitch: Sample,
        velocity: Sample,
        offset: usize, // In-block sample offset
    },
    Update {
        voice_idx: usize,
        pitch: Sample,
        velocity: Sample,
        offset: usize, // In-block sample offset
    },
    Release {
        voice_idx: usize,
        velocity: Sample,
        offset: usize, // In-block sample offset
    },
    Kill {
        voice_idx: usize,
        offset: usize, // In-block sample offset
    },
}

pub struct ExpressionEvent {
    pub voice_idx: usize,
    pub expression: Expression,
    pub offset: usize, // In-block sample offset
    pub value: Sample,
}

#[derive(Default, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum VolumeType {
    #[default]
    Gain,
    Db,
}

impl VolumeType {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Db => "dB",
            Self::Gain => "Gain",
        }
    }
}

#[derive(Default, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum MixType {
    #[default]
    Add,
    Subtract,
    Multiply,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash, Serialize, Deserialize)]
pub struct InputId {
    pub input_type: Input,
    pub module_id: ModuleId,
}

impl InputId {
    pub fn new(input: Input, id: ModuleId) -> Self {
        Self {
            input_type: input,
            module_id: id,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub(super) enum ModuleLink {
    Direct {
        src: ModuleId,
        dst: InputId,
    },
    Mixed {
        src: ModuleId,
        dst: InputId,
        amount: StereoSample,
        modulation: Option<ModuleId>,
    },
}

impl ModuleLink {
    pub fn direct(src: ModuleId, dst: InputId) -> Self {
        Self::Direct { src, dst }
    }

    pub fn mixed(src: ModuleId, dst: InputId, amount: impl Into<StereoSample>) -> Self {
        Self::Mixed {
            src,
            dst,
            amount: amount.into(),
            modulation: None,
        }
    }

    pub fn mixed_modulated(
        src: ModuleId,
        dst: InputId,
        amount: impl Into<StereoSample>,
        modulation: Option<ModuleId>,
    ) -> Self {
        Self::Mixed {
            src,
            dst,
            amount: amount.into(),
            modulation,
        }
    }

    pub fn is_direct(&self) -> bool {
        matches!(self, Self::Direct { .. })
    }

    pub fn src(&self) -> ModuleId {
        match self {
            Self::Direct { src, .. } | Self::Mixed { src, .. } => *src,
        }
    }

    pub fn dst(&self) -> InputId {
        match self {
            Self::Direct { dst, .. } | Self::Mixed { dst, .. } => *dst,
        }
    }

    pub fn modulation(&self) -> Option<ModuleId> {
        match self {
            Self::Direct { .. } => None,
            Self::Mixed { modulation, .. } => *modulation,
        }
    }

    pub fn clear_modulation(&mut self) {
        if self.is_direct() {
            return;
        }

        if let Self::Mixed { modulation, .. } = self {
            *modulation = None;
        }
    }

    pub fn set_modulation(&mut self, modulator_id: ModuleId) -> bool {
        if self.is_direct() {
            return false;
        }

        if let Self::Mixed { modulation, .. } = self {
            *modulation = Some(modulator_id);
        }

        true
    }

    pub fn from_config(config: &LinkConfig) -> Self {
        match *config {
            LinkConfig::Direct {
                src_id,
                dst_id,
                dst_input,
            } => Self::direct(src_id, InputId::new(dst_input, dst_id)),
            LinkConfig::Mixed {
                src_id,
                dst_id,
                dst_input,
                amount,
                modulator_id,
            } => Self::mixed_modulated(
                src_id,
                InputId::new(dst_input, dst_id),
                amount,
                modulator_id,
            ),
        }
    }

    pub fn config(&self) -> LinkConfig {
        match *self {
            Self::Direct { src, dst } => LinkConfig::direct(src, dst.module_id, dst.input_type),
            Self::Mixed {
                src,
                dst,
                amount,
                modulation,
            } => {
                LinkConfig::mixed_modulated(src, dst.module_id, dst.input_type, amount, modulation)
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MixedSource {
    pub module_id: ModuleId,
    pub amount: StereoSample,
    pub modulation: Option<ModuleId>,
}

impl MixedSource {
    pub fn source_ids(&self) -> impl Iterator<Item = ModuleId> {
        let mut ids: SmallVec<[ModuleId; 2]> = SmallVec::new();

        ids.push(self.module_id);

        if let Some(modulation) = self.modulation {
            ids.push(modulation);
        }

        ids.into_iter()
    }
}

#[derive(Debug, Clone)]
pub enum InputSource {
    Direct(ModuleId),
    Mixed(Vec<MixedSource>),
}

impl InputSource {
    pub fn source_ids(&self) -> impl Iterator<Item = ModuleId> + '_ {
        let direct = match self {
            Self::Direct(id) => Some(*id),
            Self::Mixed(_) => None,
        };
        let mixed = match self {
            Self::Direct(_) => None,
            Self::Mixed(sources) => Some(sources.iter().flat_map(MixedSource::source_ids)),
        };

        direct.into_iter().chain(mixed.into_iter().flatten())
    }

    pub fn contains_module(&self, module_id: ModuleId) -> bool {
        match self {
            Self::Direct(id) => *id == module_id,
            Self::Mixed(sources) => sources.iter().any(|s| s.module_id == module_id),
        }
    }

    pub(super) fn links(&self, dst: InputId) -> impl Iterator<Item = ModuleLink> + '_ {
        let direct = match self {
            Self::Direct(src) => Some(ModuleLink::direct(*src, dst)),
            Self::Mixed(_) => None,
        };
        let mixed = match self {
            Self::Direct(_) => None,
            Self::Mixed(sources) => Some(sources.iter().map(move |src| {
                ModuleLink::mixed_modulated(src.module_id, dst, src.amount, src.modulation)
            })),
        };

        direct.into_iter().chain(mixed.into_iter().flatten())
    }

    pub fn update_amount(&mut self, src: ModuleId, amount: StereoSample) -> bool {
        match self {
            Self::Mixed(sources) => {
                if let Some(source) = sources.iter_mut().find(|s| s.module_id == src) {
                    source.amount = amount;
                    true
                } else {
                    false
                }
            }
            Self::Direct(_) => false,
        }
    }

    pub fn clear_modulation(&mut self, src: ModuleId) -> bool {
        match self {
            Self::Mixed(sources) => {
                if let Some(source) = sources.iter_mut().find(|s| s.module_id == src) {
                    source.modulation = None;
                    true
                } else {
                    false
                }
            }
            Self::Direct(_) => false,
        }
    }
}

pub fn data_types_compatible(src: DataType, dst: DataType) -> bool {
    src == dst || (dst == DataType::Audio && src == DataType::Control)
}

#[derive(Clone)]
pub struct InputSlot {
    pub src_slot: usize,
    pub modulation_slot: Option<usize>,
    pub amount: StereoSample,
}

#[derive(Clone)]
pub struct InputSlots {
    pub input_type: Input,
    pub slots: Vec<InputSlot>,
}

impl InputSlots {
    pub fn new(input: Input) -> Self {
        Self {
            input_type: input,
            slots: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    pub fn first_slot(&self) -> Option<usize> {
        self.slots.first().map(|s| s.src_slot)
    }

    pub fn update_amount(&mut self, slot: usize, amount: StereoSample) {
        if let Some(src) = self.slots.iter_mut().find(|src| src.src_slot == slot) {
            src.amount = amount
        }
    }

    // Modulated input value projected into [0.0, 1.0] range for control rate input indicator
    pub fn normalized_modulated(&self, channel_idx: usize, modulated_amount: Sample) -> Sample {
        let max_mod_amount = self
            .slots
            .iter()
            .map(|s| s.amount[channel_idx])
            .sum::<Sample>()
            + Sample::EPSILON;

        (modulated_amount / max_mod_amount).clamp(-1.0, 1.0).abs()
    }
}

pub struct SpectralInputSlot {
    pub input_type: Input,
    pub slot: usize,
}
