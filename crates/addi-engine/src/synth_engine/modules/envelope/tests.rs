use super::*;
use crate::synth_engine::{
    EngineConfig, EngineParams, Input, LinkConfig, ModuleConfig, ModuleId, Note, OUTPUT_MODULE_ID,
    SynthEngine, amplifier::AmplifierConfig, harmonic_editor::HarmonicEditorConfig,
    oscillator::OscillatorConfig, power_scale, routing::VoiceEvent,
};

fn reset_event(voice_idx: usize, replaced_voice_idx: Option<usize>) -> VoiceEvent {
    VoiceEvent::Reset {
        voice_idx,
        replaced_voice_idx,
        prev_note: None,
        pitch: 0.0,
        velocity: 1.0,
        offset: 0,
    }
}

fn envelope(steal_level: bool) -> Envelope {
    Envelope::from_config(&EnvelopeConfig {
        id: 1,
        steal_level,
        ..EnvelopeConfig::default()
    })
}

#[test]
fn reset_steals_next_frame_value_from_replaced_voice() {
    let mut env = envelope(true);
    env.voices.at_mut(0, 0).next_frame_value = 0.6;
    env.voices.at_mut(1, 0).next_frame_value = 0.4;

    env.process_events(&[reset_event(1, Some(0))]);

    assert!((env.voices.at(0, 1).start_level - 0.6).abs() < 1e-6);
    assert!((env.voices.at(1, 1).start_level - 0.4).abs() < 1e-6);
}

#[test]
fn reset_does_not_steal_without_replaced_voice() {
    let mut env = envelope(true);
    env.voices.at_mut(0, 2).next_frame_value = 0.75;

    env.process_events(&[reset_event(3, None)]);

    assert_eq!(env.voices.at(0, 3).start_level, 0.0);
}

#[test]
fn reset_does_not_steal_when_disabled() {
    let mut env = envelope(false);
    env.voices.at_mut(0, 0).next_frame_value = 0.6;

    env.process_events(&[reset_event(1, Some(0))]);

    assert_eq!(env.voices.at(0, 1).start_level, 0.0);
}

const HE_ID: ModuleId = 1;
const OSC_ID: ModuleId = 2;
const ENV_ID: ModuleId = 3;
const AMP_ID: ModuleId = 4;
const RENDER_RATE: Sample = 48_000.0;

fn render_engine(env: EnvelopeConfig, amp_gain: Sample, link_envelope: bool) -> SynthEngine {
    let mut links = vec![
        LinkConfig::direct(HE_ID, OSC_ID, Input::Spectrum),
        LinkConfig::direct(OSC_ID, AMP_ID, Input::Audio),
        LinkConfig::direct(AMP_ID, OUTPUT_MODULE_ID, Input::Audio),
    ];

    if link_envelope {
        links.push(LinkConfig::mixed(ENV_ID, AMP_ID, Input::Gain, 1.0));
    }

    let config = EngineConfig {
        engine: EngineParams {
            num_voices: 1,
            block_size: 128,
            ..EngineParams::default()
        },
        modules: vec![
            ModuleConfig::HarmonicEditor(Box::new(HarmonicEditorConfig {
                id: HE_ID,
                bandwidth: 1,
                ..HarmonicEditorConfig::default()
            })),
            ModuleConfig::Oscillator(Box::new(OscillatorConfig {
                id: OSC_ID,
                ..OscillatorConfig::default()
            })),
            ModuleConfig::Envelope(Box::new(env)),
            ModuleConfig::Amplifier(Box::new(AmplifierConfig {
                id: AMP_ID,
                gain: amp_gain.into(),
                ..AmplifierConfig::default()
            })),
        ],
        links,
    };

    SynthEngine::try_new(&config, RENDER_RATE).expect("valid engine")
}

fn note(note: u8) -> Note {
    Note {
        channel: 0,
        note,
        velocity: 1.0,
        host_id: None,
    }
}

fn process(engine: &mut SynthEngine, samples: usize) -> Vec<Sample> {
    let mut out = Vec::with_capacity(samples);
    let mut terminated = Vec::new();
    let mut left = vec![0.0; 128];
    let mut right = vec![0.0; 128];
    let mut remaining = samples;

    while remaining > 0 {
        let n = remaining.min(128);
        engine.process(n, false, &mut terminated, [&mut left[..n], &mut right[..n]]);
        out.extend_from_slice(&left[..n]);
        remaining -= n;
    }

    out
}

fn rms(samples: &[Sample]) -> Sample {
    (samples.iter().map(|sample| sample * sample).sum::<Sample>() / samples.len() as Sample).sqrt()
}

#[test]
fn attack_starts_from_stolen_level() {
    let config = EnvelopeConfig {
        id: ENV_ID,
        steal_level: true,
        attack: 1.0.into(),
        attack_slope: 0.0,
        sustain: 1.0.into(),
        ..EnvelopeConfig::default()
    };
    let mut stolen = render_engine(config.clone(), 0.0, true);
    stolen.handle_note_on(note(60), 0);
    process(&mut stolen, RENDER_RATE as usize + 128);
    stolen.handle_note_on(note(64), 0);
    let stolen_attack = process(&mut stolen, 128);

    let mut fresh = render_engine(config, 0.0, true);
    fresh.handle_note_on(note(64), 0);
    let fresh_attack = process(&mut fresh, 128);

    assert!(rms(&stolen_attack) > rms(&fresh_attack) * 10.0);
}

#[test]
fn delay_holds_stolen_level() {
    let config = EnvelopeConfig {
        id: ENV_ID,
        steal_level: true,
        delay: 1.0.into(),
        attack: 0.0.into(),
        sustain: 0.55.into(),
        decay: 0.0.into(),
        ..EnvelopeConfig::default()
    };
    let mut stolen = render_engine(config.clone(), 0.0, true);
    stolen.handle_note_on(note(60), 0);
    process(&mut stolen, RENDER_RATE as usize + 128);
    stolen.handle_note_on(note(64), 0);
    let held = process(&mut stolen, 128);

    let mut fresh = render_engine(config, 0.0, true);
    fresh.handle_note_on(note(64), 0);
    let silent = process(&mut fresh, 128);

    assert!(rms(&held) > 1e-3);
    assert!(rms(&silent) < rms(&held) * 0.05);
}

#[test]
fn steal_level_round_trips_in_config() {
    let env = envelope(true);
    assert!(env.get_config().steal_level);

    let json = serde_json::to_value(EnvelopeConfig::default()).unwrap();
    let mut obj = json.as_object().unwrap().clone();
    obj.remove("steal_level");
    let decoded: EnvelopeConfig = serde_json::from_value(serde_json::Value::Object(obj)).unwrap();
    assert!(!decoded.steal_level);
}

#[test]
fn both_channels_steal_independently() {
    let mut env = envelope(true);
    for (channel_idx, voice) in env.voices.channels_at_mut(0).iter_mut().enumerate() {
        voice.next_frame_value = 0.2 + channel_idx as Sample * 0.3;
    }

    env.process_events(&[reset_event(4, Some(0))]);

    for (channel_idx, voice) in env.voices.channels_at(4).iter().enumerate() {
        let expected = 0.2 + channel_idx as Sample * 0.3;
        assert!((voice.start_level - expected).abs() < 1e-6);
    }
}

#[test]
fn fill_curve_matches_power_scale() {
    let duration = 0.2;
    let warmup = (0.05 * RENDER_RATE) as usize;
    let measured = 127;

    for slope in [0.0, 0.3, 1.0, -1.0] {
        let config = EnvelopeConfig {
            id: ENV_ID,
            attack: duration.into(),
            attack_slope: slope,
            sustain: 1.0.into(),
            decay: 0.0.into(),
            ..EnvelopeConfig::default()
        };
        let mut wet = render_engine(config, 0.0, true);
        let mut dry = render_engine(
            EnvelopeConfig {
                id: ENV_ID,
                ..EnvelopeConfig::default()
            },
            1.0,
            false,
        );

        wet.handle_note_on(note(60), 0);
        dry.handle_note_on(note(60), 0);

        let wet = process(&mut wet, warmup + measured);
        let dry = process(&mut dry, warmup + measured);
        let power = -slope * SLOPE_POWER_SCALE;
        let recip = duration.recip();

        for (i, (wet, dry)) in wet[warmup..].iter().zip(&dry[warmup..]).enumerate() {
            if dry.abs() < 1e-2 {
                continue;
            }

            let local_t = 0.05 + i as Sample / RENDER_RATE;
            let expected = power_scale(local_t * recip, power);
            let rendered = wet / dry;

            assert!(
                (rendered - expected).abs() < 1e-3,
                "slope {slope}: {rendered} vs {expected}"
            );
        }
    }
}
