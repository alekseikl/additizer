use super::{routing_helpers, *};
use crate::{
    synth_engine::{MAX_CUTOFF, MIN_CUTOFF, from_ms},
    synth_engine::{
        amplifier::AmplifierConfig, envelope::EnvelopeConfig, expressions::ExpressionsConfig,
        external_param::ExternalParamConfig, harmonic_editor::HarmonicEditorConfig, lfo::LfoConfig,
        mixer::MixerConfig, oscillator::OscillatorConfig, pitch::PitchConfig,
        spectral_blend::SpectralBlendConfig, spectral_filter::SpectralFilterConfig,
        spectral_mixer::SpectralMixerConfig, svf::SvfConfig, synth_module::SynthModule,
        wave_shaper::WaveShaperConfig,
    },
};
use addi_dsp::filters;
use rustc_hash::FxHashMap;

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
    let config = minimal_engine_config(engine, osc);

    <SynthEngine>::try_new(&config, SAMPLE_RATE).expect("valid engine config")
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
    let mut terminated = Vec::new();

    engine.process(
        samples,
        update_ui,
        &mut terminated,
        [&mut left[..], &mut right[..]],
    );

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

fn set_link_modulator(link: &mut LinkConfig, modulator_id: Option<ModuleId>) {
    match link {
        LinkConfig::Mixed {
            modulator_id: slot, ..
        } => *slot = modulator_id,
        LinkConfig::Direct { .. } => panic!("expected mixed link"),
    }
}

fn link_amount(link: &LinkConfig) -> StereoSample {
    match *link {
        LinkConfig::Mixed { amount, .. } => amount,
        LinkConfig::Direct { .. } => panic!("expected mixed link"),
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
            link(LFO_ID, OSC1_ID, Input::Detune),
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
    let config = full_patch_engine_config(engine);

    <SynthEngine>::try_new(&config, SAMPLE_RATE).expect("valid full patch config")
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

    assert_eq!(cfg.modules.len(), 17);
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

    let order = routing_helpers::process_order(
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

    engine.handle_note_on(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: None,
        },
        0,
    );

    let (left, right) = process_block(&mut engine, 64);

    assert!(rms(&left) > 1e-6);
    assert!(rms(&right) > 1e-6);
    assert!(left.iter().all(|s| s.is_finite()));
    assert!(right.iter().all(|s| s.is_finite()));
}

#[test]
fn try_new_skips_duplicate_module_id() {
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

    let engine = <SynthEngine>::try_new(&config, SAMPLE_RATE).expect("duplicate ids are skipped");

    assert!(matches!(
        engine.get_module(1),
        Some(ModuleHandle::HarmonicEditor(_))
    ));
    assert!(matches!(
        engine.get_module(OUTPUT_MODULE_ID),
        Some(ModuleHandle::Output(_))
    ));
    assert!(matches!(
        engine.get_module(PITCH_MODULE_ID),
        Some(ModuleHandle::Pitch(_))
    ));
}

#[test]
fn try_new_skips_invalid_link() {
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

    let engine =
        <SynthEngine>::try_new(&config, SAMPLE_RATE).expect("invalid links are skipped on load");

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
    set_link_modulator(&mut config.links[modulated], Some(9999));

    let engine =
        <SynthEngine>::try_new(&config, SAMPLE_RATE).expect("bad modulator skips that preset link");

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
    set_link_modulator(&mut config.links[modulated], Some(HE0_ID));

    let engine = <SynthEngine>::try_new(&config, SAMPLE_RATE)
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

    let mut engine = <SynthEngine>::try_new(&config, SAMPLE_RATE)
        .expect("extra spectral sources collapse to one");

    let cfg = engine.get_config();
    let spectrum_links: Vec<_> = cfg
        .links
        .iter()
        .filter(|link| link.dst_id() == OSC1_ID && link.dst_input() == Input::Spectrum)
        .collect();

    assert_eq!(spectrum_links.len(), 1);
    assert_eq!(spectrum_links[0].src_id(), HE1_ID);

    engine.handle_note_on(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: None,
        },
        0,
    );
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

    let engine = <SynthEngine>::try_new(&config, SAMPLE_RATE)
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

    engine.handle_note_on(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: None,
        },
        0,
    );
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
    assert_eq!(cfg.modules.len(), 3);
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
        crate::synth_engine::AVAILABLE_VOICES
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
fn process_order_rejects_cycles() {
    let links = vec![
        ModuleLink::direct(1, InputId::new(Input::Audio, 2)),
        ModuleLink::direct(2, InputId::new(Input::Audio, 1)),
    ];

    assert!(routing_helpers::process_order(&links, []).is_err());
}

#[test]
fn process_order_places_output_last() {
    let links = vec![
        ModuleLink::direct(
            HARMONIC_EDITOR_ID,
            InputId::new(Input::Spectrum, OSCILLATOR_ID),
        ),
        ModuleLink::direct(OSCILLATOR_ID, InputId::new(Input::Audio, OUTPUT_MODULE_ID)),
    ];

    let order = routing_helpers::process_order(&links, []).expect("valid graph");
    assert_eq!(*order.last().unwrap(), OUTPUT_MODULE_ID);
    assert_eq!(order.len(), 3);
}

#[test]
fn process_order_includes_unlinked_modules() {
    let links = vec![ModuleLink::direct(
        OSCILLATOR_ID,
        InputId::new(Input::Audio, OUTPUT_MODULE_ID),
    )];

    let order = routing_helpers::process_order(&links, [LFO_ID, OSCILLATOR_ID, OUTPUT_MODULE_ID])
        .expect("valid graph");

    assert!(order.contains(&LFO_ID));
    assert!(order.contains(&OSCILLATOR_ID));
    assert_eq!(*order.last().unwrap(), OUTPUT_MODULE_ID);
    assert!(
        order.iter().position(|&id| id == OSCILLATOR_ID).unwrap()
            < order.iter().position(|&id| id == OUTPUT_MODULE_ID).unwrap()
    );
}

#[test]
fn add_module_joins_process_order_before_output() {
    let mut engine = make_engine(
        EngineParams::default(),
        OscillatorConfig {
            id: OSCILLATOR_ID,
            ..OscillatorConfig::default()
        },
    );

    let lfo_id = engine.add_module(ModuleType::Lfo);
    assert!(engine.process_order.contains(&lfo_id));
    assert_eq!(*engine.process_order.last().unwrap(), OUTPUT_MODULE_ID);
}

fn oscillator_config(engine: &SynthEngine, id: ModuleId) -> OscillatorConfig {
    match engine.get_module(id) {
        Some(ModuleHandle::Oscillator(osc)) => osc.get_config(),
        _ => panic!("expected oscillator {id}"),
    }
}

#[test]
fn duplicate_module_copies_settings_without_links() {
    let mut engine = make_engine(
        EngineParams::default(),
        OscillatorConfig {
            id: OSCILLATOR_ID,
            unison_voices: 3,
            steal_phase: true,
            phase_random: 0.25,
            mono_spectrum: true,
            detune: 0.4.into(),
            ..OscillatorConfig::default()
        },
    );

    let copy_id = engine
        .duplicate_module(OSCILLATOR_ID)
        .expect("oscillator duplicates");

    assert_ne!(copy_id, OSCILLATOR_ID);
    assert!(engine.duplicate_module(OUTPUT_MODULE_ID).is_none());
    assert!(engine.duplicate_module(99).is_none());
    assert!(engine.process_order.contains(&copy_id));
    assert_eq!(*engine.process_order.last().unwrap(), OUTPUT_MODULE_ID);

    let original = oscillator_config(&engine, OSCILLATOR_ID);
    let copy = oscillator_config(&engine, copy_id);

    assert_eq!(copy.id, copy_id);
    assert_eq!(original.unison_voices, copy.unison_voices);
    assert_eq!(original.steal_phase, copy.steal_phase);
    assert_eq!(original.phase_random, copy.phase_random);
    assert_eq!(original.mono_spectrum, copy.mono_spectrum);
    assert_eq!(original.detune, copy.detune);
    assert_eq!(original.detune_power, copy.detune_power);
    assert_eq!(original.phase_shift, copy.phase_shift);
    assert_eq!(original.frequency_shift, copy.frequency_shift);
    assert_eq!(original.phases_blend, copy.phases_blend);
    assert_eq!(original.gains_blend, copy.gains_blend);

    for (left, right) in original.unison.iter().zip(copy.unison.iter()) {
        assert_eq!(left.initial_phase, right.initial_phase);
        assert_eq!(left.phase_shift, right.phase_shift);
        assert_eq!(left.phase_shift_to, right.phase_shift_to);
        assert_eq!(left.gain, right.gain);
        assert_eq!(left.gain_to, right.gain_to);
    }

    assert!(engine.get_config().links.iter().all(|link| {
        link.src_id() != copy_id && link.dst_id() != copy_id && link.modulator_id() != Some(copy_id)
    }));
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

    let amp_id = engine.add_module(ModuleType::Amplifier);
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

    engine.handle_note_on(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: None,
        },
        0,
    );

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

    let lfo_id = engine.add_module(ModuleType::Lfo);
    let dst = InputId::new(Input::Detune, OSCILLATOR_ID);

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
    assert_eq!(link_amount(links[0]), StereoSample::splat(0.25));
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

    let harmonic_b = engine.add_module(ModuleType::HarmonicEditor);
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

    let lfo_id = engine.add_module(ModuleType::Lfo);
    let dst = InputId::new(Input::Detune, OSCILLATOR_ID);
    engine
        .add_mixed_link(lfo_id, dst, StereoSample::ONE)
        .expect("detune modulation link");
    engine.update_link_amount(&lfo_id, &dst, StereoSample::splat(0.5));

    let cfg = engine.get_config();
    let link = cfg
        .links
        .iter()
        .find(|link| link.src_id() == lfo_id && link.dst_id() == OSCILLATOR_ID)
        .expect("lfo -> osc detune link");

    assert_eq!(link_amount(link), StereoSample::splat(0.5));
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
    let lfo_id = engine.add_module(ModuleType::Lfo);

    let err = engine
        .set_direct_link(lfo_id, InputId::new(Input::Detune, OSCILLATOR_ID))
        .expect_err("detune is a mixed input");

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
    let lfo_id = engine.add_module(ModuleType::Lfo);

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
    let env_id = engine.add_module(ModuleType::Envelope);

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
    let lfo_id = engine.add_module(ModuleType::Lfo);
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
    let amp_a = engine.add_module(ModuleType::Amplifier);
    let amp_b = engine.add_module(ModuleType::Amplifier);

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

    engine.handle_note_on(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: None,
        },
        0,
    );

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

    engine.handle_note_on(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: None,
        },
        0,
    );
    process_block(&mut engine, 64);

    engine.handle_note_off(
        Note {
            channel: 0,
            note: 60,
            velocity: 0.0,
            host_id: None,
        },
        0,
    );
    let (left, _) = process_block(&mut engine, 64);
    assert!(left.iter().all(|s| s.is_finite()));

    engine.handle_note_on(
        Note {
            channel: 0,
            note: 64,
            velocity: 1.0,
            host_id: None,
        },
        0,
    );
    let (left, _) = process_block(&mut engine, 64);
    assert!(rms(&left) > 1e-6);
}

#[test]
fn process_reports_terminated_notes() {
    let mut engine = make_engine(
        EngineParams::default(),
        OscillatorConfig {
            id: OSCILLATOR_ID,
            ..OscillatorConfig::default()
        },
    );

    engine.handle_note_on(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: Some(9),
        },
        0,
    );
    process_block(&mut engine, 64);

    engine.handle_note_off(
        Note {
            channel: 0,
            note: 60,
            velocity: 0.0,
            host_id: Some(9),
        },
        0,
    );

    let mut left = vec![0.0; 64];
    let mut right = vec![0.0; 64];
    let mut terminated = Vec::new();
    engine.process(64, false, &mut terminated, [&mut left[..], &mut right[..]]);

    assert_eq!(terminated.len(), 1);
    assert_eq!(terminated[0].channel, 0);
    assert_eq!(terminated[0].note, 60);
    assert_eq!(terminated[0].host_id, Some(9));
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

    engine.handle_note_on(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: None,
        },
        0,
    );
    engine.handle_note_on(
        Note {
            channel: 0,
            note: 64,
            velocity: 1.0,
            host_id: None,
        },
        0,
    );
    engine.handle_note_on(
        Note {
            channel: 0,
            note: 67,
            velocity: 1.0,
            host_id: None,
        },
        0,
    );

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
    let rebuilt =
        <SynthEngine>::try_new(&cfg, SAMPLE_RATE).expect("full patch config deserializes");

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
    engine.set_output_gain(StereoSample::splat(0.5));

    let lfo_id = engine.add_module(ModuleType::Lfo);
    let dst = InputId::new(Input::Detune, OSCILLATOR_ID);
    engine
        .add_mixed_link(lfo_id, dst, StereoSample::ONE)
        .expect("detune link");
    engine.update_link_amount(&lfo_id, &dst, StereoSample::splat(0.25));

    let cfg = engine.get_config();
    assert_eq!(cfg.engine.num_voices, 3);
    assert!(cfg.engine.legato);
    assert_eq!(cfg.engine.block_size, 32);
    assert_eq!(engine.get_voice_kill_time(), kill_time);
    assert!(cfg.engine.oversampling);
    assert_eq!(cfg.engine.output_gain, StereoSample::splat(0.5));

    let link = cfg
        .links
        .iter()
        .find(|link| link.src_id() == lfo_id && link.dst_id() == OSCILLATOR_ID)
        .expect("lfo -> osc detune link");
    assert_eq!(link_amount(link), StereoSample::splat(0.25));
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

    engine.handle_note_on(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: None,
        },
        0,
    );
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

    let amp_id = engine.add_module(ModuleType::Amplifier);
    let env_id = engine.add_module(ModuleType::Envelope);
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

    engine.handle_note_on(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: None,
        },
        0,
    );
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
        Input::Detune,
        StereoSample::splat(0.5),
    ));

    let modulated = config
        .links
        .iter()
        .position(|link| link.src_id() == ENVELOPE_AMP_ID && link.dst_id() == AMPLIFIER_ID)
        .expect("env -> amp link");
    set_link_modulator(&mut config.links[modulated], Some(LFO_ID));

    config.links.push(config.links[modulated].clone());

    let mut engine = <SynthEngine>::try_new(&config, SAMPLE_RATE).expect("modulated preset");

    let cfg = engine.get_config();
    let env_amp_links: Vec<_> = cfg
        .links
        .iter()
        .filter(|link| link.src_id() == ENVELOPE_AMP_ID && link.dst_id() == AMPLIFIER_ID)
        .collect();
    assert_eq!(env_amp_links.len(), 1);
    assert_eq!(env_amp_links[0].modulator_id(), Some(LFO_ID));

    let order = routing_helpers::process_order(
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

    engine.handle_note_on(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: None,
        },
        0,
    );
    let (left, _) = process_block(&mut engine, 64);
    assert!(rms(&left) > 1e-6);
}

#[test]
fn set_config_links_dedupes_duplicate_preset_links() {
    let mut config = full_patch_engine_config(EngineParams::default());
    let osc_out = link(OSC0_ID, MIXER_ID, Input::AudioMix(0));
    config.links.push(osc_out.clone());
    config.links.push(osc_out);

    let engine =
        <SynthEngine>::try_new(&config, SAMPLE_RATE).expect("duplicate links should be skipped");

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
fn fixed_pitch_prewires_unconnected_pitch_inputs() {
    let mut engine = make_engine(
        EngineParams::default(),
        OscillatorConfig {
            id: OSCILLATOR_ID,
            ..OscillatorConfig::default()
        },
    );

    assert!(matches!(
        engine.get_module(PITCH_MODULE_ID),
        Some(ModuleHandle::Pitch(_))
    ));
    assert!(engine.duplicate_module(PITCH_MODULE_ID).is_none());
    engine.remove_module(PITCH_MODULE_ID);
    assert!(engine.get_module(PITCH_MODULE_ID).is_some());

    let hidden: Vec<_> = engine
        .get_links()
        .into_iter()
        .filter(|link| link.is_hidden())
        .collect();

    assert!(hidden.iter().all(|link| link.is_direct()));
    assert!(hidden.iter().all(|link| link.src() == PITCH_MODULE_ID));
    assert!(
        hidden
            .iter()
            .any(|link| link.dst() == InputId::new(Input::Pitch, OSCILLATOR_ID))
    );
    assert!(
        hidden
            .iter()
            .any(|link| link.dst() == InputId::new(Input::Pitch, HARMONIC_EDITOR_ID))
    );
    assert!(!ModuleLink::mixed(1, InputId::new(Input::Gain, 2), StereoSample::ONE).is_hidden());

    let cfg = engine.get_config();
    assert_eq!(cfg.modules.len(), 3);
    assert!(
        cfg.links
            .iter()
            .all(|link| link.src_id() != PITCH_MODULE_ID)
    );

    let state = engine.get_routing_state();
    assert!(state.modules.contains_key(&PITCH_MODULE_ID));
    assert!(
        !state
            .routing
            .contains_key(&InputId::new(Input::Pitch, OSCILLATOR_ID))
    );
    let osc_io = state
        .modules_io
        .as_ref()
        .expect("modules io")
        .get(&OSCILLATOR_ID)
        .expect("oscillator io");
    assert!(
        osc_io
            .inputs
            .iter()
            .all(|input| input.meta.input_type != Input::Pitch)
    );

    assert_before(&engine.process_order, PITCH_MODULE_ID, OSCILLATOR_ID);
    assert_before(&engine.process_order, PITCH_MODULE_ID, HARMONIC_EDITOR_ID);
    assert_eq!(*engine.process_order.last().unwrap(), OUTPUT_MODULE_ID);
}

#[test]
fn explicit_pitch_link_replaces_hidden_prewire_until_removed() {
    let mut engine = make_engine(
        EngineParams::default(),
        OscillatorConfig {
            id: OSCILLATOR_ID,
            ..OscillatorConfig::default()
        },
    );
    let user_pitch = engine.add_module(ModuleType::Pitch);
    let osc_pitch = InputId::new(Input::Pitch, OSCILLATOR_ID);

    engine
        .set_direct_link(user_pitch, osc_pitch)
        .expect("user pitch -> oscillator");

    let osc_links: Vec<_> = engine
        .get_links()
        .into_iter()
        .filter(|link| link.dst() == osc_pitch)
        .collect();
    assert_eq!(osc_links.len(), 1);
    assert!(!osc_links[0].is_hidden());
    assert_eq!(osc_links[0].src(), user_pitch);

    engine
        .refresh_routing()
        .expect("refresh keeps the explicit pitch link");
    assert!(
        engine.get_links().iter().any(|link| {
            link.src() == user_pitch && link.dst() == osc_pitch && !link.is_hidden()
        })
    );
    assert!(engine.get_links().iter().any(|link| {
        link.is_hidden() && link.dst() == InputId::new(Input::Pitch, HARMONIC_EDITOR_ID)
    }));

    engine.remove_link(&user_pitch, &osc_pitch);

    assert!(engine.get_links().iter().any(|link| {
        link.is_hidden() && link.src() == PITCH_MODULE_ID && link.dst() == osc_pitch
    }));
    assert!(!engine.get_routing_state().routing.contains_key(&osc_pitch));
}

#[test]
fn refresh_routing_prewires_pitch_on_new_modules() {
    let mut engine = make_engine(
        EngineParams::default(),
        OscillatorConfig {
            id: OSCILLATOR_ID,
            ..OscillatorConfig::default()
        },
    );
    let svf_id = engine.add_module(ModuleType::Svf);

    engine
        .refresh_routing()
        .expect("refresh keeps default pitch links");

    assert!(engine.get_links().iter().any(|link| {
        link.is_hidden()
            && link.src() == PITCH_MODULE_ID
            && link.dst() == InputId::new(Input::Pitch, svf_id)
    }));
    assert!(engine.get_links().iter().any(|link| {
        link.is_hidden() && link.dst() == InputId::new(Input::Pitch, OSCILLATOR_ID)
    }));
    assert_before(&engine.process_order, PITCH_MODULE_ID, svf_id);
}

#[test]
fn hand_linked_fixed_pitch_stays_visible() {
    let mut engine = make_engine(
        EngineParams::default(),
        OscillatorConfig {
            id: OSCILLATOR_ID,
            ..OscillatorConfig::default()
        },
    );
    let osc_pitch = InputId::new(Input::Pitch, OSCILLATOR_ID);

    engine
        .set_direct_link(PITCH_MODULE_ID, osc_pitch)
        .expect("fixed pitch -> oscillator");
    engine
        .refresh_routing()
        .expect("refresh keeps the hand-made pitch link");

    let osc_links: Vec<_> = engine
        .get_links()
        .into_iter()
        .filter(|link| link.dst() == osc_pitch)
        .collect();
    assert_eq!(osc_links.len(), 1);
    assert!(!osc_links[0].is_hidden());
    assert_eq!(osc_links[0].src(), PITCH_MODULE_ID);

    let state = engine.get_routing_state();
    assert!(state.modules.contains_key(&PITCH_MODULE_ID));
    assert!(matches!(
        state.routing.get(&osc_pitch),
        Some(InputSource::Direct {
            module_id: PITCH_MODULE_ID,
            hidden: false
        })
    ));
    assert!(
        !state
            .routing
            .contains_key(&InputId::new(Input::Pitch, HARMONIC_EDITOR_ID))
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

    let osc_b = engine.add_module(ModuleType::Oscillator);
    let harmonic_b = engine.add_module(ModuleType::HarmonicEditor);
    let mixer_id = engine.add_module(ModuleType::Mixer);
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

    engine.handle_note_on(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: None,
        },
        0,
    );
    engine.handle_note_on(
        Note {
            channel: 0,
            note: 64,
            velocity: 1.0,
            host_id: None,
        },
        0,
    );

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

    engine.handle_note_on(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: None,
        },
        0,
    );
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
        engine.add_module(ModuleType::HarmonicEditor),
        engine.add_module(ModuleType::Oscillator),
        engine.add_module(ModuleType::Envelope),
        engine.add_module(ModuleType::Lfo),
        engine.add_module(ModuleType::Amplifier),
        engine.add_module(ModuleType::Mixer),
        engine.add_module(ModuleType::WaveShaper),
        engine.add_module(ModuleType::SpectralFilter),
        engine.add_module(ModuleType::SpectralBlend),
        engine.add_module(ModuleType::SpectralBandSelect),
        engine.add_module(ModuleType::SpectralMixer),
        engine.add_module(ModuleType::Expressions),
        engine.add_module(ModuleType::ExternalParam),
        engine.add_module(ModuleType::Svf),
    ];

    assert_eq!(ids.len(), 14);
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
        Some(ModuleHandle::SpectralBandSelect(_))
    ));
    assert!(matches!(
        engine.get_module(ids[10]),
        Some(ModuleHandle::SpectralMixer(_))
    ));
    assert!(matches!(
        engine.get_module(ids[11]),
        Some(ModuleHandle::Expressions(_))
    ));
    assert!(matches!(
        engine.get_module(ids[12]),
        Some(ModuleHandle::ExternalParam(_))
    ));
    assert!(matches!(
        engine.get_module(ids[13]),
        Some(ModuleHandle::Svf(_))
    ));
}

// ---- SVF ----

const SVF_ID: ModuleId = 3;

fn svf_engine(svf: SvfConfig) -> SynthEngine {
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
            ModuleConfig::Svf(Box::new(SvfConfig { id: SVF_ID, ..svf })),
        ],
        links: vec![
            LinkConfig::direct(HARMONIC_EDITOR_ID, OSCILLATOR_ID, Input::Spectrum),
            LinkConfig::direct(OSCILLATOR_ID, SVF_ID, Input::Audio),
            LinkConfig::direct(SVF_ID, OUTPUT_MODULE_ID, Input::Audio),
        ],
    };

    <SynthEngine>::try_new(&config, SAMPLE_RATE).expect("valid svf patch")
}

fn svf_output_rms(svf: SvfConfig) -> Sample {
    let mut engine = svf_engine(svf);

    engine.handle_note_on(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: None,
        },
        0,
    );

    // Let the filter settle, then measure.
    for _ in 0..8 {
        process_block(&mut engine, 128);
    }

    let (left, right) = process_block(&mut engine, 128);

    assert!(left.iter().chain(right.iter()).all(|s| s.is_finite()));

    rms(&left)
}

#[test]
fn svf_patch_round_trips_config() {
    let engine = svf_engine(SvfConfig {
        filter_type: filters::svf::SvfType::BandPass12,
        cutoff: 2.0.into(),
        ..SvfConfig::default()
    });
    let cfg = engine.get_config();

    assert_eq!(cfg.modules.len(), 4);
    assert!(matches!(
        engine.get_module(SVF_ID),
        Some(ModuleHandle::Svf(_))
    ));

    let svf_cfg = cfg
        .modules
        .iter()
        .find_map(|m| match m {
            ModuleConfig::Svf(cfg) => Some(cfg),
            _ => None,
        })
        .expect("svf config present");

    assert_eq!(svf_cfg.filter_type, filters::svf::SvfType::BandPass12);
    assert_eq!(svf_cfg.cutoff, StereoSample::splat(2.0));
}

#[test]
fn svf_lowpass_attenuates_more_as_cutoff_drops() {
    let open = svf_output_rms(SvfConfig {
        cutoff: MAX_CUTOFF.into(),
        ..SvfConfig::default()
    });
    let mid = svf_output_rms(SvfConfig {
        cutoff: 1.0.into(),
        ..SvfConfig::default()
    });
    let closed = svf_output_rms(SvfConfig {
        cutoff: MIN_CUTOFF.into(),
        ..SvfConfig::default()
    });

    assert!(open > 1e-4, "{open}");
    assert!(mid < open, "mid {mid} vs open {open}");
    assert!(closed < mid, "closed {closed} vs mid {mid}");
    assert!(closed < open * 0.05, "closed {closed} vs open {open}");
}

#[test]
fn svf_highpass_above_fundamental_removes_most_signal() {
    let open = svf_output_rms(SvfConfig {
        filter_type: filters::svf::SvfType::HighPass12,
        cutoff: MIN_CUTOFF.into(),
        ..SvfConfig::default()
    });
    let closed = svf_output_rms(SvfConfig {
        filter_type: filters::svf::SvfType::HighPass24,
        cutoff: MAX_CUTOFF.into(),
        ..SvfConfig::default()
    });

    assert!(open > 1e-4, "{open}");
    assert!(closed < open * 0.05, "closed {closed} vs open {open}");
}

#[test]
fn svf_all_modes_produce_finite_audio() {
    for filter_type in filters::svf::SvfType::ALL {
        let level = svf_output_rms(SvfConfig {
            filter_type,
            cutoff: 0.0.into(),
            resonance: 0.9.into(),
            drive: 12.0.into(),
            ..SvfConfig::default()
        });

        assert!(level.is_finite(), "{filter_type:?}");
    }
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

    let dst = InputId::new(Input::Detune, OSCILLATOR_ID);
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

    engine.handle_note_on(
        Note {
            channel: 0,
            note: 60,
            velocity: 0.5,
            host_id: None,
        },
        0,
    );
    engine.handle_note_expression(
        Note {
            channel: 0,
            note: 60,
            velocity: 0.0,
            host_id: None,
        },
        Expression::Velocity,
        0,
        1.0,
    );
    process_block(&mut engine, 64);

    engine.handle_choke(Note {
        channel: 0,
        note: 60,
        velocity: 0.0,
        host_id: None,
    });
    let (left, _) = process_block(&mut engine, 64);
    assert!(left.iter().all(|s| s.is_finite()));
}

#[test]
fn oversampling_process() {
    let mut engine = make_engine(
        EngineParams::default(),
        OscillatorConfig {
            id: OSCILLATOR_ID,
            ..OscillatorConfig::default()
        },
    );

    engine.set_oversampling(true);
    engine.handle_note_on(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: None,
        },
        0,
    );

    let (left, right) = process_block(&mut engine, 64);
    assert!(rms(&left) > 1e-6);
    assert!(left.iter().chain(right.iter()).all(|s| s.is_finite()));
}

#[test]
fn process_order_accounts_for_link_modulation() {
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

    let order = routing_helpers::process_order(&links, []).expect("valid order");
    let lfo_pos = order.iter().position(|&id| id == LFO_ID).unwrap();
    let amp_pos = order.iter().position(|&id| id == AMPLIFIER_ID).unwrap();
    assert!(lfo_pos < amp_pos);
}

fn assert_before(order: &[ModuleId], earlier: ModuleId, later: ModuleId) {
    let earlier_pos = order
        .iter()
        .position(|&id| id == earlier)
        .unwrap_or_else(|| panic!("{earlier} missing from {order:?}"));
    let later_pos = order
        .iter()
        .position(|&id| id == later)
        .unwrap_or_else(|| panic!("{later} missing from {order:?}"));

    assert!(
        earlier_pos < later_pos,
        "{earlier} should run before {later} in {order:?}"
    );
}

fn assert_unique(order: &[ModuleId]) {
    let mut ids = order.to_vec();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), order.len(), "duplicate modules in {order:?}");
}

#[test]
fn process_order_is_a_chain() {
    let links = vec![
        ModuleLink::direct(1, InputId::new(Input::Audio, 2)),
        ModuleLink::direct(2, InputId::new(Input::Audio, 3)),
        ModuleLink::direct(3, InputId::new(Input::Audio, OUTPUT_MODULE_ID)),
    ];

    let order = routing_helpers::process_order(&links, []).expect("valid chain");

    assert_eq!(order, vec![1, 2, 3, OUTPUT_MODULE_ID]);
}

#[test]
fn process_order_runs_every_source_before_the_destination() {
    let links = vec![
        ModuleLink::direct(OSC0_ID, InputId::new(Input::AudioMix(0), MIXER_ID)),
        ModuleLink::direct(OSC1_ID, InputId::new(Input::AudioMix(1), MIXER_ID)),
        ModuleLink::direct(MIXER_ID, InputId::new(Input::Audio, OUTPUT_MODULE_ID)),
    ];

    let order = routing_helpers::process_order(&links, []).expect("valid fan-in");

    assert_before(&order, OSC0_ID, MIXER_ID);
    assert_before(&order, OSC1_ID, MIXER_ID);
    assert_before(&order, MIXER_ID, OUTPUT_MODULE_ID);
    assert_eq!(*order.last().unwrap(), OUTPUT_MODULE_ID);
    assert_unique(&order);
}

#[test]
fn process_order_runs_modulator_before_destination() {
    let links = vec![
        ModuleLink::mixed_modulated(
            ENVELOPE_AMP_ID,
            InputId::new(Input::Gain, AMPLIFIER_ID),
            StereoSample::new(0.25, 0.5),
            Some(LFO_ID),
        ),
        ModuleLink::direct(AMPLIFIER_ID, InputId::new(Input::Audio, OUTPUT_MODULE_ID)),
    ];

    let order = routing_helpers::process_order(&links, [EXPRESSIONS_ID]).expect("valid order");

    assert_before(&order, ENVELOPE_AMP_ID, AMPLIFIER_ID);
    assert_before(&order, LFO_ID, AMPLIFIER_ID);
    assert_before(&order, AMPLIFIER_ID, OUTPUT_MODULE_ID);
    assert!(order.contains(&EXPRESSIONS_ID));
    assert_eq!(*order.last().unwrap(), OUTPUT_MODULE_ID);
    assert_unique(&order);
}

#[test]
fn process_order_lists_each_module_once() {
    let links = vec![
        ModuleLink::direct(1, InputId::new(Input::Audio, 2)),
        ModuleLink::direct(1, InputId::new(Input::Gain, 2)),
        ModuleLink::mixed(1, InputId::new(Input::Level, 2), StereoSample::ONE),
    ];

    let order = routing_helpers::process_order(&links, [1, 2]).expect("valid order");

    assert_eq!(order, vec![1, 2]);
}

#[test]
fn process_order_rejects_self_cycle() {
    let links = vec![ModuleLink::direct(1, InputId::new(Input::Audio, 1))];

    let err = routing_helpers::process_order(&links, []).unwrap_err();

    assert_eq!(err, "Cycles detected!");
}

#[test]
fn process_order_rejects_modulation_cycle() {
    let links = vec![ModuleLink::mixed_modulated(
        ENVELOPE_AMP_ID,
        InputId::new(Input::Gain, AMPLIFIER_ID),
        StereoSample::ONE,
        Some(AMPLIFIER_ID),
    )];

    let err = routing_helpers::process_order(&links, [LFO_ID]).unwrap_err();

    assert_eq!(err, "Cycles detected!");
}

#[test]
fn process_order_rejects_longer_cycle() {
    let links = vec![
        ModuleLink::direct(1, InputId::new(Input::Audio, 2)),
        ModuleLink::direct(2, InputId::new(Input::Audio, 3)),
        ModuleLink::direct(3, InputId::new(Input::Audio, 1)),
    ];

    let err = routing_helpers::process_order(&links, [4]).unwrap_err();

    assert_eq!(err, "Cycles detected!");
}

fn module_outputs(specs: &[(ModuleId, DataType, usize)]) -> FxHashMap<ModuleId, (DataType, usize)> {
    specs
        .iter()
        .map(|&(id, data_type, slot)| (id, (data_type, slot)))
        .collect()
}

fn mapped_slots(
    specs: &[(ModuleId, DataType, usize)],
    sources: RoutingMap,
) -> FxHashMap<ModuleId, Vec<routing::InputSlots>> {
    routing_helpers::assign_slots(&module_outputs(specs), &sources)
}

#[test]
fn map_slots_leaves_unlinked_modules_empty() {
    let mapped = mapped_slots(
        &[(1, DataType::Audio, 3), (2, DataType::Spectral, 3)],
        RoutingMap::default(),
    );

    assert!(mapped[&1].is_empty());
    assert!(mapped[&2].is_empty());
}

#[test]
fn map_slots_assigns_spectral_and_direct_variants() {
    let mut sources = RoutingMap::default();
    sources.insert(
        InputId::new(Input::Spectrum, OSCILLATOR_ID),
        InputSource::direct(HARMONIC_EDITOR_ID),
    );
    sources.insert(
        InputId::new(Input::Pitch, OSCILLATOR_ID),
        InputSource::direct(LFO_ID),
    );
    sources.insert(
        InputId::new(Input::Audio, OUTPUT_MODULE_ID),
        InputSource::direct(OSCILLATOR_ID),
    );

    // Spectral and audio arenas both use slot 0 for their first module.
    let mapped = mapped_slots(
        &[
            (HARMONIC_EDITOR_ID, DataType::Spectral, 0),
            (OSCILLATOR_ID, DataType::Audio, 0),
            (LFO_ID, DataType::Control, 4),
            (OUTPUT_MODULE_ID, DataType::Audio, usize::MAX),
        ],
        sources,
    );

    let osc = &mapped[&OSCILLATOR_ID];
    assert!(osc.iter().any(|input| matches!(
        input,
        routing::InputSlots::Spectral {
            input_type: Input::Spectrum,
            slot: 0
        }
    )));
    assert!(osc.iter().any(|input| matches!(
        input,
        routing::InputSlots::Direct {
            input_type: Input::Pitch,
            slot: 4
        }
    )));

    let output = &mapped[&OUTPUT_MODULE_ID];
    assert!(output.iter().any(|input| matches!(
        input,
        routing::InputSlots::Direct {
            input_type: Input::Audio,
            slot: 0
        }
    )));
    assert!(
        output
            .iter()
            .all(|input| !matches!(input, routing::InputSlots::Spectral { .. }))
    );

    assert!(mapped[&HARMONIC_EDITOR_ID].is_empty());
    assert!(mapped[&LFO_ID].is_empty());
}

#[test]
fn map_slots_keeps_mixed_amounts_and_modulation_slots() {
    let amount_env = StereoSample::new(0.25, 0.75);
    let amount_expr = StereoSample::splat(0.5);
    let mut sources = RoutingMap::default();
    sources.insert(
        InputId::new(Input::Detune, OSCILLATOR_ID),
        InputSource::Mixed(vec![
            routing::MixedSource {
                module_id: ENVELOPE_AMP_ID,
                amount: amount_env,
                modulation: Some(LFO_ID),
            },
            routing::MixedSource {
                module_id: EXPRESSIONS_ID,
                amount: amount_expr,
                modulation: None,
            },
        ]),
    );

    let mapped = mapped_slots(
        &[
            (ENVELOPE_AMP_ID, DataType::Control, 2),
            (LFO_ID, DataType::Control, 3),
            (EXPRESSIONS_ID, DataType::Control, 7),
            (OSCILLATOR_ID, DataType::Audio, 0),
        ],
        sources,
    );

    let detune = mapped[&OSCILLATOR_ID]
        .iter()
        .find_map(|input| match input {
            routing::InputSlots::Mixed(mixed) if mixed.input_type == Input::Detune => Some(mixed),
            _ => None,
        })
        .expect("detune slots");

    assert_eq!(detune.slots.len(), 2);
    assert_eq!(detune.slots[0].src_slot, 2);
    assert_eq!(detune.slots[0].modulation_slot, Some(3));
    assert_eq!(detune.slots[0].amount, amount_env);
    assert_eq!(detune.slots[1].src_slot, 7);
    assert_eq!(detune.slots[1].modulation_slot, None);
    assert_eq!(detune.slots[1].amount, amount_expr);
    assert!(
        mapped[&OSCILLATOR_ID]
            .iter()
            .all(|input| !matches!(input, routing::InputSlots::Spectral { .. }))
    );
    assert!(mapped[&LFO_ID].is_empty());
}

const SLOT_HE_UNUSED: ModuleId = 21;
const SLOT_HE: ModuleId = 22;
const SLOT_OSC: ModuleId = 23;
const SLOT_PITCH: ModuleId = 24;
const SLOT_ENV: ModuleId = 25;
const SLOT_LFO: ModuleId = 26;
const SLOT_EXPR: ModuleId = 27;
const SLOT_AMP: ModuleId = 28;

fn slot_mapping_engine() -> SynthEngine {
    let config = EngineConfig {
        engine: EngineParams::default(),
        modules: vec![
            ModuleConfig::HarmonicEditor(Box::new(HarmonicEditorConfig {
                id: SLOT_HE_UNUSED,
                ..HarmonicEditorConfig::default()
            })),
            ModuleConfig::HarmonicEditor(Box::new(HarmonicEditorConfig {
                id: SLOT_HE,
                ..HarmonicEditorConfig::default()
            })),
            ModuleConfig::Oscillator(Box::new(OscillatorConfig {
                id: SLOT_OSC,
                ..OscillatorConfig::default()
            })),
            ModuleConfig::Pitch(Box::new(PitchConfig {
                id: SLOT_PITCH,
                ..PitchConfig::default()
            })),
            ModuleConfig::Envelope(Box::new(EnvelopeConfig {
                id: SLOT_ENV,
                ..EnvelopeConfig::default()
            })),
            ModuleConfig::Lfo(Box::new(LfoConfig {
                id: SLOT_LFO,
                ..LfoConfig::default()
            })),
            ModuleConfig::Expressions(Box::new(ExpressionsConfig {
                id: SLOT_EXPR,
                ..ExpressionsConfig::default()
            })),
            ModuleConfig::Amplifier(Box::new(AmplifierConfig {
                id: SLOT_AMP,
                ..AmplifierConfig::default()
            })),
        ],
        links: vec![
            LinkConfig::direct(SLOT_HE, SLOT_OSC, Input::Spectrum),
            LinkConfig::direct(SLOT_PITCH, SLOT_OSC, Input::Pitch),
            LinkConfig::mixed_modulated(
                SLOT_ENV,
                SLOT_OSC,
                Input::Detune,
                StereoSample::new(0.25, 0.75),
                Some(SLOT_LFO),
            ),
            LinkConfig::mixed(SLOT_EXPR, SLOT_OSC, Input::Detune, StereoSample::splat(0.5)),
            LinkConfig::direct(SLOT_OSC, SLOT_AMP, Input::Audio),
            LinkConfig::direct(SLOT_AMP, OUTPUT_MODULE_ID, Input::Audio),
        ],
    };

    <SynthEngine>::try_new(&config, SAMPLE_RATE).expect("slot mapping patch")
}

fn output_slot_of(engine: &SynthEngine, id: ModuleId) -> usize {
    engine.get_module(id).expect("module").output_slot()
}

fn oscillator_of(engine: &SynthEngine) -> &oscillator::Oscillator {
    match engine.get_module(SLOT_OSC) {
        Some(ModuleHandle::Oscillator(osc)) => osc,
        _ => panic!("expected oscillator"),
    }
}

fn amplifier_audio_slot(engine: &SynthEngine) -> Option<usize> {
    match engine.get_module(SLOT_AMP) {
        Some(ModuleHandle::Amplifier(amp)) => amp.audio_slot(),
        _ => panic!("expected amplifier"),
    }
}

fn output_audio_slot(engine: &SynthEngine) -> Option<usize> {
    match engine.get_module(OUTPUT_MODULE_ID) {
        Some(ModuleHandle::Output(output)) => output.audio_input_slot(),
        _ => panic!("expected output"),
    }
}

#[test]
fn engine_orders_sources_before_destinations() {
    let engine = slot_mapping_engine();
    let order = &engine.process_order;

    assert_before(order, SLOT_HE, SLOT_OSC);
    assert_before(order, SLOT_PITCH, SLOT_OSC);
    assert_before(order, SLOT_ENV, SLOT_OSC);
    assert_before(order, SLOT_LFO, SLOT_OSC);
    assert_before(order, SLOT_EXPR, SLOT_OSC);
    assert_before(order, SLOT_OSC, SLOT_AMP);
    assert_before(order, SLOT_AMP, OUTPUT_MODULE_ID);
    assert!(order.contains(&SLOT_HE_UNUSED));
    assert_eq!(*order.last().unwrap(), OUTPUT_MODULE_ID);
    assert_unique(order);
}

#[test]
fn engine_maps_output_slots_onto_inputs() {
    let engine = slot_mapping_engine();
    let osc = oscillator_of(&engine);

    assert_ne!(
        output_slot_of(&engine, SLOT_HE),
        output_slot_of(&engine, SLOT_HE_UNUSED)
    );
    assert_eq!(osc.spectrum_slot(), Some(output_slot_of(&engine, SLOT_HE)));
    assert_eq!(osc.pitch_slot(), Some(output_slot_of(&engine, SLOT_PITCH)));

    let detune = osc.detune_slots();
    assert_eq!(detune.len(), 2);
    assert_eq!(detune[0].src_slot, output_slot_of(&engine, SLOT_ENV));
    assert_eq!(
        detune[0].modulation_slot,
        Some(output_slot_of(&engine, SLOT_LFO))
    );
    assert_eq!(detune[0].amount, StereoSample::new(0.25, 0.75));
    assert_eq!(detune[1].src_slot, output_slot_of(&engine, SLOT_EXPR));
    assert_eq!(detune[1].modulation_slot, None);
    assert_eq!(detune[1].amount, StereoSample::splat(0.5));

    assert_eq!(
        amplifier_audio_slot(&engine),
        Some(output_slot_of(&engine, SLOT_OSC))
    );
    assert_eq!(
        output_audio_slot(&engine),
        Some(output_slot_of(&engine, SLOT_AMP))
    );
    assert_ne!(
        output_slot_of(&engine, SLOT_OSC),
        output_slot_of(&engine, SLOT_AMP)
    );
}

#[test]
fn replacing_direct_link_remaps_the_slot() {
    let mut engine = slot_mapping_engine();

    engine
        .set_direct_link(SLOT_HE_UNUSED, InputId::new(Input::Spectrum, SLOT_OSC))
        .expect("replace spectrum source");

    assert_eq!(
        oscillator_of(&engine).spectrum_slot(),
        Some(output_slot_of(&engine, SLOT_HE_UNUSED))
    );
    assert_before(&engine.process_order, SLOT_HE_UNUSED, SLOT_OSC);
}

#[test]
fn removing_link_clears_mapped_slot() {
    let mut engine = slot_mapping_engine();

    engine.remove_link(&SLOT_HE, &InputId::new(Input::Spectrum, SLOT_OSC));
    assert_eq!(oscillator_of(&engine).spectrum_slot(), None);

    engine.remove_link(&SLOT_AMP, &InputId::new(Input::Audio, OUTPUT_MODULE_ID));
    assert_eq!(output_audio_slot(&engine), None);
    assert_eq!(*engine.process_order.last().unwrap(), OUTPUT_MODULE_ID);
}

#[test]
fn update_link_amount_updates_mapped_slot() {
    let mut engine = slot_mapping_engine();
    let amount = StereoSample::new(0.1, 0.2);

    engine.update_link_amount(&SLOT_ENV, &InputId::new(Input::Detune, SLOT_OSC), amount);

    let detune = oscillator_of(&engine).detune_slots();
    assert_eq!(detune[0].amount, amount);
    assert_eq!(detune[0].src_slot, output_slot_of(&engine, SLOT_ENV));
    assert_eq!(
        detune[0].modulation_slot,
        Some(output_slot_of(&engine, SLOT_LFO))
    );
    assert_eq!(detune[1].amount, StereoSample::splat(0.5));
}

#[test]
fn removing_modulator_clears_modulation_slot() {
    let mut engine = slot_mapping_engine();

    engine.remove_link_modulation(SLOT_ENV, &InputId::new(Input::Detune, SLOT_OSC));

    let detune = oscillator_of(&engine).detune_slots();
    assert_eq!(detune[0].modulation_slot, None);
    assert_eq!(detune[0].src_slot, output_slot_of(&engine, SLOT_ENV));
    assert_eq!(detune[0].amount, StereoSample::new(0.25, 0.75));
    assert!(engine.process_order.contains(&SLOT_LFO));
    assert_before(&engine.process_order, SLOT_ENV, SLOT_OSC);
}

#[test]
fn reused_output_slot_is_remapped() {
    let mut engine = slot_mapping_engine();
    let freed = output_slot_of(&engine, SLOT_ENV);

    engine.remove_module(SLOT_ENV);

    let new_id = engine.add_module(ModuleType::Envelope);
    assert_eq!(output_slot_of(&engine, new_id), freed);
    assert!(!engine.process_order.contains(&SLOT_ENV));
    assert_eq!(*engine.process_order.last().unwrap(), OUTPUT_MODULE_ID);

    engine
        .add_mixed_link(
            new_id,
            InputId::new(Input::Detune, SLOT_OSC),
            StereoSample::splat(0.3),
        )
        .expect("relink reused slot");

    let detune = oscillator_of(&engine).detune_slots();
    let mapped = detune
        .iter()
        .find(|slot| slot.src_slot == freed)
        .expect("reused slot mapped onto detune");
    assert_eq!(mapped.amount, StereoSample::splat(0.3));
    assert_eq!(mapped.modulation_slot, None);
    assert_before(&engine.process_order, new_id, SLOT_OSC);
    assert_eq!(*engine.process_order.last().unwrap(), OUTPUT_MODULE_ID);
}
