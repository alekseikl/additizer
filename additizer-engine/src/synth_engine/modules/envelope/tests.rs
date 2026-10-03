use super::*;
use crate::synth_engine::routing::{NUM_CHANNELS, VoiceEvent};

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
    env.voices[0][0].next_frame_value = 0.6;
    env.voices[1][0].next_frame_value = 0.4;

    env.process_events(&[reset_event(1, Some(0))]);

    assert!((env.voices[0][1].start_level - 0.6).abs() < 1e-6);
    assert!((env.voices[1][1].start_level - 0.4).abs() < 1e-6);
}

#[test]
fn reset_does_not_steal_without_replaced_voice() {
    let mut env = envelope(true);
    env.voices[0][2].next_frame_value = 0.75;

    env.process_events(&[reset_event(3, None)]);

    assert_eq!(env.voices[0][3].start_level, 0.0);
}

#[test]
fn reset_does_not_steal_when_disabled() {
    let mut env = envelope(false);
    env.voices[0][0].next_frame_value = 0.6;

    env.process_events(&[reset_event(1, Some(0))]);

    assert_eq!(env.voices[0][1].start_level, 0.0);
}

#[test]
fn attack_starts_from_stolen_level() {
    let fill = FillStage {
        t: 0.0,
        release: None,
        start_level: 0.4,
        delay: 0.0,
        attack: 1.0,
        hold: 0.0,
        decay: 0.0,
        sustain: 1.0,
        release_time: 0.0,
        t_step: 0.25,
        attack_curve: Exponential::new(0.0),
        decay_curve: Exponential::new(0.0),
        release_curve: Exponential::new(0.0),
    };
    let mut out = [0.0; 4];
    let (n, _) = fill.fill(&mut out);

    assert_eq!(n, 4);
    assert!((out[0] - 0.4).abs() < 1e-5);
    assert!(out[0] < out[1]);
    assert!(out[3] < 1.0);
}

#[test]
fn delay_holds_stolen_level() {
    let fill = FillStage {
        t: 0.0,
        release: None,
        start_level: 0.55,
        delay: 1.0,
        attack: 1.0,
        hold: 0.0,
        decay: 0.0,
        sustain: 1.0,
        release_time: 0.0,
        t_step: 0.25,
        attack_curve: Exponential::new(0.0),
        decay_curve: Exponential::new(0.0),
        release_curve: Exponential::new(0.0),
    };
    let mut out = [0.0; 4];
    let (n, _) = fill.fill(&mut out);

    assert_eq!(n, 4);
    for sample in out {
        assert!((sample - 0.55).abs() < 1e-6);
    }
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
    for channel_idx in 0..NUM_CHANNELS {
        env.voices[channel_idx][0].next_frame_value = 0.2 + channel_idx as Sample * 0.3;
    }

    env.process_events(&[reset_event(4, Some(0))]);

    for channel_idx in 0..NUM_CHANNELS {
        let expected = 0.2 + channel_idx as Sample * 0.3;
        assert!((env.voices[channel_idx][4].start_level - expected).abs() < 1e-6);
    }
}
