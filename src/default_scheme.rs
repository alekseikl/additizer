use rustc_hash::FxHashMap;

use crate::{
    preset::{Preset, PresetInfo},
    synth_engine::{
        EngineConfig, EnvelopeCurve, Input, LinkConfig, ModuleConfig, ModuleId, OUTPUT_MODULE_ID,
        StereoSample,
        amplifier::AmplifierConfig,
        envelope::EnvelopeConfig,
        harmonic_editor::HarmonicEditorConfig,
        oscillator::OscillatorConfig,
        spectral_filter::SpectralFilterConfig,
        ui_bridge::ui_config::{UiConfig, UiModuleConfig},
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

    for (id, label) in [
        (HARMONIC_EDITOR_ID, "01 - Harmonics"),
        (FILTER_ENV_ID, "03 - Cutoff Env"),
        (FILTER_ID, "03 - Filter"),
        (OSC_ID, "04 - Oscillator"),
        (AMP_ENV_ID, "06 - Amp Envelope"),
        (AMP_ID, "06 - Amplifier"),
    ] {
        modules.insert(
            id,
            UiModuleConfig {
                id,
                label: label.into(),
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
        sustain: 0.0.into(),
        release: from_ms(100.0).into(),
        attack_curve: EnvelopeCurve::ExponentialOut,
        decay_curve: EnvelopeCurve::ExponentialOut,
        ..EnvelopeConfig::default()
    };

    let amp_env = EnvelopeConfig {
        id: AMP_ENV_ID,
        decay: from_ms(400.0).into(),
        sustain: 0.6.into(),
        release: from_ms(300.0).into(),
        decay_curve: EnvelopeCurve::ExponentialOut,
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
