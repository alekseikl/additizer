use addi_dsp::segments::time_segments::{TimePoint, TimeSegments};

use super::*;
use crate::synth_engine::{StereoSample, synth_module::SynthModule, voices_handler::DecayingVoice};

/// Curve from `(seconds, level)` pairs. The first time is 0.
fn curve(points: &[(Smp, Smp)]) -> TimeSegments {
    let mut segments = TimeSegments::default();

    while segments.num_points() > 2 {
        assert!(segments.remove_point(1));
    }

    let end = points.len() - 1;

    segments.set_time(1, points[end].0);

    let start = segments.point(0);
    segments.set_point(
        0,
        TimePoint {
            y: points[0].1,
            ..start
        },
    );

    let last = segments.point(1);
    segments.set_point(
        1,
        TimePoint {
            y: points[end].1,
            ..last
        },
    );

    for (offset, &(time, y)) in points[1..end].iter().enumerate() {
        assert!(segments.add_point(
            offset + 1,
            time,
            TimePoint {
                y,
                ..TimePoint::default()
            },
        ));
    }

    segments
}

fn envelope(steal_level: bool, keep_voice_alive: bool) -> MultiEnvelope {
    MultiEnvelope::from_config(&MultiEnvelopeConfig {
        id: 1,
        steal_level,
        keep_voice_alive,
        ..MultiEnvelopeConfig::default()
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

fn render(
    env: &mut MultiEnvelope,
    out: &mut [Smp],
    durations: &[Smp],
    level_offsets: &[Smp],
    sample_rate: Smp,
    release_at: Option<usize>,
) {
    let len = out.len();
    let mut pos = 0;
    let mut release_at = release_at.map(|at| at.min(len));
    let seconds_per_sample = sample_rate.recip();
    let len_state = durations.len().max(level_offsets.len());

    env.modulated.fill(PointParams::default());

    for index in 0..len_state {
        env.modulated[index] = PointParams {
            duration: durations.get(index).copied().unwrap_or(0.0),
            level: level_offsets.get(index).copied().unwrap_or(0.0),
        };
    }

    let ctx = Ctx {
        segments: &env.params.segments,
        modulated: &env.modulated,
    };
    let voice = env.voices.at_mut(0, 0);

    while pos < len {
        let end = release_at.unwrap_or(len).max(pos);

        MultiEnvelope::<stub::Links>::render_region(
            &ctx,
            voice,
            &mut out[pos..end],
            seconds_per_sample,
            sample_rate,
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
        MultiEnvelope::<stub::Links>::begin_release(&ctx, voice, level);
    }

    voice.last_level = out.last().copied().unwrap_or_default();
}

#[test]
fn default_exposes_point_offsets() {
    let env = envelope(false, false);

    assert_eq!(
        env.inputs()
            .iter()
            .map(|input| input.input_type)
            .collect::<Vec<_>>(),
        vec![
            Input::DurationMix(0),
            Input::LinLevelMix(0),
            Input::DurationMix(1),
            Input::LinLevelMix(1),
            Input::DurationMix(2),
            Input::LinLevelMix(2),
            Input::LinLevelMix(3),
        ]
    );
    assert!(env.inputs().iter().all(|input| !input.is_direct));
}

#[test]
fn offset_inputs_follow_the_point_count() {
    let mut env = envelope(false, false);
    env.set_segments(curve(&[(0.0, 1.0), (0.25, 0.5), (0.8, 0.0)]));

    assert_eq!(
        env.inputs()
            .iter()
            .map(|input| input.input_type)
            .collect::<Vec<_>>(),
        vec![
            Input::DurationMix(0),
            Input::LinLevelMix(0),
            Input::DurationMix(1),
            Input::LinLevelMix(1),
            Input::LinLevelMix(2)
        ]
    );
}

#[test]
fn a_one_second_fall_lasts_one_second() {
    let mut env = envelope(false, false);
    env.set_segments(curve(&[(0.0, 1.0), (1.0, 0.0)]));
    let mut out = [0.0; 12];

    *env.voices.at_mut(0, 0) = playing(0, 0.0, 1.0, 0.0);
    render(&mut env, &mut out, &[1.0], &[], 10.0, None);

    for (index, sample) in out[..10].iter().enumerate() {
        let expected = 1.0 - index as Smp * 0.1;

        assert!((sample - expected).abs() < 1e-5, "{sample} vs {expected}");
    }

    assert_eq!(&out[10..], &[0.0, 0.0]);
    assert_eq!(env.voices.at(0, 0).stage, Stage::Done);
}

#[test]
fn a_longer_duration_stretches_the_same_shape() {
    let mut env = envelope(false, false);
    env.set_segments(curve(&[(0.0, 1.0), (1.0, 0.0)]));
    let mut out = [0.0; 4];

    *env.voices.at_mut(0, 0) = playing(0, 0.0, 1.0, 0.0);
    render(&mut env, &mut out, &[2.0], &[], 2.0, None);

    for (index, sample) in out.iter().enumerate() {
        let expected = 1.0 - index as Smp * 0.25;

        assert!((sample - expected).abs() < 1e-5, "{sample} vs {expected}");
    }
}

#[test]
fn release_holds_then_falls_from_the_current_level() {
    let mut env = envelope(false, true);
    let mut segments = curve(&[(0.0, 1.0), (0.5, 0.4), (1.0, 0.0)]);
    assert!(segments.set_release_point(Some(1)));
    env.set_segments(segments);

    let mut out = [0.0; 8];

    *env.voices.at_mut(0, 0) = playing(0, 0.0, 1.0, 1.0);
    render(&mut env, &mut out, &[0.5, 0.5], &[], 4.0, None);

    assert_eq!(env.voices.at(0, 0).stage, Stage::Sustain);
    assert!(
        out[2..].iter().all(|sample| (*sample - 0.4).abs() < 1e-4),
        "{out:?}"
    );

    let mut released = [0.0; 4];

    env.voices.at_mut(0, 0).release = Release::Pending(0);
    render(&mut env, &mut released, &[0.5, 0.5], &[], 4.0, Some(0));

    assert!(released[0] <= 0.4 + 1e-4);
    assert!(released[3] < released[0]);
    assert_eq!(env.voices.at(0, 0).stage, Stage::Done);
}

#[test]
fn loop_returns_to_the_loop_point() {
    let mut env = envelope(false, false);
    let mut segments = curve(&[(0.0, 0.0), (0.5, 1.0), (1.0, 0.0)]);
    assert!(segments.set_loop_point(Some(0)));
    env.set_segments(segments);

    let mut out = [0.0; 4];

    *env.voices.at_mut(0, 0) = playing(1, 0.0, 1.0, 1.0);
    render(&mut env, &mut out, &[0.5, 0.5], &[], 8.0, None);

    assert_eq!(
        env.voices.at(0, 0).stage,
        Stage::Segment {
            index: 0,
            position: 0.0,
            start: None,
        }
    );
}

#[test]
fn keep_alive_holds_a_playing_release() {
    let mut env = envelope(false, true);
    let mut segments = curve(&[(0.0, 1.0), (0.5, 1.0), (1.0, 0.0)]);
    assert!(segments.set_release_point(Some(1)));
    env.set_segments(segments);
    env.voices.at_mut(0, 2).stage = Stage::segment(0, 0.2);

    let mut playing = [DecayingVoice::new(2)];
    env.poll_decaying_voices(&mut playing);
    assert!(!playing[0].is_done());

    env.voices.at_mut(0, 2).stage = Stage::Done;
    let mut finished = [DecayingVoice::new(2)];
    env.poll_decaying_voices(&mut finished);
    assert!(finished[0].is_done());
}

#[test]
fn config_roundtrip() {
    let mut segments = curve(&[(0.0, 0.2), (0.2, 0.6), (0.4, 1.0)]);
    assert!(segments.set_release_point(Some(1)));

    let mut source = MultiEnvelopeConfig {
        id: 7,
        steal_level: true,
        keep_voice_alive: true,
        segments,
        ..MultiEnvelopeConfig::default()
    };
    source.set_duration_offset(0, StereoSample::new(0.25, -0.5));
    source.set_level_offset(1, StereoSample::splat(0.1));

    let env: MultiEnvelope = MultiEnvelope::from_config(&source);
    let config = env.get_config();

    assert_eq!(config.id, 7);
    assert!(config.steal_level);
    assert!(config.keep_voice_alive);
    assert_eq!(config.segments.release_point_idx(), Some(1));
    assert!((config.segments.point(0).y - 0.2).abs() < 1e-6);
    assert_eq!(config.offsets[0].duration, StereoSample::new(0.25, -0.5));
    assert_eq!(config.offsets[1].level, StereoSample::splat(0.1));

    let json = serde_json::to_string(&MultiEnvelopeConfig::default()).unwrap();
    let restored: MultiEnvelopeConfig = serde_json::from_str(&json).unwrap();
    assert!((restored.segments.total_time() - 1.2).abs() < 1e-6);

    let legacy: MultiEnvelopeConfig = serde_json::from_str(r#"{"id":1}"#).unwrap();
    assert!(legacy.offsets.is_empty());
    assert_eq!(legacy.duration_offset(0), StereoSample::ZERO);
    assert_eq!(legacy.level_offset(0), StereoSample::ZERO);
}

#[test]
fn level_offset_lifts_the_segment() {
    let mut env = envelope(false, false);
    env.set_segments(curve(&[(0.0, 1.0), (1.0, 0.0)]));
    let mut out = [0.0; 2];

    *env.voices.at_mut(0, 0) = Voice {
        stage: Stage::segment(0, 0.0),
        ..Voice::default()
    };
    render(&mut env, &mut out, &[1.0], &[0.25, 0.25], 2.0, None);

    assert!((out[0] - 1.0).abs() < 1e-5, "{out:?}");
    assert!((out[1] - 0.625).abs() < 1e-5, "{out:?}");
}

#[test]
fn level_offset_lowers_the_segment() {
    let mut env = envelope(false, false);
    env.set_segments(curve(&[(0.0, 1.0), (1.0, 0.0)]));
    let mut out = [0.0; 2];

    *env.voices.at_mut(0, 0) = Voice {
        stage: Stage::segment(0, 0.0),
        ..Voice::default()
    };
    render(&mut env, &mut out, &[1.0], &[-0.25], 2.0, None);

    assert!((out[0] - 0.75).abs() < 1e-5, "{out:?}");
    assert!((out[1] - 0.375).abs() < 1e-5, "{out:?}");
}

#[test]
fn level_offset_stays_inside_zero_and_one() {
    let mut env = envelope(false, false);
    env.set_segments(curve(&[(0.0, 1.0), (1.0, 0.0)]));
    let mut above = [0.0; 2];
    let mut below = [0.0; 2];

    *env.voices.at_mut(0, 0) = Voice {
        stage: Stage::segment(0, 0.0),
        ..Voice::default()
    };
    render(&mut env, &mut above, &[1.0], &[1.0, 1.0], 2.0, None);

    *env.voices.at_mut(0, 0) = Voice {
        stage: Stage::segment(0, 0.0),
        ..Voice::default()
    };
    render(&mut env, &mut below, &[1.0], &[-1.0, -1.0], 2.0, None);

    assert!(above.iter().all(|sample| *sample == 1.0), "{above:?}");
    assert!(below.iter().all(|sample| *sample == 0.0), "{below:?}");
}

#[test]
fn level_offset_keeps_a_stolen_start() {
    let mut env = envelope(false, false);
    let mut out = [0.0; 1];

    *env.voices.at_mut(0, 0) = playing(0, 0.0, 0.4, 0.4);
    render(&mut env, &mut out, &[1.0], &[0.2], 10.0, None);

    assert!((out[0] - 0.4).abs() < 1e-5, "{out:?}");
}

#[test]
fn last_point_duration_is_not_an_input() {
    let mut env = envelope(false, false);

    env.set_segments(curve(&[(0.0, 1.0), (0.25, 0.5), (0.8, 0.0)]));
    env.set_duration_offset(1, StereoSample::splat(0.5));
    env.set_segments(curve(&[(0.0, 1.0), (1.0, 0.0)]));

    assert_eq!(
        env.get_config().duration_offset(1),
        StereoSample::splat(0.5)
    );
    assert!(
        env.inputs()
            .iter()
            .all(|input| input.input_type != Input::DurationMix(1))
    );
}

#[test]
fn duration_offset_is_clamped_to_the_slider_range() {
    let mut env = envelope(false, false);

    env.set_duration_offset(0, StereoSample::splat(-100.0));

    assert_eq!(
        env.get_config().offsets[0].duration,
        StereoSample::splat(-MAX_DURATION_OFFSET)
    );
}
