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
    SegmentsEnvelope::from_config(&SegmentsEnvelopeConfig {
        id: 1,
        steal_level,
        keep_voice_alive,
        ..SegmentsEnvelopeConfig::default()
    })
}

fn playing(index: usize, position: Smp, start: Smp, last_level: Smp) -> Voice {
    Voice {
        stage: Stage::Segment {
            index,
            position,
            start: (index == 0).then_some(start),
        },
        last_level,
        ..Voice::default()
    }
}

fn held(index: usize, position: Smp, start: Smp, last_level: Smp) -> Voice {
    Voice {
        release: Release::None,
        ..playing(index, position, start, last_level)
    }
}

fn process_trigger(env: &mut SegmentsEnvelope, channel: usize, voice: usize, phase: Smp) {
    let segments = env.params.segments.clone();
    let voice = env.voices.at_mut(channel, voice);
    let Stage::Trigger(level) = voice.stage else {
        panic!("expected a pending trigger, got {:?}", voice.stage);
    };

    SegmentsEnvelope::<stub::Links>::apply_trigger(&segments, voice, phase, level);
}

fn render_playback(
    env: &SegmentsEnvelope,
    voice: &Voice,
    out: &mut [Smp],
    samples: usize,
    phase_step: Smp,
    release_at: Option<usize>,
) -> Voice {
    let mut voice = *voice;
    let len = samples.min(out.len());
    let out = &mut out[..len];
    let mut pos = 0;
    let mut release_at = release_at.map(|at| at.min(len));
    let segments = &env.params.segments;

    while pos < len {
        let end = release_at.unwrap_or(len).max(pos);

        SegmentsEnvelope::<stub::Links>::render_region(
            segments,
            &mut voice,
            &mut out[pos..end],
            phase_step,
        );

        pos = end;

        if release_at.take().is_none() {
            break;
        }

        let level = if pos == 0 {
            voice.last_level
        } else {
            out[pos - 1]
        };
        SegmentsEnvelope::<stub::Links>::begin_release(segments, &mut voice, level);
    }

    voice.last_level = out.last().copied().unwrap_or_default();
    voice
}

fn render_stage(
    env: &SegmentsEnvelope,
    voice: &Voice,
    out: &mut [Smp],
    samples: usize,
    phase_step: Smp,
) -> Voice {
    render_playback(env, voice, out, samples, phase_step, None)
}

fn play_at_start() -> Stage {
    Stage::segment(0, 0.0)
}

fn assert_stage(stage: Stage, index: usize, position: Smp) {
    let Stage::Segment {
        index: got_index,
        position: got_position,
        ..
    } = stage
    else {
        panic!("expected a playing segment, got {stage:?}");
    };

    assert_eq!(got_index, index);
    assert!(
        (got_position - position).abs() < 1e-5,
        "position {got_position} vs {position}"
    );
}

#[test]
fn reset_starts_at_the_drawn_start() {
    let mut env = envelope(false, false);
    env.set_segments(Segments::from_points(&[(0.0, 0.2), (1.0, 0.8)], false));
    *env.voices.at_mut(0, 0) = playing(0, 0.4, 0.2, 0.9);

    env.process_events(&[reset_event(1, Some(0))]);

    assert_eq!(env.voices.at(0, 1).stage, Stage::Trigger(None));
    assert_eq!(env.voices.at(1, 1).stage, Stage::Trigger(None));

    process_trigger(&mut env, 0, 1, 0.0);
    process_trigger(&mut env, 1, 1, 0.0);

    assert_eq!(env.voices.at(0, 1).stage, play_at_start());
    assert_eq!(env.voices.at(1, 1).stage, play_at_start());
}

#[test]
fn reset_steals_the_replaced_level() {
    let mut env = envelope(true, false);
    env.set_segments(Segments::from_points(&[(0.0, 0.0), (1.0, 1.0)], false));
    *env.voices.at_mut(0, 0) = playing(0, 0.4, 0.0, 0.6);
    *env.voices.at_mut(1, 0) = Voice {
        stage: Stage::Done,
        last_level: 0.25,
        ..Voice::default()
    };

    env.process_events(&[reset_event(1, Some(0))]);

    assert!(matches!(
        env.voices.at(0, 1).stage,
        Stage::Trigger(Some(level)) if (level - 0.6).abs() < 1e-6
    ));
    assert!(matches!(
        env.voices.at(1, 1).stage,
        Stage::Trigger(Some(level)) if (level - 0.25).abs() < 1e-6
    ));

    process_trigger(&mut env, 0, 1, 0.0);
    process_trigger(&mut env, 1, 1, 0.0);

    assert_stage(env.voices.at(0, 1).stage, 0, 0.0);
    assert_stage(env.voices.at(1, 1).stage, 0, 0.0);
    assert!(matches!(
        env.voices.at(0, 1).stage,
        Stage::Segment { start: Some(level), .. } if (level - 0.6).abs() < 1e-6
    ));
    assert!(matches!(
        env.voices.at(1, 1).stage,
        Stage::Segment { start: Some(level), .. } if (level - 0.25).abs() < 1e-6
    ));
}

#[test]
fn a_stored_trigger_starts_at_the_phase_shift() {
    let mut env = envelope(false, false);
    env.set_segments(Segments::from_points(
        &[(0.0, 0.0), (0.5, 1.0), (1.0, 0.0)],
        false,
    ));
    env.set_phase_shift(StereoSample::new(0.25, 0.75));
    *env.voices.at_mut(0, 0) = playing(1, 0.4, 0.0, 0.6);

    env.process_events(&[reset_event(0, None)]);

    assert_eq!(env.voices.at(0, 0).stage, Stage::Trigger(None));
    assert_eq!(env.voices.at(1, 0).stage, Stage::Trigger(None));

    let left = env.channel_params[0].phase_shift;
    let right = env.channel_params[1].phase_shift;
    process_trigger(&mut env, 0, 0, left);
    process_trigger(&mut env, 1, 0, right);

    assert_stage(env.voices.at(0, 0).stage, 0, 0.5);
    assert_stage(env.voices.at(1, 0).stage, 1, 0.5);
    assert!((env.voices.at(0, 0).last_level - 0.5).abs() < 1e-5);
    assert!((env.voices.at(1, 0).last_level - 0.5).abs() < 1e-5);
}

#[test]
fn phase_shift_selects_the_trigger_stage() {
    let mut env = envelope(false, false);
    env.set_segments(Segments::from_points(
        &[(0.0, 0.0), (0.5, 1.0), (1.0, 0.0)],
        false,
    ));
    let segments = &env.params.segments;
    let stage_at = |phase: Smp| {
        let mut voice = Voice::default();
        SegmentsEnvelope::<stub::Links>::apply_trigger(
            segments,
            &mut voice,
            phase.rem_euclid(1.0),
            None,
        );
        voice.stage
    };

    assert_stage(stage_at(0.25), 0, 0.5);
    assert_stage(stage_at(-0.25), 1, 0.5);
    assert_eq!(stage_at(1.0), play_at_start());
    assert_eq!(stage_at(-1.0), play_at_start());

    let voice = playing(0, 0.5, 0.0, 0.0);
    let mut out = [0.0; 2];
    let next = render_stage(&env, &voice, &mut out, 2, 0.1);

    assert!((out[0] - 0.5).abs() < 1e-5);
    assert!((out[1] - 0.7).abs() < 1e-5);
    assert_stage(next.stage, 0, 0.9);
}

#[test]
fn reset_does_not_steal_without_a_replaced_voice() {
    let mut env = envelope(true, false);
    env.set_segments(Segments::from_points(&[(0.0, 0.2), (1.0, 1.0)], false));
    *env.voices.at_mut(0, 0) = playing(0, 0.4, 0.2, 0.9);

    env.process_events(&[reset_event(1, None)]);

    assert_eq!(env.voices.at(0, 1).stage, Stage::Trigger(None));

    process_trigger(&mut env, 0, 1, 0.0);

    assert_eq!(env.voices.at(0, 1).stage, play_at_start());
}

#[test]
fn stolen_level_is_the_first_sample() {
    let mut env = envelope(true, false);
    env.set_segments(Segments::from_points(
        &[(0.0, 0.0), (0.5, 1.0), (1.0, 0.25)],
        false,
    ));
    env.voices.at_mut(0, 0).last_level = 0.5;

    env.process_events(&[reset_event(1, Some(0))]);
    process_trigger(&mut env, 0, 1, 0.0);

    let voice = *env.voices.at(0, 1);
    let mut out = [0.0; 4];
    let next = render_stage(&env, &voice, &mut out, 4, 0.25);

    assert!((out[0] - 0.5).abs() < 1e-5);
    assert!((out[1] - 0.75).abs() < 1e-5);
    assert!((out[2] - 1.0).abs() < 1e-5);
    assert!((out[3] - 0.625).abs() < 1e-5);
    assert_eq!(next.stage, Stage::Done);
    assert!((next.last_level - 0.625).abs() < 1e-5);
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

    assert_stage(next.stage, 0, 0.4);
    assert!((next.last_level - 0.3).abs() < 1e-6);
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
    let Stage::Segment {
        index, position, ..
    } = next.stage
    else {
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
    assert_stage(next.stage, 0, 0.25);
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
        let expected = segments.value_at(i as Smp * step, Some(0.2), None);

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
    env.voices.at_mut(0, 2).stage = Stage::segment(0, 0.2);
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
    off.voices.at_mut(0, 2).stage = Stage::segment(0, 0.2);
    let mut released = [DecayingVoice::new(2)];
    off.poll_decaying_voices(&mut released);
    assert!(released[0].is_done());
}

#[test]
fn from_config_sanitizes_params() {
    let env: SegmentsEnvelope = SegmentsEnvelope::from_config(&SegmentsEnvelopeConfig {
        rate: StereoSample::new(1_000.0, Smp::NAN),
        phase_shift: StereoSample::new(-4.0, 2.0),
        segments: Segments::from_points(&[(0.0, -2.0), (1.0, 3.0)], false),
        ..SegmentsEnvelopeConfig::default()
    });
    let config = env.get_config();

    assert_eq!(config.rate, StereoSample::new(50.0, 0.0));
    assert_eq!(config.phase_shift, StereoSample::new(0.0, 1.0));
    assert_eq!(config.timebase, Timebase::Seconds);
    assert_eq!(config.segments.point(0).y, 0.0);
    assert_eq!(config.segments.point(1).y, 1.0);
}

#[test]
fn missing_timebase_defaults_to_seconds() {
    let config: SegmentsEnvelopeConfig = serde_json::from_str(r#"{"id":1}"#).unwrap();

    assert_eq!(config.timebase, Timebase::Seconds);
    assert_eq!(config.rate, StereoSample::splat(1.0));
    assert_eq!(config.phase_shift, StereoSample::splat(0.0));
    assert!(!config.keep_voice_alive);
    assert!(!config.steal_level);

    let stolen: SegmentsEnvelopeConfig =
        serde_json::from_str(r#"{"id":1,"steal_phase":true}"#).unwrap();
    assert!(stolen.steal_level);
}

#[test]
fn release_point_sustains_while_the_key_is_held() {
    let mut env = envelope(false, false);
    let mut segments = Segments::from_points(&[(0.0, 0.0), (0.5, 1.0), (1.0, 0.25)], false);
    assert!(segments.set_release_point(Some(1)));
    env.set_segments(segments);

    let mut out = [0.0; 4];
    let next = render_playback(&env, &held(0, 0.0, 0.0, 0.0), &mut out, 4, 0.25, None);

    assert!((out[0] - 0.0).abs() < 1e-5);
    assert!((out[1] - 0.5).abs() < 1e-5);
    assert!((out[2] - 1.0).abs() < 1e-5);
    assert!((out[3] - 1.0).abs() < 1e-5);
    assert_eq!(next.stage, Stage::Sustain);
}

#[test]
fn note_off_jumps_to_the_release_point_at_the_current_level() {
    let mut env = envelope(false, false);
    let mut segments = Segments::from_points(&[(0.0, 0.0), (0.5, 1.0), (1.0, 0.25)], false);
    assert!(segments.set_release_point(Some(1)));
    env.set_segments(segments);

    let mut out = [0.0; 4];
    let next = render_playback(&env, &held(0, 0.0, 0.0, 0.0), &mut out, 4, 0.25, Some(1));

    assert!((out[0] - 0.0).abs() < 1e-5);
    assert!((out[1] - 0.0).abs() < 1e-5);
    assert!((out[2] - 0.125).abs() < 1e-5);
    assert!((out[3] - 0.25).abs() < 1e-5);
    assert_eq!(next.stage, Stage::Done);
    assert!(matches!(next.release, Release::Releasing(_)));
}

#[test]
fn release_from_sustain_leaves_at_the_sustain_level() {
    let mut env = envelope(false, false);
    let mut segments = Segments::from_points(&[(0.0, 0.0), (0.5, 1.0), (1.0, 0.25)], false);
    assert!(segments.set_release_point(Some(1)));
    env.set_segments(segments);

    let sustained = render_playback(&env, &held(0, 0.0, 0.0, 0.0), &mut [0.0; 4], 4, 0.25, None);
    assert_eq!(sustained.stage, Stage::Sustain);

    let mut out = [0.0; 4];
    let next = render_playback(&env, &sustained, &mut out, 4, 0.25, Some(0));

    assert!((out[0] - 1.0).abs() < 1e-5);
    assert!((out[1] - 0.625).abs() < 1e-5);
    assert!((out[2] - 0.25).abs() < 1e-5);
    assert!((out[3] - 0.25).abs() < 1e-5);
    assert_eq!(next.stage, Stage::Done);
}

#[test]
fn a_release_on_the_last_point_holds_the_level_at_note_off() {
    let mut env = envelope(false, false);
    let mut segments = Segments::from_points(&[(0.0, 0.0), (1.0, 1.0)], false);
    assert!(segments.set_release_point(Some(1)));
    env.set_segments(segments);

    let mut out = [0.0; 4];
    let next = render_playback(&env, &held(0, 0.0, 0.0, 0.0), &mut out, 4, 0.5, Some(1));

    assert!((out[0] - 0.0).abs() < 1e-5);
    assert_eq!(out[1..], [0.0, 0.0, 0.0]);
    assert_eq!(next.stage, Stage::Done);
    assert!(matches!(next.release, Release::Releasing(level) if level == 0.0));
}

#[test]
fn loop_point_repeats_to_the_end() {
    let mut env = envelope(false, false);
    let mut segments = Segments::from_points(&[(0.0, 0.0), (0.5, 1.0), (1.0, 0.0)], false);
    assert!(segments.set_loop_point(Some(1)));
    env.set_segments(segments);

    let mut out = [0.0; 17];
    let next = render_playback(&env, &held(0, 0.0, 0.0, 0.0), &mut out, 17, 0.0625, Some(2));

    assert!((out[0] - 0.0).abs() < 1e-5);
    assert!((out[8] - 1.0).abs() < 1e-5);
    assert!((out[16] - 1.0).abs() < 1e-5);
    assert_stage(next.stage, 1, 0.125);
}

#[test]
fn loop_runs_until_the_release_point_while_the_key_is_held() {
    let mut env = envelope(false, false);
    let mut segments =
        Segments::from_points(&[(0.0, 0.0), (0.25, 1.0), (0.75, 0.5), (1.0, 0.0)], false);
    assert!(segments.set_loop_point(Some(1)));
    assert!(segments.set_release_point(Some(2)));
    env.set_segments(segments);

    let mut out = [0.0; 13];
    let next = render_playback(&env, &held(0, 0.0, 0.0, 0.0), &mut out, 13, 0.0625, None);

    assert!((out[0] - 0.0).abs() < 1e-5);
    assert!((out[4] - 1.0).abs() < 1e-5);
    assert!((out[12] - 1.0).abs() < 1e-5);
    assert_stage(next.stage, 1, 0.125);
}

#[test]
fn note_off_leaves_the_loop_through_the_release_point() {
    let mut env = envelope(false, false);
    let mut segments =
        Segments::from_points(&[(0.0, 0.0), (0.25, 1.0), (0.75, 0.5), (1.0, 0.0)], false);
    assert!(segments.set_loop_point(Some(1)));
    assert!(segments.set_release_point(Some(2)));
    env.set_segments(segments);

    let mut out = [0.0; 6];
    let next = render_playback(&env, &held(0, 0.0, 0.0, 0.0), &mut out, 6, 0.25, Some(3));

    assert!((out[0] - 0.0).abs() < 1e-5);
    assert!((out[1] - 1.0).abs() < 1e-5);
    assert!((out[2] - 0.75).abs() < 1e-5);
    assert!((out[3] - 0.75).abs() < 1e-5);
    assert!((out[4] - 0.0).abs() < 1e-5);
    assert!((out[5] - 0.0).abs() < 1e-5);
    assert_eq!(next.stage, Stage::Done);
}

#[test]
fn a_phase_past_the_release_point_sustains_or_loops() {
    let mut env = envelope(false, false);
    let mut segments = Segments::from_points(&[(0.0, 0.0), (0.5, 1.0), (1.0, 0.0)], false);
    assert!(segments.set_release_point(Some(1)));
    env.set_segments(segments.clone());

    let mut voice = Voice::default();
    SegmentsEnvelope::<stub::Links>::apply_trigger(&env.params.segments, &mut voice, 0.8, None);
    assert_eq!(voice.stage, Stage::Sustain);

    assert!(segments.set_loop_point(Some(0)));
    env.set_segments(segments);

    SegmentsEnvelope::<stub::Links>::apply_trigger(&env.params.segments, &mut voice, 0.8, None);
    assert_eq!(voice.stage, Stage::segment(0, 0.0));
}

#[test]
fn a_repeated_opening_uses_the_drawn_level() {
    let mut env = envelope(false, false);
    let mut segments = Segments::from_points(&[(0.0, 0.2), (1.0, 0.8)], false);
    assert!(segments.set_loop_point(Some(0)));
    env.set_segments(segments);

    let mut out = [0.0; 9];
    render_playback(&env, &held(0, 0.0, 0.5, 0.0), &mut out, 9, 0.125, None);

    assert!((out[0] - 0.5).abs() < 1e-5);
    assert!((out[8] - 0.2).abs() < 1e-5);
}

#[test]
fn a_zero_width_loop_does_not_spin() {
    let mut env = envelope(false, false);
    let mut segments = Segments::from_points(&[(0.0, 0.2), (1.0, 0.5), (1.0, 0.8)], false);
    assert!(segments.set_loop_point(Some(1)));
    env.set_segments(segments);

    let mut out = [0.0; 8];
    let next = render_playback(&env, &playing(0, 0.0, 0.2, 0.0), &mut out, 8, 0.25, None);

    assert!(out.iter().all(|sample| sample.is_finite()));
    assert!(out[4..].iter().all(|sample| (*sample - 0.8).abs() < 1e-5));
    assert_eq!(next.stage, Stage::Done);
}

#[test]
fn a_loop_shorter_than_eight_samples_plays_once() {
    let mut env = envelope(false, false);
    let mut segments = Segments::from_points(&[(0.0, 0.0), (0.5, 1.0), (1.0, 0.0)], false);
    assert!(segments.set_loop_point(Some(1)));
    env.set_segments(segments);

    let mut out = [0.0; 8];
    let next = render_playback(&env, &held(0, 0.0, 0.0, 0.0), &mut out, 8, 0.25, None);

    assert!((out[0] - 0.0).abs() < 1e-5);
    assert!((out[2] - 1.0).abs() < 1e-5);
    assert!(out[4..].iter().all(|sample| sample.abs() < 1e-5));
    assert_eq!(next.stage, Stage::Done);
}

#[test]
fn note_off_is_stored_until_the_voice_is_reset() {
    let mut env = envelope(false, false);

    env.process_events(&[VoiceEvent::Release {
        voice_idx: 0,
        velocity: 0.5,
        offset: 3,
    }]);

    assert!(matches!(env.voices.at(0, 0).release, Release::Pending(3)));
    assert!(matches!(env.voices.at(1, 0).release, Release::Pending(3)));

    env.process_events(&[reset_event(0, None)]);

    assert!(matches!(env.voices.at(0, 0).release, Release::None));
    assert!(matches!(env.voices.at(1, 0).release, Release::None));
}
