use std::sync::Arc;

use nih_plug::prelude::*;

use super::*;
use crate::{
    synth_engine::{
        amplifier::AmplifierConfig, envelope::EnvelopeConfig, expressions::ExpressionsConfig,
        external_param::ExternalParamConfig, harmonic_editor::HarmonicEditorConfig, lfo::LfoConfig,
        mixer::MixerConfig, oscillator::OscillatorConfig, spectral_blend::SpectralBlendConfig,
        spectral_filter::SpectralFilterConfig, spectral_mixer::SpectralMixerConfig,
        wave_shaper::WaveShaperConfig,
    },
    utils::from_ms,
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
            LinkConfig::direct(HARMONIC_EDITOR_ID, OSCILLATOR_ID, Input::Spectrum),
            LinkConfig::direct(OSCILLATOR_ID, OUTPUT_MODULE_ID, Input::Audio),
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
    process_block_with_ui(engine, samples, false)
}

fn process_block_with_ui(
    engine: &mut SynthEngine,
    samples: usize,
    update_ui: bool,
) -> (Vec<Sample>, Vec<Sample>) {
    let mut left = vec![0.0; samples];
    let mut right = vec![0.0; samples];

    engine.process(samples, update_ui, &mut [&mut left[..], &mut right[..]]);

    (left, right)
}

fn rms(samples: &[Sample]) -> Sample {
    (samples.iter().map(|s| s * s).sum::<Sample>() / samples.len() as Sample).sqrt()
}

fn link(src_id: ModuleId, dst_id: ModuleId, dst_input: Input) -> LinkConfig {
    match dst_input {
        Input::Audio
        | Input::AudioMix(_)
        | Input::Spectrum
        | Input::SpectrumMix(_)
        | Input::SpectrumTo => LinkConfig::direct(src_id, dst_id, dst_input),
        _ => LinkConfig::mixed(src_id, dst_id, dst_input, StereoSample::ONE),
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

    assert!(matches!(
        engine.get_module(HARMONIC_EDITOR_ID),
        Some(ModuleHandle::HarmonicEditor(_))
    ));
    assert!(matches!(
        engine.get_module(OSCILLATOR_ID),
        Some(ModuleHandle::Oscillator(_))
    ));
    assert!(matches!(
        engine.get_module(OUTPUT_MODULE_ID),
        Some(ModuleHandle::Output(_))
    ));
}

#[test]
fn try_new_builds_full_patch() {
    let engine = make_full_patch_engine(EngineParams::default());
    let cfg = engine.get_config();

    assert_eq!(cfg.modules.len(), 16);
    assert_eq!(cfg.links.len(), 17);

    assert!(matches!(
        engine.get_module(HE0_ID),
        Some(ModuleHandle::HarmonicEditor(_))
    ));
    assert!(matches!(
        engine.get_module(HE1_ID),
        Some(ModuleHandle::HarmonicEditor(_))
    ));
    assert!(matches!(
        engine.get_module(HE2_ID),
        Some(ModuleHandle::HarmonicEditor(_))
    ));
    assert!(matches!(
        engine.get_module(SPECTRAL_MIXER_ID),
        Some(ModuleHandle::SpectralMixer(_))
    ));
    assert!(matches!(
        engine.get_module(SPECTRAL_BLEND_ID),
        Some(ModuleHandle::SpectralBlend(_))
    ));
    assert!(matches!(
        engine.get_module(SPECTRAL_FILTER_ID),
        Some(ModuleHandle::SpectralFilter(_))
    ));
    assert!(matches!(
        engine.get_module(ENVELOPE_FILTER_ID),
        Some(ModuleHandle::Envelope(_))
    ));
    assert!(matches!(
        engine.get_module(ENVELOPE_AMP_ID),
        Some(ModuleHandle::Envelope(_))
    ));
    assert!(matches!(
        engine.get_module(OSC0_ID),
        Some(ModuleHandle::Oscillator(_))
    ));
    assert!(matches!(
        engine.get_module(OSC1_ID),
        Some(ModuleHandle::Oscillator(_))
    ));
    assert!(matches!(
        engine.get_module(LFO_ID),
        Some(ModuleHandle::Lfo(_))
    ));
    assert!(matches!(
        engine.get_module(MIXER_ID),
        Some(ModuleHandle::Mixer(_))
    ));
    assert!(matches!(
        engine.get_module(AMPLIFIER_ID),
        Some(ModuleHandle::Amplifier(_))
    ));
    assert!(matches!(
        engine.get_module(WAVE_SHAPER_ID),
        Some(ModuleHandle::WaveShaper(_))
    ));
    assert!(matches!(
        engine.get_module(EXTERNAL_PARAM_ID),
        Some(ModuleHandle::ExternalParam(_))
    ));
    assert!(matches!(
        engine.get_module(EXPRESSIONS_ID),
        Some(ModuleHandle::Expressions(_))
    ));
    assert!(matches!(
        engine.get_module(OUTPUT_MODULE_ID),
        Some(ModuleHandle::Output(_))
    ));

    let order = SynthEngine::calc_execution_order(
        &cfg.links
            .iter()
            .map(ModuleLink::from_config)
            .collect::<Vec<_>>(),
        [],
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

    engine.handle_note_on(0, 60, 1.0, 0);

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
fn try_new_skips_invalid_link() {
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
        links: vec![LinkConfig::direct(
            // Harmonic editor outputs spectrum, not audio — cannot feed the output module directly.
            HARMONIC_EDITOR_ID,
            OUTPUT_MODULE_ID,
            Input::Audio,
        )],
    };

    let engine = SynthEngine::try_new(&config, volume, external_params, SAMPLE_RATE)
        .expect("invalid links are skipped on load");

    assert!(
        engine
            .get_config()
            .links
            .iter()
            .all(|link| !(link.src_id() == HARMONIC_EDITOR_ID && link.dst_id() == OUTPUT_MODULE_ID))
    );
}

#[test]
fn try_new_skips_link_with_missing_modulator() {
    let mut config = full_patch_engine_config(EngineParams::default());
    let modulated = config
        .links
        .iter()
        .position(|link| link.src_id() == ENVELOPE_AMP_ID && link.dst_id() == AMPLIFIER_ID)
        .expect("env -> amp link");
    config.links[modulated].set_modulator_id(Some(9999));

    let (volume, external_params) = test_deps();
    let engine = SynthEngine::try_new(&config, volume, external_params, SAMPLE_RATE)
        .expect("bad modulator skips that preset link");

    assert!(
        engine.get_config().links.iter().all(|link| {
            !(link.src_id() == ENVELOPE_AMP_ID
                && link.dst_id() == AMPLIFIER_ID
                && link.dst_input() == Input::Gain)
        }),
        "env -> amp gain link with invalid modulator must be skipped"
    );
}

#[test]
fn try_new_skips_link_with_incompatible_modulator() {
    let mut config = full_patch_engine_config(EngineParams::default());
    let modulated = config
        .links
        .iter()
        .position(|link| link.src_id() == ENVELOPE_AMP_ID && link.dst_id() == AMPLIFIER_ID)
        .expect("env -> amp link");
    // Spectral source cannot modulate a control (gain) input.
    config.links[modulated].set_modulator_id(Some(HE0_ID));

    let (volume, external_params) = test_deps();
    let engine = SynthEngine::try_new(&config, volume, external_params, SAMPLE_RATE)
        .expect("incompatible modulator skips that preset link");

    assert!(engine.get_config().links.iter().all(|link| {
        !(link.src_id() == ENVELOPE_AMP_ID
            && link.dst_id() == AMPLIFIER_ID
            && link.dst_input() == Input::Gain)
    }));
}

#[test]
fn set_config_links_direct_exclusivity_keeps_last_source() {
    let mut config = full_patch_engine_config(EngineParams::default());
    // full_patch already has HE0 -> OSC1.Spectrum; a later Direct replaces it.
    config.links.push(link(HE1_ID, OSC1_ID, Input::Spectrum));

    let (volume, external_params) = test_deps();
    let mut engine = SynthEngine::try_new(&config, volume, external_params, SAMPLE_RATE)
        .expect("extra spectral sources collapse to one");

    let cfg = engine.get_config();
    let spectrum_links: Vec<_> = cfg
        .links
        .iter()
        .filter(|link| link.dst_id() == OSC1_ID && link.dst_input() == Input::Spectrum)
        .collect();

    assert_eq!(spectrum_links.len(), 1);
    assert_eq!(spectrum_links[0].src_id(), HE1_ID);

    engine.handle_note_on(0, 60, 1.0, 0);
    let (left, right) = process_block(&mut engine, 64);
    assert!(left.iter().chain(right.iter()).all(|s| s.is_finite()));
}

#[test]
fn set_config_links_skips_mixed_kind_on_direct_input() {
    let mut config = full_patch_engine_config(EngineParams::default());
    config.links.push(LinkConfig::mixed(
        HE1_ID,
        OSC1_ID,
        Input::Spectrum,
        StereoSample::ONE,
    ));

    let (volume, external_params) = test_deps();
    let engine = SynthEngine::try_new(&config, volume, external_params, SAMPLE_RATE)
        .expect("wrong-kind mixed link should be skipped");

    let cfg = engine.get_config();
    let spectrum_links: Vec<_> = cfg
        .links
        .iter()
        .filter(|link| link.dst_id() == OSC1_ID && link.dst_input() == Input::Spectrum)
        .collect();

    assert_eq!(spectrum_links.len(), 1);
    assert_eq!(spectrum_links[0].src_id(), HE0_ID);
    assert!(matches!(spectrum_links[0], LinkConfig::Direct { .. }));
}

#[test]
fn add_link_rejects_direct_input() {
    let mut engine = make_engine(
        EngineParams::default(),
        OscillatorConfig {
            id: OSCILLATOR_ID,
            ..OscillatorConfig::default()
        },
    );

    let err = engine
        .add_mixed_link(
            HARMONIC_EDITOR_ID,
            InputId::new(Input::Spectrum, OSCILLATOR_ID),
            StereoSample::ONE,
        )
        .expect_err("spectrum is a direct input");

    assert!(err.contains("Direct") || err.contains("set_direct_link"));
}

#[test]
fn set_direct_link_replaces_spectral_source() {
    let mut engine = make_full_patch_engine(EngineParams::default());
    let dst = InputId::new(Input::Spectrum, OSC1_ID);

    engine
        .set_direct_link(HE1_ID, dst)
        .expect("second spectral source replaces the first");

    let cfg = engine.get_config();
    let spectrum_links: Vec<_> = cfg
        .links
        .iter()
        .filter(|link| link.dst_id() == OSC1_ID && link.dst_input() == Input::Spectrum)
        .collect();

    assert_eq!(spectrum_links.len(), 1);
    assert_eq!(spectrum_links[0].src_id(), HE1_ID);

    engine.handle_note_on(0, 60, 1.0, 0);
    let (left, _) = process_block(&mut engine, 64);
    assert!(left.iter().all(|s| s.is_finite()));
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
        ModuleLink::direct(1, InputId::new(Input::Audio, 2)),
        ModuleLink::direct(2, InputId::new(Input::Audio, 1)),
    ];

    assert!(SynthEngine::calc_execution_order(&links, []).is_err());
}

#[test]
fn execution_order_places_output_last() {
    let links = vec![
        ModuleLink::direct(
            HARMONIC_EDITOR_ID,
            InputId::new(Input::Spectrum, OSCILLATOR_ID),
        ),
        ModuleLink::direct(OSCILLATOR_ID, InputId::new(Input::Audio, OUTPUT_MODULE_ID)),
    ];

    let order = SynthEngine::calc_execution_order(&links, []).expect("valid graph");
    assert_eq!(*order.last().unwrap(), OUTPUT_MODULE_ID);
    assert_eq!(order.len(), 3);
}

#[test]
fn execution_order_includes_unlinked_modules() {
    let links = vec![ModuleLink::direct(
        OSCILLATOR_ID,
        InputId::new(Input::Audio, OUTPUT_MODULE_ID),
    )];

    let order =
        SynthEngine::calc_execution_order(&links, [LFO_ID, OSCILLATOR_ID, OUTPUT_MODULE_ID])
            .expect("valid graph");

    assert!(order.contains(&LFO_ID));
    assert!(order.contains(&OSCILLATOR_ID));
    assert!(order.contains(&OUTPUT_MODULE_ID));
    assert!(
        order.iter().position(|&id| id == OSCILLATOR_ID).unwrap()
            < order.iter().position(|&id| id == OUTPUT_MODULE_ID).unwrap()
    );
}

#[test]
fn add_module_appends_to_execution_order() {
    let mut engine = make_engine(
        EngineParams::default(),
        OscillatorConfig {
            id: OSCILLATOR_ID,
            ..OscillatorConfig::default()
        },
    );

    let lfo_id = engine.add_lfo();
    assert_eq!(*engine.execution_order.last().unwrap(), lfo_id);
    assert!(engine.execution_order.contains(&OUTPUT_MODULE_ID));
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
    let osc_to_out = InputId::new(Input::Audio, OUTPUT_MODULE_ID);

    engine.remove_link(&OSCILLATOR_ID, &osc_to_out);
    engine
        .set_direct_link(OSCILLATOR_ID, InputId::new(Input::Audio, amp_id))
        .expect("osc -> amp");
    engine
        .set_direct_link(amp_id, osc_to_out)
        .expect("amp -> output");

    match engine.get_module_mut(amp_id) {
        Some(ModuleHandle::Amplifier(amp)) => amp.set_gain(StereoSample::ONE),
        _ => panic!("amplifier module"),
    }

    assert!(matches!(
        engine.get_module(amp_id),
        Some(ModuleHandle::Amplifier(_))
    ));

    engine.handle_note_on(0, 60, 1.0, 0);

    let (left, _right) = process_block(&mut engine, 64);
    assert!(rms(&left) > 1e-6);
}

#[test]
fn add_link_overrides_existing_link() {
    let mut engine = make_engine(
        EngineParams::default(),
        OscillatorConfig {
            id: OSCILLATOR_ID,
            ..OscillatorConfig::default()
        },
    );

    let lfo_id = engine.add_lfo();
    let dst = InputId::new(Input::Gain, OSCILLATOR_ID);

    engine
        .add_mixed_link(lfo_id, dst, StereoSample::ONE)
        .expect("first link");
    engine
        .add_mixed_link(lfo_id, dst, StereoSample::splat(0.25))
        .expect("override link");

    let cfg = engine.get_config();
    let links: Vec<_> = cfg
        .links
        .iter()
        .filter(|link| link.src_id() == lfo_id && link.dst_id() == OSCILLATOR_ID)
        .collect();

    assert_eq!(links.len(), 1);
    assert_eq!(links[0].amount(), StereoSample::splat(0.25));
    assert!(links[0].modulator_id().is_none());
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
    let dst = InputId::new(Input::Spectrum, OSCILLATOR_ID);

    engine
        .set_direct_link(harmonic_b, dst)
        .expect("replace spectrum source");

    assert!(
        engine
            .get_config()
            .links
            .iter()
            .any(|link| link.src_id() == harmonic_b && link.dst_id() == OSCILLATOR_ID)
    );
    assert!(
        !engine
            .get_config()
            .links
            .iter()
            .any(|link| link.src_id() == HARMONIC_EDITOR_ID && link.dst_id() == OSCILLATOR_ID)
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

    let dst = InputId::new(Input::Spectrum, OSCILLATOR_ID);
    engine.remove_link(&HARMONIC_EDITOR_ID, &dst);

    assert!(
        !engine
            .get_config()
            .links
            .iter()
            .any(|link| link.src_id() == HARMONIC_EDITOR_ID && link.dst_id() == OSCILLATOR_ID)
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

    let lfo_id = engine.add_lfo();
    let dst = InputId::new(Input::Gain, OSCILLATOR_ID);
    engine
        .add_mixed_link(lfo_id, dst, StereoSample::ONE)
        .expect("gain modulation link");
    engine.update_link_amount(&lfo_id, &dst, StereoSample::splat(0.5));

    let cfg = engine.get_config();
    let link = cfg
        .links
        .iter()
        .find(|link| link.src_id() == lfo_id && link.dst_id() == OSCILLATOR_ID)
        .expect("lfo -> osc gain link");

    assert_eq!(link.amount(), StereoSample::splat(0.5));
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
            InputId::new(Input::Audio, OUTPUT_MODULE_ID),
        )
        .expect_err("spectral source cannot drive audio output");

    assert!(err.contains("mismatch") || err.contains("Invalid"));
}

#[test]
fn set_direct_link_rejects_mixed_input() {
    let mut engine = make_engine(
        EngineParams::default(),
        OscillatorConfig {
            id: OSCILLATOR_ID,
            ..OscillatorConfig::default()
        },
    );
    let lfo_id = engine.add_lfo();

    let err = engine
        .set_direct_link(lfo_id, InputId::new(Input::Gain, OSCILLATOR_ID))
        .expect_err("gain is a mixed input");

    assert!(err.contains("Mixed") || err.contains("add_mixed_link"));
}

#[test]
fn add_mixed_link_allows_control_into_audio_input() {
    let mut engine = make_engine(
        EngineParams::default(),
        OscillatorConfig {
            id: OSCILLATOR_ID,
            ..OscillatorConfig::default()
        },
    );
    let lfo_id = engine.add_lfo();

    engine
        .add_mixed_link(
            lfo_id,
            InputId::new(Input::PhaseShift, OSCILLATOR_ID),
            StereoSample::ONE,
        )
        .expect("control may drive mixed audio PhaseShift");
}

#[test]
fn add_mixed_link_rejects_audio_into_control() {
    let mut engine = make_engine(
        EngineParams::default(),
        OscillatorConfig {
            id: OSCILLATOR_ID,
            ..OscillatorConfig::default()
        },
    );
    let env_id = engine.add_envelope();

    let err = engine
        .add_mixed_link(
            OSCILLATOR_ID,
            InputId::new(Input::Attack, env_id),
            StereoSample::ONE,
        )
        .expect_err("audio cannot drive control Attack");

    assert!(err.contains("mismatch") || err.contains("Data types"));
}

#[test]
fn add_mixed_link_clears_src_as_modulator_on_same_dst() {
    let mut engine = make_full_patch_engine(EngineParams::default());
    let gain = InputId::new(Input::Gain, AMPLIFIER_ID);

    engine
        .set_link_modulation(ENVELOPE_AMP_ID, &gain, LFO_ID)
        .expect("lfo modulates amp-env -> gain");

    engine
        .add_mixed_link(LFO_ID, gain, StereoSample::splat(0.5))
        .expect("lfo becomes a gain source");

    let cfg = engine.get_config();
    let env_link = cfg
        .links
        .iter()
        .find(|link| link.src_id() == ENVELOPE_AMP_ID && link.dst_input() == Input::Gain)
        .expect("env -> gain kept");
    assert!(env_link.modulator_id().is_none());

    let lfo_link = cfg
        .links
        .iter()
        .find(|link| link.src_id() == LFO_ID && link.dst_input() == Input::Gain)
        .expect("lfo -> gain");
    assert!(lfo_link.modulator_id().is_none());
}

#[test]
fn set_link_modulation_rejects_direct_edge() {
    let mut engine = make_engine(
        EngineParams::default(),
        OscillatorConfig {
            id: OSCILLATOR_ID,
            ..OscillatorConfig::default()
        },
    );
    let lfo_id = engine.add_lfo();
    let spectrum = InputId::new(Input::Spectrum, OSCILLATOR_ID);

    let err = engine
        .set_link_modulation(HARMONIC_EDITOR_ID, &spectrum, lfo_id)
        .expect_err("direct spectrum link cannot be modulated");

    assert!(err.contains("Direct") || err.contains("modulat") || err.contains("Invalid"));
}

#[test]
fn cyclic_direct_links_rejected() {
    let mut engine = make_engine(
        EngineParams::default(),
        OscillatorConfig {
            id: OSCILLATOR_ID,
            ..OscillatorConfig::default()
        },
    );
    let amp_a = engine.add_amplifier();
    let amp_b = engine.add_amplifier();

    engine
        .set_direct_link(amp_a, InputId::new(Input::Audio, amp_b))
        .expect("a -> b");

    let err = engine
        .set_direct_link(amp_b, InputId::new(Input::Audio, amp_a))
        .expect_err("b -> a would cycle");

    assert!(err.contains("Cycles"));

    // First link must remain after the failed update.
    assert!(
        engine
            .get_config()
            .links
            .iter()
            .any(|link| link.src_id() == amp_a && link.dst_id() == amp_b)
    );
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

    assert!(!matches!(
        engine.get_module(HARMONIC_EDITOR_ID),
        Some(ModuleHandle::HarmonicEditor(_))
    ));
    assert!(
        !engine
            .get_config()
            .links
            .iter()
            .any(|link| link.src_id() == HARMONIC_EDITOR_ID)
    );
}

#[test]
fn remove_module_clears_modulation_source() {
    let mut engine = make_full_patch_engine(EngineParams::default());
    let gain_dst = InputId::new(Input::Gain, AMPLIFIER_ID);

    engine
        .set_link_modulation(ENVELOPE_AMP_ID, &gain_dst, LFO_ID)
        .expect("attach lfo as gain modulator");

    engine.remove_module(LFO_ID);

    let cfg = engine.get_config();
    let link = cfg
        .links
        .iter()
        .find(|link| link.src_id() == ENVELOPE_AMP_ID && link.dst_id() == AMPLIFIER_ID)
        .expect("env -> amp gain link kept");
    assert!(link.modulator_id().is_none());
    assert!(engine.get_module(LFO_ID).is_none());
}

#[test]
fn remove_output_links_clears_modulation_source() {
    let mut engine = make_full_patch_engine(EngineParams::default());
    let gain_dst = InputId::new(Input::Gain, AMPLIFIER_ID);

    engine
        .set_link_modulation(ENVELOPE_AMP_ID, &gain_dst, LFO_ID)
        .expect("attach lfo as gain modulator");

    engine.remove_output_links(LFO_ID);

    let cfg = engine.get_config();
    let link = cfg
        .links
        .iter()
        .find(|link| link.src_id() == ENVELOPE_AMP_ID && link.dst_id() == AMPLIFIER_ID)
        .expect("env -> amp gain link kept");
    assert!(link.modulator_id().is_none());
    assert!(
        !cfg.links.iter().any(|link| link.src_id() == LFO_ID),
        "direct lfo outputs removed"
    );
    assert!(engine.get_module(LFO_ID).is_some());
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

    engine.handle_note_on(0, 60, 1.0, 0);

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

    engine.handle_note_on(0, 60, 1.0, 0);
    process_block(&mut engine, 64);

    engine.handle_note_off(0, 60, 0.0, 0);
    let (left, _) = process_block(&mut engine, 64);
    assert!(left.iter().all(|s| s.is_finite()));

    engine.handle_note_on(0, 64, 1.0, 0);
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

    engine.handle_note_on(0, 60, 1.0, 0);
    engine.handle_note_on(0, 64, 1.0, 0);
    engine.handle_note_on(0, 67, 1.0, 0);

    let (left, _) = process_block(&mut engine, 64);
    assert!(rms(&left) > 1e-6);
}

// ---- Extended coverage ----

#[test]
fn full_patch_config_round_trips() {
    let engine = make_full_patch_engine(EngineParams {
        num_voices: 2,
        block_size: 64,
        ..EngineParams::default()
    });
    let cfg = engine.get_config();
    let (volume, external_params) = test_deps();

    let rebuilt = SynthEngine::try_new(&cfg, volume, external_params, SAMPLE_RATE)
        .expect("full patch config deserializes");

    assert_eq!(rebuilt.get_config().modules.len(), cfg.modules.len());
    assert_eq!(rebuilt.get_config().links.len(), cfg.links.len());
    assert_eq!(rebuilt.get_config().engine.block_size, 64);
}

#[test]
fn engine_extended_setters_round_trip() {
    let mut engine = make_engine(
        EngineParams::default(),
        OscillatorConfig {
            id: OSCILLATOR_ID,
            ..OscillatorConfig::default()
        },
    );

    let kill_time = from_ms(20.0);
    engine.set_num_voices(3);
    engine.set_legato(true);
    engine.set_block_size(32);
    engine.set_voice_kill_time(kill_time);
    engine.set_oversampling(true);
    engine.set_stereo_spectrum(false);
    engine.set_output_gain(StereoSample::splat(0.5));

    let lfo_id = engine.add_lfo();
    let dst = InputId::new(Input::Gain, OSCILLATOR_ID);
    engine
        .add_mixed_link(lfo_id, dst, StereoSample::ONE)
        .expect("gain link");
    engine.update_link_amount(&lfo_id, &dst, StereoSample::splat(0.25));

    let cfg = engine.get_config();
    assert_eq!(cfg.engine.num_voices, 3);
    assert!(cfg.engine.legato);
    assert_eq!(cfg.engine.block_size, 32);
    assert_eq!(engine.get_voice_kill_time(), kill_time);
    assert!(cfg.engine.oversampling);
    assert!(!cfg.engine.stereo_spectrum);
    assert_eq!(cfg.engine.output_gain, StereoSample::splat(0.5));

    let link = cfg
        .links
        .iter()
        .find(|link| link.src_id() == lfo_id && link.dst_id() == OSCILLATOR_ID)
        .expect("lfo -> osc gain link");
    assert_eq!(link.amount(), StereoSample::splat(0.25));
}

#[test]
fn process_with_update_ui_runs() {
    let mut engine = make_engine(
        EngineParams::default(),
        OscillatorConfig {
            id: OSCILLATOR_ID,
            ..OscillatorConfig::default()
        },
    );

    engine.handle_note_on(0, 60, 1.0, 0);
    let (left, _) = process_block_with_ui(&mut engine, 64, true);
    assert!(rms(&left) > 1e-6);
}

#[test]
fn add_link_connects_new_modules() {
    let mut engine = make_engine(
        EngineParams::default(),
        OscillatorConfig {
            id: OSCILLATOR_ID,
            ..OscillatorConfig::default()
        },
    );

    let amp_id = engine.add_amplifier();
    let env_id = engine.add_envelope();
    let osc_to_out = InputId::new(Input::Audio, OUTPUT_MODULE_ID);

    engine.remove_link(&OSCILLATOR_ID, &osc_to_out);
    engine
        .set_direct_link(OSCILLATOR_ID, InputId::new(Input::Audio, amp_id))
        .expect("osc -> amp");
    engine
        .set_direct_link(amp_id, osc_to_out)
        .expect("amp -> output");
    engine
        .add_mixed_link(env_id, InputId::new(Input::Gain, amp_id), StereoSample::ONE)
        .expect("env -> amp gain");

    match engine.get_module_mut(amp_id) {
        Some(ModuleHandle::Amplifier(amp)) => amp.set_gain(StereoSample::ONE),
        _ => panic!("amplifier"),
    }

    assert!(
        engine
            .get_config()
            .links
            .iter()
            .any(|link| link.src_id() == env_id && link.dst_id() == amp_id)
    );

    engine.handle_note_on(0, 60, 1.0, 0);
    let (left, _) = process_block(&mut engine, 64);
    assert!(rms(&left) > 1e-6);
}

#[test]
fn link_modulation_round_trips_in_config() {
    let mut engine = make_full_patch_engine(EngineParams::default());
    let gain_dst = InputId::new(Input::Gain, AMPLIFIER_ID);

    engine
        .set_link_modulation(ENVELOPE_AMP_ID, &gain_dst, LFO_ID)
        .expect("attach lfo as gain modulator");

    let cfg = engine.get_config();
    let link = cfg
        .links
        .iter()
        .find(|link| link.src_id() == ENVELOPE_AMP_ID && link.dst_id() == AMPLIFIER_ID)
        .expect("env -> amp gain");
    assert_eq!(link.modulator_id(), Some(LFO_ID));

    engine.remove_link_modulation(ENVELOPE_AMP_ID, &gain_dst);

    let cfg = engine.get_config();
    let link = cfg
        .links
        .iter()
        .find(|link| link.src_id() == ENVELOPE_AMP_ID && link.dst_id() == AMPLIFIER_ID)
        .expect("env -> amp gain");
    assert!(link.modulator_id().is_none());
}

#[test]
fn link_modulation_in_preset_builds() {
    let mut config = full_patch_engine_config(EngineParams::default());
    config.links.push(LinkConfig::mixed(
        LFO_ID,
        OSC1_ID,
        Input::PitchShift,
        StereoSample::splat(0.5),
    ));

    let modulated = config
        .links
        .iter()
        .position(|link| link.src_id() == ENVELOPE_AMP_ID && link.dst_id() == AMPLIFIER_ID)
        .expect("env -> amp link");
    config.links[modulated].set_modulator_id(Some(LFO_ID));

    config.links.push(config.links[modulated].clone());

    let (volume, external_params) = test_deps();
    let mut engine = SynthEngine::try_new(&config, volume, external_params, SAMPLE_RATE)
        .expect("modulated preset");

    let cfg = engine.get_config();
    let env_amp_links: Vec<_> = cfg
        .links
        .iter()
        .filter(|link| link.src_id() == ENVELOPE_AMP_ID && link.dst_id() == AMPLIFIER_ID)
        .collect();
    assert_eq!(env_amp_links.len(), 1);
    assert_eq!(env_amp_links[0].modulator_id(), Some(LFO_ID));

    let order = SynthEngine::calc_execution_order(
        &cfg.links
            .iter()
            .map(ModuleLink::from_config)
            .collect::<Vec<_>>(),
        [],
    )
    .expect("execution order");

    assert!(
        order.iter().position(|&id| id == LFO_ID).unwrap()
            < order.iter().position(|&id| id == AMPLIFIER_ID).unwrap()
    );

    engine.handle_note_on(0, 60, 1.0, 0);
    let (left, _) = process_block(&mut engine, 64);
    assert!(rms(&left) > 1e-6);
}

#[test]
fn set_config_links_dedupes_duplicate_preset_links() {
    let mut config = full_patch_engine_config(EngineParams::default());
    let osc_out = link(OSC0_ID, MIXER_ID, Input::AudioMix(0));
    config.links.push(osc_out.clone());
    config.links.push(osc_out);

    let (volume, external_params) = test_deps();
    let engine = SynthEngine::try_new(&config, volume, external_params, SAMPLE_RATE)
        .expect("duplicate links should be skipped");

    let count = engine
        .get_config()
        .links
        .iter()
        .filter(|l| {
            l.src_id() == OSC0_ID && l.dst_id() == MIXER_ID && l.dst_input() == Input::AudioMix(0)
        })
        .count();
    assert_eq!(count, 1);
}

#[test]
fn refresh_routing_drops_links_to_removed_inputs() {
    let mut engine = make_full_patch_engine(EngineParams::default());

    assert!(engine.get_config().links.iter().any(|link| {
        link.src_id() == OSC1_ID
            && link.dst_id() == MIXER_ID
            && link.dst_input() == Input::AudioMix(1)
    }));

    match engine.get_module_mut(MIXER_ID) {
        Some(ModuleHandle::Mixer(mixer)) => mixer.set_num_inputs(1),
        _ => panic!("expected mixer"),
    }

    engine
        .refresh_routing()
        .expect("routing should rebuild after input meta shrink");

    let cfg = engine.get_config();
    assert!(cfg.links.iter().any(|link| {
        link.src_id() == OSC0_ID
            && link.dst_id() == MIXER_ID
            && link.dst_input() == Input::AudioMix(0)
    }));
    assert!(
        cfg.links
            .iter()
            .all(|link| { !(link.dst_id() == MIXER_ID && link.dst_input() == Input::AudioMix(1)) })
    );
}

#[test]
fn set_link_modulation_rejects_unknown_link() {
    let engine = make_full_patch_engine(EngineParams::default());
    let mut engine = engine;

    let err = engine
        .set_link_modulation(HE0_ID, &InputId::new(Input::Gain, AMPLIFIER_ID), LFO_ID)
        .expect_err("harmonic editor is not wired to amp gain");

    assert!(err.contains("Invalid"));
}

#[test]
fn set_link_modulation_cycle_leaves_state_unchanged() {
    let mut engine = make_full_patch_engine(EngineParams::default());
    let filter_attack = InputId::new(Input::Attack, ENVELOPE_FILTER_ID);
    let lfo_skew = InputId::new(Input::Skew, LFO_ID);

    // filter_env <- amp_env, and lfo <- filter_env; modulating the first link with the
    // lfo would require filter_env <-> lfo and must be rejected without mutating state.
    engine
        .add_mixed_link(ENVELOPE_AMP_ID, filter_attack, StereoSample::ONE)
        .expect("amp env -> filter env attack");
    engine
        .add_mixed_link(ENVELOPE_FILTER_ID, lfo_skew, StereoSample::ONE)
        .expect("filter env -> lfo skew");

    let err = engine
        .set_link_modulation(ENVELOPE_AMP_ID, &filter_attack, LFO_ID)
        .expect_err("lfo modulation would cycle through filter env");

    assert!(err.contains("Cycles"));

    let cfg = engine.get_config();
    let link = cfg
        .links
        .iter()
        .find(|link| {
            link.src_id() == ENVELOPE_AMP_ID
                && link.dst_id() == ENVELOPE_FILTER_ID
                && link.dst_input() == Input::Attack
        })
        .expect("amp env -> filter env link kept");
    assert!(link.modulator_id().is_none());
}

#[test]
fn dual_audio_sources_mix_via_mixer() {
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

    let osc_b = engine.add_oscillator();
    let harmonic_b = engine.add_harmonic_editor();
    let mixer_id = engine.add_mixer();
    let out_audio = InputId::new(Input::Audio, OUTPUT_MODULE_ID);

    engine.remove_link(&OSCILLATOR_ID, &out_audio);
    engine
        .set_direct_link(OSCILLATOR_ID, InputId::new(Input::AudioMix(0), mixer_id))
        .expect("osc a -> mixer");
    engine
        .set_direct_link(osc_b, InputId::new(Input::AudioMix(1), mixer_id))
        .expect("osc b -> mixer");
    engine
        .set_direct_link(mixer_id, out_audio)
        .expect("mixer -> output");
    engine
        .set_direct_link(harmonic_b, InputId::new(Input::Spectrum, osc_b))
        .expect("harmonic b -> osc b");

    engine.handle_note_on(0, 60, 1.0, 0);
    engine.handle_note_on(0, 64, 1.0, 0);

    let (left, _) = process_block(&mut engine, 64);
    assert!(rms(&left) > 1e-6);
}

#[test]
fn non_unity_link_amount_processes() {
    let mut engine = make_engine(
        EngineParams::default(),
        OscillatorConfig {
            id: OSCILLATOR_ID,
            ..OscillatorConfig::default()
        },
    );

    let dst = InputId::new(Input::Spectrum, OSCILLATOR_ID);
    engine.update_link_amount(&HARMONIC_EDITOR_ID, &dst, StereoSample::splat(0.5));

    engine.handle_note_on(0, 60, 1.0, 0);
    let (left, _) = process_block(&mut engine, 64);
    assert!(rms(&left) > 1e-6);
}

#[test]
fn runtime_add_all_module_types() {
    let mut engine = make_engine(
        EngineParams::default(),
        OscillatorConfig {
            id: OSCILLATOR_ID,
            ..OscillatorConfig::default()
        },
    );

    let ids = [
        engine.add_harmonic_editor(),
        engine.add_oscillator(),
        engine.add_envelope(),
        engine.add_lfo(),
        engine.add_amplifier(),
        engine.add_mixer(),
        engine.add_wave_shaper(),
        engine.add_spectral_filter(),
        engine.add_spectral_blend(),
        engine.add_spectral_mixer(),
        engine.add_expressions(),
        engine.add_external_param(),
    ];

    assert_eq!(ids.len(), 12);
    assert!(matches!(
        engine.get_module(ids[0]),
        Some(ModuleHandle::HarmonicEditor(_))
    ));
    assert!(matches!(
        engine.get_module(ids[1]),
        Some(ModuleHandle::Oscillator(_))
    ));
    assert!(matches!(
        engine.get_module(ids[2]),
        Some(ModuleHandle::Envelope(_))
    ));
    assert!(matches!(
        engine.get_module(ids[3]),
        Some(ModuleHandle::Lfo(_))
    ));
    assert!(matches!(
        engine.get_module(ids[4]),
        Some(ModuleHandle::Amplifier(_))
    ));
    assert!(matches!(
        engine.get_module(ids[5]),
        Some(ModuleHandle::Mixer(_))
    ));
    assert!(matches!(
        engine.get_module(ids[6]),
        Some(ModuleHandle::WaveShaper(_))
    ));
    assert!(matches!(
        engine.get_module(ids[7]),
        Some(ModuleHandle::SpectralFilter(_))
    ));
    assert!(matches!(
        engine.get_module(ids[8]),
        Some(ModuleHandle::SpectralBlend(_))
    ));
    assert!(matches!(
        engine.get_module(ids[9]),
        Some(ModuleHandle::SpectralMixer(_))
    ));
    assert!(matches!(
        engine.get_module(ids[10]),
        Some(ModuleHandle::Expressions(_))
    ));
    assert!(matches!(
        engine.get_module(ids[11]),
        Some(ModuleHandle::ExternalParam(_))
    ));
}

#[test]
fn remove_missing_module_is_noop() {
    let mut engine = make_engine(
        EngineParams::default(),
        OscillatorConfig {
            id: OSCILLATOR_ID,
            ..OscillatorConfig::default()
        },
    );

    let links_before = engine.get_config().links.len();
    engine.remove_module(9999);
    assert_eq!(engine.get_config().links.len(), links_before);
}

#[test]
fn link_rejects_invalid_module_id() {
    let mut engine = make_engine(
        EngineParams::default(),
        OscillatorConfig {
            id: OSCILLATOR_ID,
            ..OscillatorConfig::default()
        },
    );

    let dst = InputId::new(Input::Gain, OSCILLATOR_ID);
    let err = engine
        .add_mixed_link(9999, dst, StereoSample::ONE)
        .expect_err("unknown source module");

    assert!(err.contains("Invalid") || err.contains("mismatch"));
}

#[test]
fn handle_note_expression_and_choke_process() {
    let mut engine = make_full_patch_engine(EngineParams {
        num_voices: 2,
        ..EngineParams::default()
    });

    engine.handle_note_on(0, 60, 0.5, 0);
    engine.handle_note_expression(0, 60, Expression::Velocity, 0, 1.0);
    process_block(&mut engine, 64);

    engine.handle_choke(0, 60);
    let (left, _) = process_block(&mut engine, 64);
    assert!(left.iter().all(|s| s.is_finite()));
}

#[test]
fn oversampling_and_mono_spectrum_process() {
    let mut engine = make_engine(
        EngineParams {
            stereo_spectrum: false,
            ..EngineParams::default()
        },
        OscillatorConfig {
            id: OSCILLATOR_ID,
            ..OscillatorConfig::default()
        },
    );

    engine.set_oversampling(true);
    engine.handle_note_on(0, 60, 1.0, 0);

    let (left, right) = process_block(&mut engine, 64);
    assert!(rms(&left) > 1e-6);
    assert!(left.iter().chain(right.iter()).all(|s| s.is_finite()));
}

#[test]
fn execution_order_accounts_for_link_modulation() {
    let links = vec![
        ModuleLink::mixed(
            LFO_ID,
            InputId::new(Input::Gain, AMPLIFIER_ID),
            StereoSample::ONE,
        ),
        ModuleLink::mixed_modulated(
            ENVELOPE_AMP_ID,
            InputId::new(Input::Gain, AMPLIFIER_ID),
            StereoSample::ONE,
            Some(LFO_ID),
        ),
        ModuleLink::direct(AMPLIFIER_ID, InputId::new(Input::Audio, OUTPUT_MODULE_ID)),
    ];

    let order = SynthEngine::calc_execution_order(&links, []).expect("valid order");
    let lfo_pos = order.iter().position(|&id| id == LFO_ID).unwrap();
    let amp_pos = order.iter().position(|&id| id == AMPLIFIER_ID).unwrap();
    assert!(lfo_pos < amp_pos);
}
