use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

use crate::synth_engine::{Sample, StereoSample};

mod outputs;
mod outputs_arena;
mod process_context;
mod voice_router;

pub use outputs::{SamplesOutput, SpectralOutput};
pub use outputs_arena::OutputsArena;
pub use process_context::{ProcessContext, ProcessParams};
pub use voice_router::{
    AudioRouterType, ControlRouterType, OutputRouterType, RouterFactory, SpectralRouterType,
    VoiceRouter,
};

pub type ModuleId = i32;

pub const MAX_VOICES: usize = 24;
pub const NUM_CHANNELS: usize = 2;
pub const OUTPUT_MODULE_ID: ModuleId = 0;
pub const MIN_MODULE_ID: ModuleId = 1;

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
    Distortion,    // dB
    ClippingLevel, // dB
    PitchShift,
    Detune,
    DetunePower,
    Glide,
    GlideSlope,
    PhaseShift,
    FrequencyShift,
    Spectrum,
    SpectrumMix(u8),
    SpectrumTo,
    Blend,
    PhasesBlend,
    GainsBlend,
    LowFrequency,
    Cutoff,
    Q,
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
    pub const fn audio(input: Input) -> Self {
        Self {
            input_type: input,
            data_type: DataType::Audio,
            is_direct: true,
        }
    }

    pub const fn audio_mixed(input: Input) -> Self {
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
    Velocity,
    Gain,
    Pan,
    Pitch,
    Timbre,
    Pressure,
}

#[derive(Debug)]
pub enum VoiceEvent {
    Trigger {
        voice_idx: usize,
        prev_voice_idx: Option<usize>,
        pitch: Sample,
        velocity: Sample,
    },
    Update {
        voice_idx: usize,
        pitch: Sample,
        velocity: Sample,
    },
    Release {
        voice_idx: usize,
        velocity: Sample,
    },
    Kill {
        voice_idx: usize,
    },
    Expression {
        voice_idx: usize,
        expression: Expression,
        value: Sample,
    },
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
pub(super) struct ModuleLink {
    pub src: ModuleId,
    pub dst: InputId,
    pub amount: StereoSample,
    pub modulation: Option<ModuleId>,
}

impl ModuleLink {
    pub fn link(src: ModuleId, dst: InputId) -> Self {
        Self {
            src,
            dst,
            amount: StereoSample::ONE,
            modulation: None,
        }
    }

    pub fn scaled(src: ModuleId, dst: InputId, amount: impl Into<StereoSample>) -> Self {
        Self {
            src,
            dst,
            amount: amount.into(),
            modulation: None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct InputSource {
    pub module_id: ModuleId,
    pub amount: StereoSample,
    pub modulation: Option<ModuleId>,
}

impl InputSource {
    pub fn source_ids(&self) -> impl Iterator<Item = ModuleId> {
        let mut ids: SmallVec<[ModuleId; 2]> = SmallVec::new();

        ids.push(self.module_id);

        if let Some(modulation) = self.modulation {
            ids.push(modulation);
        }

        ids.into_iter()
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
    pub fn empty(input_type: Input) -> Self {
        Self {
            input_type,
            slots: Vec::new(),
        }
    }

    pub fn first_slot(&self) -> Option<usize> {
        self.slots.first().map(|s| s.src_slot)
    }

    pub fn update_amount(&mut self, slot: usize, amount: StereoSample) {
        if let Some(src) = self.slots.iter_mut().find(|src| src.src_slot == slot) {
            src.amount = amount
        }
    }
}

pub struct SpectralInputSlot {
    pub input_type: Input,
    pub slot: usize,
}
