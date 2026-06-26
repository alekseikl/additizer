use rustc_hash::FxHashMap;

use crate::{
    preset::{Preset, PresetInfo},
    synth_engine::{
        EngineConfig, Input, LinkConfig, ModuleConfig, ModuleId, OUTPUT_MODULE_ID, StereoSample,
        amplifier::AmplifierConfig,
        envelope::EnvelopeConfig,
        harmonic_editor::HarmonicEditorConfig,
        oscillator::OscillatorConfig,
        spectral_filter::SpectralFilterConfig,
        ui_bridge::{GridVec, ui_config::{UiConfig, UiModuleConfig}},
    },
    utils::{from_ms, st_to_octave},
};

const HARMONIC_EDITOR_ID: ModuleId = 1;
const FILTER_ENV_ID: ModuleId = 2;
const FILTER_ID: ModuleId = 3;
const OSC_ID: ModuleId = 4;
const AMP_ID: ModuleId = 5;
const AMP_ENV_ID: ModuleId = 6;

fn default_ui_config() -> UiConfig {
    let mut modules = FxHashMap::default();

    // Layout: main signal chain across the top row (y=2), modulation sources below (y=4).
    // Each module occupies 4 grid columns wide × 2 rows tall (154×77 px per module).
    // Column spacing of 6 gives a 77 px gap between adjacent modules.
    //
    //  col:  0                    6                           12                          18                24
    //  y=0: [HarmonicEditor                      ]            [Oscillator                      ]
    //  y=2:            [SpectralFilter                      ]             [Amplifier               ]  [Output]
    //  y=4: [FilterEnv                           ]             [AmpEnv                            ]
    for (id, label, grid_x, grid_y) in [
        (HARMONIC_EDITOR_ID, "01 - Harmonics",  0, 0),
        (FILTER_ENV_ID,      "03 - Cutoff Env", 0, 4),
        (FILTER_ID,          "03 - Filter",     6, 2),
        (OSC_ID,             "04 - Oscillator", 12, 0),
        (AMP_ENV_ID,         "06 - Amp Env",    12, 4),
        (AMP_ID,             "06 - Amplifier",  18, 2),
        (OUTPUT_MODULE_ID,   "Output",          24, 2),
    ] {
        modules.insert(
            id,
            UiModuleConfig {
                id,
                label: label.into(),
                position: GridVec { x: grid_x, y: grid_y },
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
        decay: from_ms(400.0).into(),
        sustain: 0.6.into(),
        release: from_ms(300.0).into(),
        smooth: from_ms(4.0).into(),
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
            LinkConfig {
                src_id: HARMONIC_EDITOR_ID,
                dst_id: FILTER_ID,
                dst_input: Input::Spectrum,
                amount: StereoSample::ONE,
                modulator_id: None,
            },
            LinkConfig {
                src_id: FILTER_ENV_ID,
                dst_id: FILTER_ID,
                dst_input: Input::Cutoff,
                amount: st_to_octave(64.0).into(),
                modulator_id: None,
            },
            LinkConfig {
                src_id: FILTER_ID,
                dst_id: OSC_ID,
                dst_input: Input::Spectrum,
                amount: StereoSample::ONE,
                modulator_id: None,
            },
            LinkConfig {
                src_id: OSC_ID,
                dst_id: AMP_ID,
                dst_input: Input::Audio,
                amount: StereoSample::ONE,
                modulator_id: None,
            },
            LinkConfig {
                src_id: AMP_ENV_ID,
                dst_id: AMP_ID,
                dst_input: Input::Gain,
                amount: StereoSample::ONE,
                modulator_id: None,
            },
            LinkConfig {
                src_id: AMP_ID,
                dst_id: OUTPUT_MODULE_ID,
                dst_input: Input::Audio,
                amount: StereoSample::ONE,
                modulator_id: None,
            },
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
