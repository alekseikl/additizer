use rustc_hash::FxHashMap;

use crate::{
    preset::{Preset, PresetInfo},
    synth_engine::{
        EngineConfig, Input, LinkConfig, ModuleConfig, ModuleId, OUTPUT_MODULE_ID, StereoSample,
        amplifier::AmplifierConfig,
        envelope::EnvelopeConfig,
        harmonic_editor::HarmonicEditorConfig,
        oscillator::OscillatorConfig,
        pitch::PitchConfig,
        spectral_filter::SpectralFilterConfig,
        ui_bridge::{
            GridVec,
            ui_config::{UiConfig, UiModuleConfig},
        },
    },
    utils::{from_ms, from_st},
};

const HARMONIC_EDITOR_ID: ModuleId = 1;
const FILTER_ENV_ID: ModuleId = 2;
const FILTER_ID: ModuleId = 3;
const OSC_ID: ModuleId = 4;
const AMP_ID: ModuleId = 5;
const AMP_ENV_ID: ModuleId = 6;
const PITCH_ID: ModuleId = 7;

fn default_ui_config() -> UiConfig {
    let mut modules = FxHashMap::default();

    for (id, label, grid_x, grid_y) in [
        (PITCH_ID, "Pitch", 0, 0),
        (HARMONIC_EDITOR_ID, "Harmonics", 3, 2),
        (FILTER_ENV_ID, "Cutoff Envelope", 3, 4),
        (FILTER_ID, "Filter", 8, 2),
        (OSC_ID, "Oscillator", 13, 2),
        (AMP_ENV_ID, "Amp Envelope", 13, 4),
        (AMP_ID, "Amp", 18, 2),
        (OUTPUT_MODULE_ID, "Out", 20, 2),
    ] {
        modules.insert(
            id,
            UiModuleConfig {
                id,
                label: label.into(),
                position: GridVec {
                    x: grid_x,
                    y: grid_y,
                },
            },
        );
    }

    UiConfig { modules }
}

fn default_engine_config() -> EngineConfig {
    let filter_env = EnvelopeConfig {
        id: FILTER_ENV_ID,
        attack: 0.0.into(),
        decay: from_ms(500.0).into(),
        decay_curvature: 0.5,
        sustain: 0.0.into(),
        release: from_ms(100.0).into(),
        ..EnvelopeConfig::default()
    };

    let amp_env = EnvelopeConfig {
        id: AMP_ENV_ID,
        attack: from_ms(2.0).into(),
        decay: from_ms(400.0).into(),
        sustain: 0.6.into(),
        release: from_ms(300.0).into(),
        keep_voice_alive: true,
        ..EnvelopeConfig::default()
    };

    let spectral_filter = SpectralFilterConfig {
        id: FILTER_ID,
        cutoff: 2.0.into(),
        ..SpectralFilterConfig::default()
    };

    EngineConfig {
        engine: Default::default(),
        modules: vec![
            ModuleConfig::HarmonicEditor(Box::new(HarmonicEditorConfig {
                id: HARMONIC_EDITOR_ID,
                ..HarmonicEditorConfig::default()
            })),
            ModuleConfig::Envelope(Box::new(filter_env)),
            ModuleConfig::SpectralFilter(Box::new(spectral_filter)),
            ModuleConfig::Pitch(Box::new(PitchConfig {
                id: PITCH_ID,
                ..PitchConfig::default()
            })),
            ModuleConfig::Oscillator(Box::new(OscillatorConfig {
                id: OSC_ID,
                ..OscillatorConfig::default()
            })),
            ModuleConfig::Amplifier(Box::new(AmplifierConfig {
                id: AMP_ID,
                ..AmplifierConfig::default()
            })),
            ModuleConfig::Envelope(Box::new(amp_env)),
        ],
        links: vec![
            LinkConfig::direct(HARMONIC_EDITOR_ID, FILTER_ID, Input::Spectrum),
            LinkConfig::mixed(FILTER_ENV_ID, FILTER_ID, Input::Cutoff, from_st(64.0)),
            LinkConfig::direct(FILTER_ID, OSC_ID, Input::Spectrum),
            LinkConfig::direct(PITCH_ID, HARMONIC_EDITOR_ID, Input::Pitch),
            LinkConfig::direct(PITCH_ID, FILTER_ID, Input::Pitch),
            LinkConfig::direct(PITCH_ID, OSC_ID, Input::Pitch),
            LinkConfig::direct(OSC_ID, AMP_ID, Input::Audio),
            LinkConfig::mixed(AMP_ENV_ID, AMP_ID, Input::Gain, StereoSample::ONE),
            LinkConfig::direct(AMP_ID, OUTPUT_MODULE_ID, Input::Audio),
        ],
    }
}

pub fn build_default_preset() -> Preset {
    Preset {
        info: PresetInfo::default(),
        engine: default_engine_config(),
        ui: default_ui_config(),
    }
}
