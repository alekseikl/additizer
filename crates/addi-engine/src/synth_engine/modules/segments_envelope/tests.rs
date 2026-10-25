use addi_dsp::segments::{Segment, Segments};

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
    SegmentsEnvelope::from_config(&SegmentsEnvelopeConfig {
        id: 1,
        steal_level,
        keep_voice_alive,
        ..SegmentsEnvelopeConfig::default()
    })
}

fn playing(index: usize, position: Smp, start_level: Smp, level: Smp) -> Voice {
    Voice {
        stage: Stage::Segment { index, position },
        start_level,
        level,
    }
}

fn render_stage(
    env: &SegmentsEnvelope,
    voice: &Voice,
    out: &mut [Smp],
    samples: usize,
    phase_step: Smp,
) -> Voice {
    let segments = &env.params.segments;
    let len = samples.min(out.len());
    let spans = segments.num_segments().saturating_sub(1);
    let mut stage = voice.stage;
    let mut pos = 0;

    while let Stage::Segment { index, position } = stage
        && index < spans
        && pos < len
    {
        let mut start = segments.segment(index);
        let end = segments.segment(index + 1);

        if index == 0 {
            start.y = voice.start_level;
        }

        match Segment::render_segment(&mut out[pos..len], position, phase_step, start, end) {
            Ok(position) => {
                stage = Stage::Segment { index, position };
                pos = len;
            }
            Err(written) => {
                pos += written;
                stage = if index + 1 < spans {
                    Stage::Segment {
                        index: index + 1,
                        position: 0.0,
                    }
                } else {
                    Stage::Done
                };
            }
        }
    }

    if pos < len {
        out[pos..len].fill(segments.value_at(1.0));
        stage = Stage::Done;
    }

    Voice {
        stage,
        start_level: voice.start_level,
        level: out[..len].last().copied().unwrap_or_default(),
    }
}

fn play_at_start() -> Stage {
    Stage::Segment {
        index: 0,
        position: 0.0,
    }
}

#[test]
fn reset_starts_at_the_drawn_start() {
    let mut env = envelope(false, false);
    env.set_segments(Segments::from_points(&[(0.0, 0.2), (1.0, 0.8)], false));
    *env.voices.at_mut(0, 0) = playing(0, 0.4, 0.2, 0.9);

    env.process_events(&[reset_event(1, Some(0))]);

    let left = env.voices.at(0, 1);
    let right = env.voices.at(1, 1);

    assert_eq!(left.stage, play_at_start());
    assert_eq!(right.stage, play_at_start());
    assert!((left.start_level - 0.2).abs() < 1e-6);
    assert!((right.start_level - 0.2).abs() < 1e-6);
}

#[test]
fn reset_steals_the_replaced_level() {
    let mut env = envelope(true, false);
    env.set_segments(Segments::from_points(&[(0.0, 0.0), (1.0, 1.0)], false));
    *env.voices.at_mut(0, 0) = playing(0, 0.4, 0.0, 0.6);
    *env.voices.at_mut(1, 0) = Voice {
        stage: Stage::Done,
        start_level: 0.0,
        level: 0.25,
    };

    env.process_events(&[reset_event(1, Some(0))]);

    let left = env.voices.at(0, 1);
    let right = env.voices.at(1, 1);

    assert_eq!(left.stage, play_at_start());
    assert_eq!(right.stage, play_at_start());
    assert!((left.start_level - 0.6).abs() < 1e-6);
    assert!((right.start_level - 0.25).abs() < 1e-6);
}

#[test]
fn reset_does_not_steal_without_a_replaced_voice() {
    let mut env = envelope(true, false);
    env.set_segments(Segments::from_points(&[(0.0, 0.2), (1.0, 1.0)], false));
    *env.voices.at_mut(0, 0) = playing(0, 0.4, 0.2, 0.9);

    env.process_events(&[reset_event(1, None)]);

    let voice = env.voices.at(0, 1);

    assert_eq!(voice.stage, play_at_start());
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

    let voice = {
        let stored = env.voices.at(0, 1);

        Voice {
            stage: stored.stage,
            start_level: stored.start_level,
            level: stored.level,
        }
    };
    let mut out = [0.0; 4];
    let next = render_stage(&env, &voice, &mut out, 4, 0.25);

    assert!((out[0] - 0.5).abs() < 1e-5);
    assert!((out[1] - 0.75).abs() < 1e-5);
    assert!((out[2] - 1.0).abs() < 1e-5);
    assert!((out[3] - 0.625).abs() < 1e-5);
    assert_eq!(next.stage, Stage::Done);
    assert!((next.level - 0.625).abs() < 1e-5);
}

#[test]
fn playback_holds_the_last_level() {
    let mut env = envelope(false, false);
    env.set_segments(Segments::from_points(&[(0.0, 0.0), (1.0, 1.0)], false));
    let mut out = [0.0; 4];
    let next = render_stage(&env, &playing(0, 0.0, 0.0, 0.0), &mut out, 4, 0.5);

    assert_eq!(next.stage, Stage::Done);
    assert!((out[0] - 0.0).abs() < 1e-6);
    assert!((out[1] - 0.5).abs() < 1e-6);
    assert!((out[2] - 1.0).abs() < 1e-6);
    assert!((out[3] - 1.0).abs() < 1e-6);

    let mut held = [0.0; 3];
    assert_eq!(
        render_stage(&env, &Voice::default(), &mut held, 3, 0.5).stage,
        Stage::Done
    );
    assert_eq!(held, [1.0, 1.0, 1.0]);
}

#[test]
fn play_advances_inside_the_segment() {
    let mut env = envelope(false, false);
    env.set_segments(Segments::from_points(&[(0.0, 0.0), (1.0, 1.0)], false));
    let next = render_stage(&env, &playing(0, 0.0, 0.0, 0.0), &mut [0.0; 4], 4, 0.1);

    assert_eq!(
        next.stage,
        Stage::Segment {
            index: 0,
            position: 0.4,
        }
    );
    assert!((next.level - 0.3).abs() < 1e-6);
}

#[test]
fn play_enters_the_next_segment() {
    let mut env = envelope(false, false);
    env.set_segments(Segments::from_points(
        &[(0.0, 0.0), (0.25, 1.0), (1.0, 0.0)],
        false,
    ));
    let mut out = [0.0; 4];
    let next = render_stage(&env, &playing(0, 0.0, 0.0, 0.0), &mut out, 4, 0.1);
    let Stage::Segment { index, position } = next.stage else {
        panic!("still playing");
    };

    assert_eq!(index, 1);
    assert!((position - 0.2 / 0.75).abs() < 1e-5);
    assert!((out[0] - 0.0).abs() < 1e-6);
    assert!((out[1] - 0.4).abs() < 1e-6);
    assert!((out[2] - 1.0).abs() < 1e-5);
    assert!((out[3] - (1.0 - 0.1 / 0.75)).abs() < 1e-5);
}

#[test]
fn a_zero_step_holds_the_current_level() {
    let mut env = envelope(false, false);
    env.set_segments(Segments::from_points(&[(0.0, 0.0), (1.0, 1.0)], false));
    let mut out = [0.0; 3];
    let next = render_stage(&env, &playing(0, 0.25, 0.0, 0.0), &mut out, 3, 0.0);

    assert_eq!(out, [0.25, 0.25, 0.25]);
    assert_eq!(
        next.stage,
        Stage::Segment {
            index: 0,
            position: 0.25,
        }
    );
}

#[test]
fn curved_segment_matches_the_drawn_curve() {
    let mut env = envelope(false, false);
    let mut segments = Segments::from_points(&[(0.0, 0.2), (1.0, 0.85)], true);
    segments.set_power(0, 5.0);
    env.set_segments(segments);

    let step = 1.0 / 16.0;
    let mut out = [0.0; 16];
    let next = render_stage(&env, &playing(0, 0.0, 0.2, 0.0), &mut out, 16, step);
    let segments = &env.params.segments;

    for (i, sample) in out.iter().enumerate() {
        let expected = segments.value_from(i as Smp * step, 0.2);

        assert!(
            (sample - expected).abs() < 2e-5,
            "sample {i}: {sample} vs {expected}"
        );
    }

    assert_eq!(next.stage, Stage::Done);
}

#[test]
fn keep_alive_holds_only_a_playing_stage() {
    let mut env = envelope(false, true);
    env.voices.at_mut(0, 2).stage = Stage::Segment {
        index: 0,
        position: 0.2,
    };
    env.voices.at_mut(1, 2).stage = Stage::Done;

    let mut playing = [DecayingVoice::new(2)];
    env.poll_decaying_voices(&mut playing);
    assert!(!playing[0].is_done());

    env.voices.at_mut(0, 2).stage = Stage::Done;
    let mut finished = [DecayingVoice::new(2)];
    env.poll_decaying_voices(&mut finished);
    assert!(finished[0].is_done());

    let done = envelope(false, true);
    let mut decaying = [DecayingVoice::new(2)];
    done.poll_decaying_voices(&mut decaying);
    assert!(decaying[0].is_done());

    let mut off = envelope(false, false);
    off.voices.at_mut(0, 2).stage = Stage::Segment {
        index: 0,
        position: 0.2,
    };
    let mut released = [DecayingVoice::new(2)];
    off.poll_decaying_voices(&mut released);
    assert!(released[0].is_done());
}

#[test]
fn from_config_sanitizes_params() {
    let env: SegmentsEnvelope = SegmentsEnvelope::from_config(&SegmentsEnvelopeConfig {
        rate: StereoSample::new(1_000.0, Smp::NAN),
        segments: Segments::from_points(&[(0.0, -2.0), (1.0, 3.0)], false),
        ..SegmentsEnvelopeConfig::default()
    });
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
