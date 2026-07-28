use egui::{Color32, ecolor::Hsva};

use crate::{
    editor::slider::{self, Slider},
    synth_engine::{DataType, Input, ModuleType, StereoSample},
    utils::from_st,
};

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
            Self::AudioMix(idx) => format!("Audio #{}", idx + 1),
            Self::Gain => "Gain".to_string(),
            Self::GainMix(idx) => format!("Gain #{}", idx + 1),
            Self::Level => "Level".to_string(),
            Self::LevelMix(idx) => format!("Level #{}", idx + 1),
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
            Self::SpectrumMix(idx) => format!("Spectrum #{}", idx + 1),
            Self::SpectrumTo => "Spectrum To".to_string(),
            Self::Blend => "Blend".to_string(),
            Self::PhasesBlend => "Phases Blend".to_string(),
            Self::GainsBlend => "Gains Blend".to_string(),
            Self::LowFrequency => "Low Frequency".to_string(),
            Self::Cutoff => "Cutoff".to_string(),
            Self::Resonance => "Resonance".to_string(),
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
            Self::Resonance => 0.34,
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

    pub fn amount_slider<'a>(&self, amount: &'a mut StereoSample) -> Slider<'a> {
        fn bipolar<'a>(amount: &'a mut StereoSample) -> Slider<'a> {
            Slider::stereo(amount, 0.0..=1.0, Some(-1.0)).default(0.0)
        }

        match self {
            Self::Gain | Self::GainMix(_) => bipolar(amount),
            Self::Level | Self::LevelMix(_) => Slider::stereo(amount, 0.0..=100.0, Some(-100.0))
                .default(0.0)
                .skew(2.0)
                .units(slider::Units::Db),
            Self::Drive | Self::ClippingLevel => Slider::stereo(amount, 0.0..=24.0, Some(-24.0))
                .default(0.0)
                .units(slider::Units::Db),
            Self::Distortion => Slider::stereo(amount, 0.0..=48.0, Some(-48.0))
                .default(0.0)
                .units(slider::Units::Db),
            Self::Blend | Self::GainsBlend | Self::PhasesBlend => bipolar(amount),
            Self::Cutoff => Slider::stereo(amount, 0.0..=10.0, Some(-10.0))
                .default(0.0)
                .units(slider::Units::Octaves),
            Self::Resonance => bipolar(amount),
            Self::Detune => Slider::stereo(amount, 0.0..=from_st(1.0), Some(-from_st(1.0)))
                .default(0.0)
                .units(slider::Units::Octaves),
            Self::DetunePower => Slider::stereo(amount, 0.0..=5.0, Some(-5.0)).default(0.0),
            Self::PitchShift => Slider::stereo(amount, 0.0..=8.0, Some(-8.0))
                .skew(1.8)
                .default(0.0)
                .units(slider::Units::Octaves),
            Self::Glide => Slider::stereo(amount, 0.0..=5.0, Some(-5.0))
                .default(0.0)
                .skew(2.0)
                .units(slider::Units::Time),
            Self::GlideSlope => bipolar(amount),
            Self::PhaseShift => bipolar(amount),
            Self::FrequencyShift => Slider::stereo(amount, 0.0..=880.0, Some(-880.0))
                .default(0.0)
                .skew(2.0)
                .units(slider::Units::Frequency),
            Self::LowFrequency => Slider::stereo(amount, 0.0..=100.0, Some(-100.0))
                .default(0.0)
                .skew(1.8)
                .units(slider::Units::Frequency),
            Self::Skew => bipolar(amount),
            Self::Sustain => {
                Slider::stereo(amount, 0.0..=1.0, None).units(slider::Units::Normalized)
            }
            Self::Delay | Self::Attack | Self::Hold | Self::Decay | Self::Release => {
                Slider::stereo(amount, 0.0..=8.0, Some(-8.0))
                    .default(0.0)
                    .skew(2.0)
                    .units(slider::Units::Time)
            }
            Self::Audio
            | Self::AudioMix(_)
            | Self::Spectrum
            | Self::SpectrumTo
            | Self::SpectrumMix(_) => bipolar(amount),
        }
    }

    pub fn param_slider<'a>(&self, value: &'a mut StereoSample) -> Slider<'a> {
        match self {
            Self::Gain | Self::GainMix(_) => {
                Slider::stereo(value, 0.0..=1.0, Some(-1.0)).default(1.0)
            }
            Self::Level | Self::LevelMix(_) => Slider::stereo(value, 0.0..=1.0, None)
                .default(0.0)
                .over(0.0),
            Self::Drive | Self::ClippingLevel => Slider::stereo(value, 0.0..=24.0, Some(-24.0))
                .default(0.0)
                .units(slider::Units::Db),
            Self::Distortion => Slider::stereo(value, 0.0..=40.0, None)
                .default(0.0)
                .units(slider::Units::Db),
            Self::Blend | Self::GainsBlend | Self::PhasesBlend => {
                Slider::stereo(value, 0.0..=1.0, None).default(0.0)
            }
            Self::Cutoff => Slider::stereo(value, -2.0..=10.0, Some(-10.0))
                .over(8.0)
                .default(0.0)
                .units(slider::Units::Octaves),
            Self::Resonance => Slider::stereo(value, 0.0..=1.0, Some(-1.0)).default(0.0),
            Self::Detune => Slider::stereo(value, 0.0..=from_st(1.0), None)
                .default(from_st(0.2))
                .units(slider::Units::Octaves),
            Self::DetunePower => Slider::stereo(value, 0.0..=1.0, Some(-1.0)),
            Self::PitchShift => Slider::stereo(value, 0.0..=8.0, Some(-8.0))
                .skew(1.8)
                .default(0.0)
                .units(slider::Units::Octaves),
            Self::Glide => Slider::stereo(value, 0.0..=5.0, None)
                .default(0.0)
                .skew(2.0)
                .units(slider::Units::Time),
            Self::GlideSlope => Slider::stereo(value, 0.0..=1.0, Some(-1.0)),
            Self::PhaseShift => Slider::stereo(value, 0.0..=1.0, Some(-1.0)),
            Self::FrequencyShift => Slider::stereo(value, 0.0..=1_000.0, Some(-1_000.0))
                .default(0.0)
                .skew(2.0)
                .units(slider::Units::Frequency),
            Self::LowFrequency => Slider::stereo(value, 0.0..=100.0, Some(-100.0))
                .default(1.0)
                .skew(1.8)
                .units(slider::Units::Frequency),
            Self::Skew => Slider::stereo(value, 0.0..=1.0, None).default(0.5),
            Self::Sustain => Slider::stereo(value, 0.0..=1.0, None)
                .default(0.5)
                .units(slider::Units::Normalized),
            Self::Delay | Self::Attack | Self::Hold | Self::Decay | Self::Release => {
                Slider::stereo(value, 0.0..=8.0, None)
                    .default(0.0)
                    .skew(2.0)
                    .units(slider::Units::Time)
            }
            Self::Audio
            | Self::AudioMix(_)
            | Self::Spectrum
            | Self::SpectrumTo
            | Self::SpectrumMix(_) => Slider::stereo(value, 0.0..=1.0, None),
        }
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

impl ModuleType {
    pub fn input_label(self, input: Input) -> String {
        match self {
            Self::Mixer => match input {
                Input::Gain => "Output gain".into(),
                Input::Level => "Output level (dB)".into(),
                Input::AudioMix(i) => format!("Audio In #{}", i + 1),
                Input::GainMix(i) => format!("Input #{} gain", i + 1),
                Input::LevelMix(i) => format!("Input #{} level (dB)", i + 1),
                _ => input.label(),
            },
            Self::SpectralMixer => match input {
                Input::Gain => "Output gain".into(),
                Input::Level => "Output level (dB)".into(),
                Input::SpectrumMix(i) => format!("Spectral In #{}", i + 1),
                Input::GainMix(i) => format!("Input #{} gain", i + 1),
                Input::LevelMix(i) => format!("Input #{} level (dB)", i + 1),
                _ => input.label(),
            },
            _ => input.label(),
        }
    }
}
