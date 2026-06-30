use egui::{Color32, ecolor::Hsva};

use crate::synth_engine::{DataType, Input};

const IO_COLOR_S: f32 = 0.8;
const IO_COLOR_V: f32 = 0.5;

fn color_from_hue(h: f32) -> Color32 {
    Color32::from(Hsva {
        h,
        s: IO_COLOR_S,
        v: IO_COLOR_V,
        a: 1.0,
    })
}

impl Input {
    pub fn label(&self) -> String {
        match self {
            Self::Audio => "Audio".to_string(),
            Self::AudioMix(idx) => format!("Audio Mix {}", idx + 1),
            Self::Gain => "Gain".to_string(),
            Self::GainMix(idx) => format!("Gain Mix {}", idx + 1),
            Self::Level => "Level".to_string(),
            Self::LevelMix(idx) => format!("Level Mix {}", idx + 1),
            Self::Distortion => "Distortion".to_string(),
            Self::ClippingLevel => "Clipping Level".to_string(),
            Self::PitchShift => "Pitch Shift".to_string(),
            Self::Detune => "Detune".to_string(),
            Self::DetunePower => "Detune Power".to_string(),
            Self::Glide => "Glide".to_string(),
            Self::GlideSlope => "Glide Slope".to_string(),
            Self::PhaseShift => "Phase Shift".to_string(),
            Self::FrequencyShift => "Frequency Shift".to_string(),
            Self::Spectrum => "Spectrum".to_string(),
            Self::SpectrumMix(idx) => format!("Spectrum Mix {}", idx + 1),
            Self::SpectrumTo => "Spectrum To".to_string(),
            Self::Blend => "Blend".to_string(),
            Self::PhasesBlend => "Phases Blend".to_string(),
            Self::GainsBlend => "Gains Blend".to_string(),
            Self::LowFrequency => "Low Frequency".to_string(),
            Self::Cutoff => "Cutoff".to_string(),
            Self::Q => "Q".to_string(),
            Self::Drive => "Drive".to_string(),
            Self::Skew => "Skew".to_string(),
            Self::Delay => "Delay".to_string(),
            Self::Attack => "Attack".to_string(),
            Self::Hold => "Hold".to_string(),
            Self::Decay => "Decay".to_string(),
            Self::Sustain => "Sustain".to_string(),
            Self::Release => "Release".to_string(),
        }
    }

    pub fn hue(&self) -> f32 {
        match self {
            Self::Audio => 0.0,
            Self::AudioMix(idx) => 0.0 + *idx as f32 * 0.012,
            Self::Gain => 0.10,
            Self::GainMix(idx) => 0.10 + *idx as f32 * 0.012,
            Self::Level => 0.14,
            Self::LevelMix(idx) => 0.14 + *idx as f32 * 0.012,
            Self::Distortion => 0.02,
            Self::ClippingLevel => 0.04,
            Self::Drive => 0.06,
            Self::PitchShift => 0.70,
            Self::Detune => 0.73,
            Self::DetunePower => 0.76,
            Self::Glide => 0.67,
            Self::GlideSlope => 0.64,
            Self::PhaseShift => 0.79,
            Self::FrequencyShift => 0.68,
            Self::Spectrum => 0.86,
            Self::SpectrumMix(idx) => 0.86 + *idx as f32 * 0.012,
            Self::SpectrumTo => 0.83,
            Self::Blend => 0.42,
            Self::PhasesBlend => 0.39,
            Self::GainsBlend => 0.45,
            Self::LowFrequency => 0.30,
            Self::Cutoff => 0.32,
            Self::Q => 0.34,
            Self::Skew => 0.18,
            Self::Delay => 0.20,
            Self::Attack => 0.58,
            Self::Hold => 0.53,
            Self::Decay => 0.48,
            Self::Sustain => 0.43,
            Self::Release => 0.38,
        }
    }

    pub fn color(&self) -> Color32 {
        color_from_hue(self.hue())
    }
}

impl DataType {
    pub fn hue(&self) -> f32 {
        match self {
            Self::Audio => 0.0,
            Self::Control => 0.36,
            Self::Spectral => 0.84,
        }
    }

    pub fn color(&self) -> Color32 {
        color_from_hue(self.hue())
    }
}
