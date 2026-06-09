use std::sync::Arc;

use nih_plug::prelude::*;

use super::*;
use crate::synth_engine::{
    amplifier::AmplifierConfig,
    envelope::EnvelopeConfig,
    expressions::ExpressionsConfig,
    external_param::ExternalParamConfig,
    harmonic_editor::HarmonicEditorConfig,
    lfo::LfoConfig,
    mixer::MixerConfig,
    modules::Output,
    oscillator::OscillatorConfig,
    spectral_blend::SpectralBlendConfig,
    spectral_filter::SpectralFilterConfig,
    spectral_mixer::SpectralMixerConfig,
    wave_shaper::WaveShaperConfig,
};

const SAMPLE_RATE: Sample = 48_000.0;
const HARMONIC_EDITOR_ID: ModuleId = 1;
const OSCILLATOR_ID: ModuleId = 2;

const HE0_ID: ModuleId = 1;
const HE1_ID: ModuleId = 2;
const HE2_ID: ModuleId = 3;
const SPECTRAL_MIXER_ID: ModuleId = 4;
const SPECTRAL_BLEND_ID: ModuleId = 5;
const SPECTRAL_FILTER_ID: ModuleId = 6;
const ENVELOPE_FILTER_ID: ModuleId = 7;
const ENVELOPE_AMP_ID: ModuleId = 8;
const OSC0_ID: ModuleId = 9;
const OSC1_ID: ModuleId = 10;
const LFO_ID: ModuleId = 11;
const MIXER_ID: ModuleId = 12;
const AMPLIFIER_ID: ModuleId = 13;
const WAVE_SHAPER_ID: ModuleId = 14;
const EXTERNAL_PARAM_ID: ModuleId = 15;
const EXPRESSIONS_ID: ModuleId = 16;

fn test_deps() -> (Arc<FloatParam>, Arc<ExternalParamsBlock>) {
    let volume = Arc::new(FloatParam::new(
        "Volume",
        0.0,
        FloatRange::Linear { min: 0.0, max: 1.0 },
    ));

    let float_param = |name: &str| {
        Arc::new(FloatParam::new(
            name,
            0.0,
            FloatRange::Linear { min: 0.0, max: 1.0 },
        ))
    };

    let external_params = Arc::new(ExternalParamsBlock {
        float_params: [
            float_param("Float Param 1"),
            float_param("Float Param 2"),
            float_param("Float Param 3"),
            float_param("Float Param 4"),
        ],
    });

    (volume, external_params)
}

fn minimal_engine_config(engine: EngineParams, osc: OscillatorConfig) -> EngineConfig {
    EngineConfig {
        engine,
        modules: vec![
            ModuleConfig::HarmonicEditor(Box::new(HarmonicEditorConfig {
                id: HARMONIC_EDITOR_ID,
                ..HarmonicEditorConfig::default()
            })),
            ModuleConfig::Oscillator(Box::new(osc)),
        ],
        links: vec![
            LinkConfig {
                src_id: HARMONIC_EDITOR_ID,
                dst_id: OSCILLATOR_ID,
                dst_input: Input::Spectrum,
                amount: StereoSample::ONE,
                modulator_id: None,
            },
            LinkConfig {
                src_id: OSCILLATOR_ID,
                dst_id: OUTPUT_MODULE_ID,
                dst_input: Input::Audio,
                amount: StereoSample::ONE,
                modulator_id: None,
            },
        ],
    }
}

fn make_engine(engine: EngineParams, osc: OscillatorConfig) -> SynthEngine {
    let (volume, external_params) = test_deps();
    let config = minimal_engine_config(engine, osc);

    SynthEngine::try_new(&config, volume, external_params, SAMPLE_RATE)
        .expect("valid engine config")
}

fn process_block(engine: &mut SynthEngine, samples: usize) -> (Vec<Sample>, Vec<Sample>) {
    let mut left = vec![0.0; samples];
    let mut right = vec![0.0; samples];

    engine.process(samples, false, [&mut left[..], &mut right[..]].into_iter());

    (left, right)
}

fn rms(samples: &[Sample]) -> Sample {
    (samples.iter().map(|s| s * s).sum::<Sample>() / samples.len() as Sample).sqrt()
}

fn link(src_id: ModuleId, dst_id: ModuleId, dst_input: Input) -> LinkConfig {
    LinkConfig {
        src_id,
        dst_id,
        dst_input,
        amount: StereoSample::ONE,
        modulator_id: None,
    }
}

fn full_patch_engine_config(engine: EngineParams) -> EngineConfig {
    EngineConfig {
        engine,
        modules: vec![
            ModuleConfig::HarmonicEditor(Box::new(HarmonicEditorConfig {
                id: HE0_ID,
                ..HarmonicEditorConfig::default()
            })),
            ModuleConfig::HarmonicEditor(Box::new(HarmonicEditorConfig {
                id: HE1_ID,
                ..HarmonicEditorConfig::default()
            })),
            ModuleConfig::HarmonicEditor(Box::new(HarmonicEditorConfig {
                id: HE2_ID,
                ..HarmonicEditorConfig::default()
            })),
            ModuleConfig::SpectralMixer(Box::new(SpectralMixerConfig {
                id: SPECTRAL_MIXER_ID,
                ..SpectralMixerConfig::default()
            })),
            ModuleConfig::SpectralBlend(Box::new(SpectralBlendConfig {
                id: SPECTRAL_BLEND_ID,
                ..SpectralBlendConfig::default()
            })),
            ModuleConfig::SpectralFilter(Box::new(SpectralFilterConfig {
                id: SPECTRAL_FILTER_ID,
                ..SpectralFilterConfig::default()
            })),
            ModuleConfig::Envelope(Box::new(EnvelopeConfig {
                id: ENVELOPE_FILTER_ID,
                ..EnvelopeConfig::default()
            })),
            ModuleConfig::Envelope(Box::new(EnvelopeConfig {
                id: ENVELOPE_AMP_ID,
                ..EnvelopeConfig::default()
            })),
            ModuleConfig::Oscillator(Box::new(OscillatorConfig {
                id: OSC0_ID,
                ..OscillatorConfig::default()
            })),
            ModuleConfig::Oscillator(Box::new(OscillatorConfig {
                id: OSC1_ID,
                ..OscillatorConfig::default()
            })),
            ModuleConfig::Lfo(Box::new(LfoConfig {
                id: LFO_ID,
                ..LfoConfig::default()
            })),
            ModuleConfig::Mixer(Box::new(MixerConfig {
                id: MIXER_ID,
                ..MixerConfig::default()
            })),
            ModuleConfig::Amplifier(Box::new(AmplifierConfig {
                id: AMPLIFIER_ID,
                ..AmplifierConfig::default()
            })),
            ModuleConfig::WaveShaper(Box::new(WaveShaperConfig {
                id: WAVE_SHAPER_ID,
                ..WaveShaperConfig::default()
            })),
            ModuleConfig::ExternalParam(Box::new(ExternalParamConfig {
                id: EXTERNAL_PARAM_ID,
                ..ExternalParamConfig::default()
            })),
            ModuleConfig::Expressions(Box::new(ExpressionsConfig {
                id: EXPRESSIONS_ID,
                ..ExpressionsConfig::default()
            })),
        ],
        links: vec![
            link(HE0_ID, SPECTRAL_MIXER_ID, Input::SpectrumMix(0)),
            link(HE1_ID, SPECTRAL_MIXER_ID, Input::SpectrumMix(1)),
            link(SPECTRAL_MIXER_ID, SPECTRAL_BLEND_ID, Input::Spectrum),
            link(HE2_ID, SPECTRAL_BLEND_ID, Input::SpectrumTo),
            link(SPECTRAL_BLEND_ID, SPECTRAL_FILTER_ID, Input::Spectrum),
            link(ENVELOPE_FILTER_ID, SPECTRAL_FILTER_ID, Input::Cutoff),
            link(SPECTRAL_FILTER_ID, OSC0_ID, Input::Spectrum),
            link(HE0_ID, OSC1_ID, Input::Spectrum),
            link(LFO_ID, OSC1_ID, Input::PitchShift),
            link(OSC0_ID, MIXER_ID, Input::AudioMix(0)),
            link(OSC1_ID, MIXER_ID, Input::AudioMix(1)),
            link(MIXER_ID, AMPLIFIER_ID, Input::Audio),
            link(ENVELOPE_AMP_ID, AMPLIFIER_ID, Input::Gain),
            link(AMPLIFIER_ID, WAVE_SHAPER_ID, Input::Audio),
            link(EXTERNAL_PARAM_ID, WAVE_SHAPER_ID, Input::ClippingLevel),
            link(EXPRESSIONS_ID, WAVE_SHAPER_ID, Input::Distortion),
            link(WAVE_SHAPER_ID, OUTPUT_MODULE_ID, Input::Audio),
        ],
    }
}

fn make_full_patch_engine(engine: EngineParams) -> SynthEngine {
    let (volume, external_params) = test_deps();
    let config = full_patch_engine_config(engine);

    SynthEngine::try_new(&config, volume, external_params, SAMPLE_RATE)
        .expect("valid full patch config")
}

// ---- Construction ----

#[test]
fn try_new_builds_minimal_patch() {
    let engine = make_engine(
        EngineParams::default(),
        OscillatorConfig {
            id: OSCILLATOR_ID,
            ..OscillatorConfig::default()
        },
    );

    assert!(
        engine
            .get_typed_module::<HarmonicEditor>(HARMONIC_EDITOR_ID)
            .is_some()
    );
    assert!(
        engine
            .get_typed_module::<Oscillator>(OSCILLATOR_ID)
            .is_some()
    );
    assert!(
        engine
            .get_typed_module::<Output>(OUTPUT_MODULE_ID)
            .is_some()
    );
}

#[test]
fn try_new_builds_full_patch() {
    let engine = make_full_patch_engine(EngineParams::default());
    let cfg = engine.get_config();

    assert_eq!(cfg.modules.len(), 16);
    assert_eq!(cfg.links.len(), 17);

    assert!(engine.get_typed_module::<HarmonicEditor>(HE0_ID).is_some());
    assert!(engine.get_typed_module::<HarmonicEditor>(HE1_ID).is_some());
    assert!(engine.get_typed_module::<HarmonicEditor>(HE2_ID).is_some());
    assert!(
        engine
            .get_typed_module::<SpectralMixer>(SPECTRAL_MIXER_ID)
            .is_some()
    );
    assert!(
        engine
            .get_typed_module::<SpectralBlend>(SPECTRAL_BLEND_ID)
            .is_some()
    );
    assert!(
        engine
            .get_typed_module::<SpectralFilter>(SPECTRAL_FILTER_ID)
            .is_some()
    );
    assert!(
        engine
            .get_typed_module::<Envelope>(ENVELOPE_FILTER_ID)
            .is_some()
    );
    assert!(
        engine
            .get_typed_module::<Envelope>(ENVELOPE_AMP_ID)
            .is_some()
    );
    assert!(engine.get_typed_module::<Oscillator>(OSC0_ID).is_some());
    assert!(engine.get_typed_module::<Oscillator>(OSC1_ID).is_some());
    assert!(engine.get_typed_module::<Lfo>(LFO_ID).is_some());
    assert!(engine.get_typed_module::<Mixer>(MIXER_ID).is_some());
    assert!(
        engine
            .get_typed_module::<Amplifier>(AMPLIFIER_ID)
            .is_some()
    );
    assert!(
        engine
            .get_typed_module::<WaveShaper>(WAVE_SHAPER_ID)
            .is_some()
    );
    assert!(
        engine
            .get_typed_module::<ExternalParam>(EXTERNAL_PARAM_ID)
            .is_some()
    );
    assert!(
        engine
            .get_typed_module::<Expressions>(EXPRESSIONS_ID)
            .is_some()
    );
    assert!(
        engine
            .get_typed_module::<Output>(OUTPUT_MODULE_ID)
            .is_some()
    );

    let order = SynthEngine::calc_execution_order(
        &cfg.links
            .iter()
            .map(|l| ModuleLink::scaled(l.src_id, ModuleInput::new(l.dst_input, l.dst_id), l.amount))
            .collect::<Vec<_>>(),
    )
    .expect("full patch execution order");

    assert_eq!(*order.last().unwrap(), OUTPUT_MODULE_ID);
}

#[test]
fn full_patch_produces_audio() {
    let mut engine = make_full_patch_engine(EngineParams {
        num_voices: 2,
        ..EngineParams::default()
    });

    engine.handle_note_on(0, 60, 1.0);

    let (left, right) = process_block(&mut engine, 64);

    assert!(rms(&left) > 1e-6);
    assert!(rms(&right) > 1e-6);
    assert!(left.iter().all(|s| s.is_finite()));
    assert!(right.iter().all(|s| s.is_finite()));
}

#[test]
fn try_new_rejects_duplicate_module_id() {
    let (volume, external_params) = test_deps();
    let config = EngineConfig {
        engine: EngineParams::default(),
        modules: vec![
            ModuleConfig::HarmonicEditor(Box::new(HarmonicEditorConfig {
                id: 1,
                ..HarmonicEditorConfig::default()
            })),
            ModuleConfig::Oscillator(Box::new(OscillatorConfig {
                id: 1,
                ..OscillatorConfig::default()
            })),
        ],
        links: vec![],
    };

    assert!(SynthEngine::try_new(&config, volume, external_params, SAMPLE_RATE).is_none());
}

#[test]
fn try_new_rejects_invalid_link() {
    let (volume, external_params) = test_deps();
    let config = EngineConfig {
        engine: EngineParams::default(),
        modules: vec![
            ModuleConfig::HarmonicEditor(Box::new(HarmonicEditorConfig {
                id: HARMONIC_EDITOR_ID,
                ..HarmonicEditorConfig::default()
            })),
            ModuleConfig::Oscillator(Box::new(OscillatorConfig {
                id: OSCILLATOR_ID,
                ..OscillatorConfig::default()
            })),
        ],
        links: vec![LinkConfig {
            // Harmonic editor outputs spectrum, not audio — cannot feed the output module directly.
            src_id: HARMONIC_EDITOR_ID,
            dst_id: OUTPUT_MODULE_ID,
            dst_input: Input::Audio,
            amount: StereoSample::ONE,
            modulator_id: None,
        }],
    };

    assert!(SynthEngine::try_new(&config, volume, external_params, SAMPLE_RATE).is_none());
}

#[test]
fn config_round_trips_minimal_patch() {
    let engine = make_engine(
        EngineParams {
            num_voices: 4,
            block_size: 64,
            ..EngineParams::default()
        },
        OscillatorConfig {
            id: OSCILLATOR_ID,
            unison_voices: 3,
            ..OscillatorConfig::default()
        },
    );

    let cfg = engine.get_config();

    assert_eq!(cfg.engine.num_voices, 4);
    assert_eq!(cfg.engine.block_size, 64);
    assert_eq!(cfg.modules.len(), 2);
    assert_eq!(cfg.links.len(), 2);

    let osc = cfg
        .modules
        .iter()
        .find_map(|m| match m {
            ModuleConfig::Oscillator(c) => Some(c.as_ref()),
            _ => None,
        })
        .expect("oscillator config");

    assert_eq!(osc.id, OSCILLATOR_ID);
    assert_eq!(osc.unison_voices, 3);
}

// ---- Engine parameter setters ----

#[test]
fn block_size_clamps() {
    let mut engine = make_engine(
        EngineParams::default(),
        OscillatorConfig {
            id: OSCILLATOR_ID,
            ..OscillatorConfig::default()
        },
    );

    engine.set_block_size(0);
    assert_eq!(engine.block_size(), 4);

    engine.set_block_size(999);
    assert_eq!(engine.block_size(), MAX_BLOCK_SIZE);

    engine.set_block_size(32);
    assert_eq!(engine.get_config().engine.block_size, 32);
}

#[test]
fn num_voices_and_legato_setters() {
    let mut engine = make_engine(
        EngineParams::default(),
        OscillatorConfig {
            id: OSCILLATOR_ID,
            ..OscillatorConfig::default()
        },
    );

    engine.set_num_voices(0);
    assert_eq!(engine.get_config().engine.num_voices, 1);

    engine.set_num_voices(999);
    assert_eq!(
        engine.get_config().engine.num_voices,
        SynthEngine::AVAILABLE_VOICES
    );

    engine.set_legato(true);
    assert!(engine.get_config().engine.legato);
}

#[test]
fn output_gain_setters() {
    let mut engine = make_engine(
        EngineParams::default(),
        OscillatorConfig {
            id: OSCILLATOR_ID,
            ..OscillatorConfig::default()
        },
    );

    engine.set_output_gain(StereoSample::new(0.25, 0.75));
    assert_eq!(engine.get_output_gain(), StereoSample::new(0.25, 0.75));
    assert_eq!(
        engine.get_config().engine.output_gain,
        StereoSample::new(0.25, 0.75)
    );
}

// ---- Routing ----

#[test]
fn execution_order_rejects_cycles() {
    let links = vec![
        ModuleLink::link(1, ModuleInput::new(Input::Audio, 2)),
        ModuleLink::link(2, ModuleInput::new(Input::Audio, 1)),
    ];

    assert!(SynthEngine::calc_execution_order(&links).is_err());
}

#[test]
fn execution_order_places_output_last() {
    let links = vec![
        ModuleLink::link(
            HARMONIC_EDITOR_ID,
            ModuleInput::new(Input::Spectrum, OSCILLATOR_ID),
        ),
        ModuleLink::link(
            OSCILLATOR_ID,
            ModuleInput::new(Input::Audio, OUTPUT_MODULE_ID),
        ),
    ];

    let order = SynthEngine::calc_execution_order(&links).expect("valid graph");
    assert_eq!(*order.last().unwrap(), OUTPUT_MODULE_ID);
    assert_eq!(order.len(), 3);
}

#[test]
fn add_module_at_runtime() {
    let mut engine = make_engine(
        EngineParams::default(),
        OscillatorConfig {
            id: OSCILLATOR_ID,
            ..OscillatorConfig::default()
        },
    );

    let amp_id = engine.add_amplifier();
    let osc_to_out = ModuleInput::new(Input::Audio, OUTPUT_MODULE_ID);

    engine.remove_link(&OSCILLATOR_ID, &osc_to_out);
    engine
        .set_direct_link(OSCILLATOR_ID, ModuleInput::new(Input::Audio, amp_id))
        .expect("osc -> amp");
    engine
        .set_direct_link(amp_id, osc_to_out)
        .expect("amp -> output");

    engine
        .get_typed_module_mut::<Amplifier>(amp_id)
        .expect("amplifier module")
        .set_gain(StereoSample::ONE);

    assert!(engine.get_typed_module::<Amplifier>(amp_id).is_some());

    engine.handle_note_on(0, 60, 1.0);

    let (left, _right) = process_block(&mut engine, 64);
    assert!(rms(&left) > 1e-6);
}

#[test]
fn add_link_is_idempotent() {
    let mut engine = make_engine(
        EngineParams::default(),
        OscillatorConfig {
            id: OSCILLATOR_ID,
            ..OscillatorConfig::default()
        },
    );

    let dst = ModuleInput::new(Input::Spectrum, OSCILLATOR_ID);

    engine
        .add_link(HARMONIC_EDITOR_ID, dst, StereoSample::ONE)
        .expect("first link");
    engine
        .add_link(HARMONIC_EDITOR_ID, dst, StereoSample::ONE)
        .expect("duplicate link");

    assert_eq!(
        engine
            .get_config()
            .links
            .iter()
            .filter(|link| link.src_id == HARMONIC_EDITOR_ID && link.dst_id == OSCILLATOR_ID)
            .count(),
        1
    );
}

#[test]
fn set_direct_link_replaces_existing_source() {
    let mut engine = make_engine(
        EngineParams::default(),
        OscillatorConfig {
            id: OSCILLATOR_ID,
            ..OscillatorConfig::default()
        },
    );

    let harmonic_b = engine.add_harmonic_editor();
    let dst = ModuleInput::new(Input::Spectrum, OSCILLATOR_ID);

    engine
        .set_direct_link(harmonic_b, dst)
        .expect("replace spectrum source");

    assert!(
        engine
            .get_config()
            .links
            .iter()
            .any(|link| link.src_id == harmonic_b && link.dst_id == OSCILLATOR_ID)
    );
    assert!(
        !engine
            .get_config()
            .links
            .iter()
            .any(|link| link.src_id == HARMONIC_EDITOR_ID && link.dst_id == OSCILLATOR_ID)
    );
}

#[test]
fn remove_link_disconnects_modules() {
    let mut engine = make_engine(
        EngineParams::default(),
        OscillatorConfig {
            id: OSCILLATOR_ID,
            ..OscillatorConfig::default()
        },
    );

    let dst = ModuleInput::new(Input::Spectrum, OSCILLATOR_ID);
    engine.remove_link(&HARMONIC_EDITOR_ID, &dst);

    assert!(
        !engine
            .get_config()
            .links
            .iter()
            .any(|link| link.src_id == HARMONIC_EDITOR_ID && link.dst_id == OSCILLATOR_ID)
    );
}

#[test]
fn update_link_amount_changes_routing() {
    let mut engine = make_engine(
        EngineParams::default(),
        OscillatorConfig {
            id: OSCILLATOR_ID,
            ..OscillatorConfig::default()
        },
    );

    let dst = ModuleInput::new(Input::Spectrum, OSCILLATOR_ID);
    engine.update_link_amount(&HARMONIC_EDITOR_ID, &dst, StereoSample::splat(0.5));

    let cfg = engine.get_config();
    let link = cfg
        .links
        .iter()
        .find(|link| link.src_id == HARMONIC_EDITOR_ID && link.dst_id == OSCILLATOR_ID)
        .expect("harmonic -> osc link");

    assert_eq!(link.amount, StereoSample::splat(0.5));
}

#[test]
fn link_rejects_type_mismatch() {
    let mut engine = make_engine(
        EngineParams::default(),
        OscillatorConfig {
            id: OSCILLATOR_ID,
            ..OscillatorConfig::default()
        },
    );

    let err = engine
        .set_direct_link(
            HARMONIC_EDITOR_ID,
            ModuleInput::new(Input::Audio, OUTPUT_MODULE_ID),
        )
        .expect_err("spectral source cannot drive audio output");

    assert!(err.contains("mismatch") || err.contains("Invalid"));
}

#[test]
fn remove_module_rebuilds_routing() {
    let mut engine = make_engine(
        EngineParams::default(),
        OscillatorConfig {
            id: OSCILLATOR_ID,
            ..OscillatorConfig::default()
        },
    );

    engine.remove_module(HARMONIC_EDITOR_ID);

    assert!(
        engine
            .get_typed_module::<HarmonicEditor>(HARMONIC_EDITOR_ID)
            .is_none()
    );
    assert!(
        !engine
            .get_config()
            .links
            .iter()
            .any(|link| link.src_id == HARMONIC_EDITOR_ID)
    );
}

// ---- Process & MIDI ----

#[test]
fn process_is_silent_without_notes() {
    let mut engine = make_engine(
        EngineParams::default(),
        OscillatorConfig {
            id: OSCILLATOR_ID,
            ..OscillatorConfig::default()
        },
    );

    let (left, right) = process_block(&mut engine, 64);

    assert!(left.iter().all(|&s| s == 0.0));
    assert!(right.iter().all(|&s| s == 0.0));
}

#[test]
fn process_produces_audio_after_note_on() {
    let mut engine = make_engine(
        EngineParams::default(),
        OscillatorConfig {
            id: OSCILLATOR_ID,
            ..OscillatorConfig::default()
        },
    );

    engine.handle_note_on(0, 60, 1.0);

    let (left, right) = process_block(&mut engine, 64);

    assert!(rms(&left) > 1e-6);
    assert!(rms(&right) > 1e-6);
    assert!(left.iter().all(|s| s.is_finite()));
}

#[test]
fn note_on_off_and_retrigger_processes() {
    let mut engine = make_engine(
        EngineParams {
            num_voices: 2,
            ..EngineParams::default()
        },
        OscillatorConfig {
            id: OSCILLATOR_ID,
            ..OscillatorConfig::default()
        },
    );

    engine.handle_note_on(0, 60, 1.0);
    process_block(&mut engine, 64);

    engine.handle_note_off(0, 60, 0.0);
    let (left, _) = process_block(&mut engine, 64);
    assert!(left.iter().all(|s| s.is_finite()));

    engine.handle_note_on(0, 64, 1.0);
    let (left, _) = process_block(&mut engine, 64);
    assert!(rms(&left) > 1e-6);
}

#[test]
fn polyphonic_notes_mix_to_output() {
    let mut engine = make_engine(
        EngineParams {
            num_voices: 4,
            ..EngineParams::default()
        },
        OscillatorConfig {
            id: OSCILLATOR_ID,
            ..OscillatorConfig::default()
        },
    );

    engine.handle_note_on(0, 60, 1.0);
    engine.handle_note_on(0, 64, 1.0);
    engine.handle_note_on(0, 67, 1.0);

    let (left, _) = process_block(&mut engine, 64);
    assert!(rms(&left) > 1e-6);
}
