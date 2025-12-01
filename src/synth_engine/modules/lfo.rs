use serde::{Deserialize, Serialize};

use crate::synth_engine::{Sample, phase::Phase, routing::NUM_CHANNELS, types::ScalarOutput};

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct ChannelParams {
    frequency: Sample,
    phase_shift: Sample,
    skew: Sample,
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct LfoConfig {
    label: Option<String>,
    reset_phase: bool,
    channels: [ChannelParams; NUM_CHANNELS],
}

struct Voice {
    phase: Phase,
    triggered: bool,
    output: ScalarOutput,
}

pub struct Lfo {}
