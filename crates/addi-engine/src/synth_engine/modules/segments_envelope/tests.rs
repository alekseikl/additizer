use addi_dsp::segments::Segments;

use super::*;
use crate::synth_engine::{routing::VoiceEvent, voices_handler::DecayingVoice};

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

fn envelope(steal_level: bool, keep_voice_alive: bool) -> SegmentsEnvelope {
    SegmentsEnvelope::from_config(
        &SegmentsEnvelopeConfig {
            id: 1,
            steal_level,
            keep_voice_alive,
            ..SegmentsEnvelopeConfig::default()
        },
        16,
    )
}

fn playing(phase: Smp, start_level: Smp, level: Smp) -> Voice {
    Voice {
        shot: Shot::Play(phase),
        start_level,
        level,
    }
}

#[test]
fn reset_starts_at_the_drawn_start() {
    let mut env = envelope(false, false);
    env.set_segments(Segments::from_points(&[(0.0, 0.2), (1.0, 0.8)], false));
    *env.voices.at_mut(0, 0) = playing(0.4, 0.2, 0.9);

    env.process_events(&[reset_event(1, Some(0))]);

    let left = env.voices.at(0, 1);
    let right = env.voices.at(1, 1);

    assert_eq!(left.shot, Shot::Play(0.0));
    assert_eq!(right.shot, Shot::Play(0.0));
    assert!((left.start_level - 0.2).abs() < 1e-6);
    assert!((right.start_level - 0.2).abs() < 1e-6);
}

#[test]
fn reset_steals_the_replaced_level() {
    let mut env = envelope(true, false);
    env.set_segments(Segments::from_points(&[(0.0, 0.0), (1.0, 1.0)], false));
    *env.voices.at_mut(0, 0) = playing(0.4, 0.0, 0.6);
    *env.voices.at_mut(1, 0) = Voice {
        shot: Shot::Done,
        start_level: 0.0,
        level: 0.25,
    };

    env.process_events(&[reset_event(1, Some(0))]);

    let left = env.voices.at(0, 1);
    let right = env.voices.at(1, 1);

    assert_eq!(left.shot, Shot::Play(0.0));
    assert_eq!(right.shot, Shot::Play(0.0));
    assert!((left.start_level - 0.6).abs() < 1e-6);
    assert!((right.start_level - 0.25).abs() < 1e-6);
}

#[test]
fn reset_does_not_steal_without_a_replaced_voice() {
    let mut env = envelope(true, false);
    env.set_segments(Segments::from_points(&[(0.0, 0.2), (1.0, 1.0)], false));
    *env.voices.at_mut(0, 0) = playing(0.4, 0.2, 0.9);

    env.process_events(&[reset_event(1, None)]);

    let voice = env.voices.at(0, 1);

    assert_eq!(voice.shot, Shot::Play(0.0));
    assert!((voice.start_level - 0.2).abs() < 1e-6);
}

#[test]
fn stolen_level_is_the_first_sample() {
    let mut env = envelope(true, false);
    env.set_segments(Segments::from_points(
        &[(0.0, 0.0), (0.5, 1.0), (1.0, 0.25)],
        false,
    ));
    env.voices.at_mut(0, 0).level = 0.5;

    env.process_events(&[reset_event(1, Some(0))]);

    env.buffers.rate[..4].fill(0.25);

    let voice = *env.voices.at(0, 1);
    let mut out = [0.0; 4];
    let next = env.render_shot(&voice, &mut out, 4);

    assert!((out[0] - 0.5).abs() < 1e-5);
    assert!((out[1] - 0.75).abs() < 1e-5);
    assert!((out[2] - 1.0).abs() < 1e-5);
    assert!((out[3] - 0.625).abs() < 1e-5);
    assert_eq!(next.shot, Shot::Done);
    assert!((next.level - 0.625).abs() < 1e-5);
}

#[test]
fn playback_holds_the_last_level() {
    let mut env = envelope(false, false);
    env.set_segments(Segments::from_points(&[(0.0, 0.0), (1.0, 1.0)], false));
    env.buffers.rate[..4].fill(0.5);

    let mut out = [0.0; 4];
    let next = env.render_shot(&playing(0.0, 0.0, 0.0), &mut out, 4);

    assert_eq!(next.shot, Shot::Done);
    assert!((out[0] - 0.0).abs() < 1e-6);
    assert!((out[1] - 0.5).abs() < 1e-6);
    assert!((out[2] - 1.0).abs() < 1e-6);
    assert!((out[3] - 1.0).abs() < 1e-6);

    let mut held = [0.0; 3];
    assert_eq!(
        env.render_shot(&Voice::default(), &mut held, 3).shot,
        Shot::Done
    );
    assert_eq!(held, [1.0, 1.0, 1.0]);
}

#[test]
fn keep_alive_holds_only_a_playing_shot() {
    let mut env = envelope(false, true);
    env.voices.at_mut(0, 2).shot = Shot::Play(0.2);
    env.voices.at_mut(1, 2).shot = Shot::Done;

    let mut playing = [DecayingVoice::new(2)];
    env.poll_decaying_voices(&mut playing);
    assert!(!playing[0].is_done());

    env.voices.at_mut(0, 2).shot = Shot::Done;
    let mut finished = [DecayingVoice::new(2)];
    env.poll_decaying_voices(&mut finished);
    assert!(finished[0].is_done());

    let done = envelope(false, true);
    let mut decaying = [DecayingVoice::new(2)];
    done.poll_decaying_voices(&mut decaying);
    assert!(decaying[0].is_done());

    let mut off = envelope(false, false);
    off.voices.at_mut(0, 2).shot = Shot::Play(0.2);
    let mut released = [DecayingVoice::new(2)];
    off.poll_decaying_voices(&mut released);
    assert!(released[0].is_done());
}

#[test]
fn from_config_sanitizes_params() {
    let env: SegmentsEnvelope = SegmentsEnvelope::from_config(
        &SegmentsEnvelopeConfig {
            rate: StereoSample::new(1_000.0, Smp::NAN),
            segments: Segments::from_points(&[(0.0, -2.0), (1.0, 3.0)], false),
            ..SegmentsEnvelopeConfig::default()
        },
        16,
    );
    let config = env.get_config();

    assert_eq!(config.rate, StereoSample::new(100.0, 0.0));
    assert_eq!(config.timebase, Timebase::Seconds);
    assert_eq!(config.segments.segment(0).y, 0.0);
    assert_eq!(config.segments.segment(1).y, 1.0);
}

#[test]
fn missing_timebase_defaults_to_seconds() {
    let config: SegmentsEnvelopeConfig = serde_json::from_str(r#"{"id":1}"#).unwrap();

    assert_eq!(config.timebase, Timebase::Seconds);
    assert_eq!(config.rate, StereoSample::splat(1.0));
    assert!(!config.keep_voice_alive);
    assert!(!config.steal_level);

    let stolen: SegmentsEnvelopeConfig =
        serde_json::from_str(r#"{"id":1,"steal_phase":true}"#).unwrap();
    assert!(stolen.steal_level);
}
