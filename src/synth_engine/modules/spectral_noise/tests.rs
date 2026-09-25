use std::f32::consts::PI;

use super::*;
use crate::{
    synth_engine::{
        ComplexSample, EngineConfig, EngineParams, Input, LinkConfig, ModuleConfig, ModuleId, Note,
        OUTPUT_MODULE_ID, SPECTRAL_BUFFER_SIZE, Sample, SynthEngine, VoiceEvent, buffer::DC_OFFSET,
        oscillator::OscillatorConfig, routing::LEFT_CHANNEL, synth_module::SynthModule,
        voices_handler::BAND_LIMIT_FREQUENCY,
    },
    utils::{C4_PITCH, MIN_LEVEL_DB, db_to_gain, note_to_pitch, pitch_to_freq},
};

const SAMPLE_RATE: Sample = 48_000.0;
const DRAW_LEN: usize = 32;

fn noise_with(color: NoiseColor, level: StereoSample, bandwidth: i32) -> SpectralNoise {
    SpectralNoise::from_config(&SpectralNoiseConfig {
        id: 1,
        color,
        bandwidth,
        level,
        stereo: true,
        ..SpectralNoiseConfig::default()
    })
}

fn draw(
    noise: &mut SpectralNoise,
    voice: usize,
    channel: usize,
    length: usize,
) -> Vec<ComplexSample> {
    let amount = noise.channel_params[channel].amount;
    let level = noise.channel_params[channel].level;
    let mut out = vec![ComplexSample::ZERO; length];

    if channel == LEFT_CHANNEL {
        noise.apply_phase_reset(voice);
    }

    let gain = SpectralNoise::level_gain(level);
    let phase_channel = if noise.stereo { channel } else { LEFT_CHANNEL };
    let cutoff = noise.cutoff[phase_channel];
    let rolloff = noise.rolloff[phase_channel];

    if noise.stereo || channel == LEFT_CHANNEL {
        let phases = &mut noise.phases[phase_channel][voice][DC_OFFSET..length];
        let rng = &mut noise.random;
        let cutoff_log2 = cutoff;
        let cutoff_harmonic = cutoff_log2.exp2();
        let db_per_oct = rolloff.clamp(MIN_ROLLOFF, MAX_ROLLOFF);
        let split = (cutoff_harmonic.ceil() as usize)
            .saturating_sub(DC_OFFSET)
            .min(phases.len());
        let (limited, full) = phases.split_at_mut(split);
        let full_turn = PI * amount;

        for (offset, phase) in limited.iter_mut().enumerate() {
            let harmonic = DC_OFFSET + offset;
            let octaves_below = (cutoff_log2 - noise.log2[harmonic]).max(0.0);
            let scale = db_to_gain(-db_per_oct * octaves_below);

            SpectralNoise::turn_phase(phase, full_turn * scale, rng);
        }

        for phase in full {
            SpectralNoise::turn_phase(phase, full_turn, rng);
        }
    } else {
        let [left, right] = noise.phases.each_mut();
        right[voice][..length].copy_from_slice(&left[voice][..length]);
    }

    noise.write_harmonics(voice, phase_channel, gain, &mut out);
    out
}

fn rolloff_gain(octaves_below: Sample, db_per_oct: Sample) -> Sample {
    let octaves_below = octaves_below.max(0.0);
    let db_per_oct = db_per_oct.clamp(MIN_ROLLOFF, MAX_ROLLOFF);

    db_to_gain(-db_per_oct * octaves_below)
}

#[test]
fn dc_bin_is_silent() {
    let mut noise = noise_with(NoiseColor::White, StereoSample::ZERO, 0);
    let spectrum = draw(&mut noise, 0, LEFT_CHANNEL, DRAW_LEN);

    assert_eq!(spectrum[0], ComplexSample::ZERO);
}

#[test]
fn magnitudes_follow_noise_color() {
    let mut brown = noise_with(NoiseColor::Brown, StereoSample::ZERO, 0);
    let mut pink = noise_with(NoiseColor::Pink, StereoSample::ZERO, 0);
    let mut white = noise_with(NoiseColor::White, StereoSample::ZERO, 0);
    let brown = draw(&mut brown, 0, LEFT_CHANNEL, DRAW_LEN);
    let pink = draw(&mut pink, 0, LEFT_CHANNEL, DRAW_LEN);
    let white = draw(&mut white, 0, LEFT_CHANNEL, DRAW_LEN);

    let saw = 1.0 / std::f32::consts::PI;
    assert!((brown[1].norm() - saw).abs() < 1e-5);
    assert!((brown[2].norm() / brown[1].norm() - 0.5).abs() < 1e-4);
    assert!((brown[4].norm() / brown[1].norm() - 0.25).abs() < 1e-4);

    assert!((pink[4].norm() / pink[1].norm() - 0.5).abs() < 1e-4);
    assert!((white[4].norm() / white[1].norm() - 1.0).abs() < 1e-4);
    assert!((white[17].norm() / white[1].norm() - 1.0).abs() < 1e-4);
}

#[test]
fn stereo_channels_share_magnitude_and_differ_in_phase() {
    let mut noise = noise_with(NoiseColor::Pink, StereoSample::ZERO, 0);
    let left = draw(&mut noise, 0, LEFT_CHANNEL, DRAW_LEN);
    let right = draw(&mut noise, 0, 1, DRAW_LEN);

    for idx in [1, 2, 5, 16] {
        assert!((left[idx].norm() - right[idx].norm()).abs() < 1e-5);
        assert_ne!(left[idx], right[idx], "phase idx {idx}");
    }
}

#[test]
fn mono_channels_share_harmonics() {
    let mut noise = SpectralNoise::from_config(&SpectralNoiseConfig {
        id: 1,
        color: NoiseColor::White,
        level: StereoSample::new(0.0, -6.0),
        stereo: false,
        ..SpectralNoiseConfig::default()
    });
    let left = draw(&mut noise, 0, LEFT_CHANNEL, DRAW_LEN);
    let right = draw(&mut noise, 0, 1, DRAW_LEN);
    let gain = db_to_gain(-6.0);

    for idx in [1, 3, 8, 20] {
        assert!((right[idx] - left[idx] * gain).norm() < 1e-5, "idx {idx}");
    }
}

#[test]
fn voices_get_independent_harmonics() {
    let mut noise = SpectralNoise::from_config(&SpectralNoiseConfig {
        id: 1,
        stereo: false,
        ..SpectralNoiseConfig::default()
    });
    let first_left = draw(&mut noise, 0, LEFT_CHANNEL, 16);
    let second_left = draw(&mut noise, 1, LEFT_CHANNEL, 16);
    let first_right = draw(&mut noise, 0, 1, 16);
    let second_right = draw(&mut noise, 1, 1, 16);

    assert_ne!(first_left[1], second_left[1]);
    assert_eq!(first_left[1], first_right[1]);
    assert_eq!(second_left[1], second_right[1]);
}

#[test]
fn level_scales_magnitude() {
    let mut unity = noise_with(NoiseColor::Brown, StereoSample::ZERO, 0);
    let mut quieter = noise_with(NoiseColor::Brown, StereoSample::splat(-6.0), 0);
    let unity = draw(&mut unity, 0, LEFT_CHANNEL, 8);
    let quieter = draw(&mut quieter, 0, LEFT_CHANNEL, 8);
    let ratio = quieter[3].norm() / unity[3].norm();

    assert!((ratio - db_to_gain(-6.0)).abs() < 1e-5);
}

#[test]
fn min_level_silences_harmonics() {
    let mut noise = noise_with(NoiseColor::White, StereoSample::splat(MIN_LEVEL_DB), 0);
    let spectrum = draw(&mut noise, 0, LEFT_CHANNEL, 16);

    assert_eq!(spectrum[1].norm(), 0.0);
    assert_eq!(spectrum[8].norm(), 0.0);
}

fn circular_distance(a: Sample, b: Sample) -> Sample {
    let delta = (a - b).abs() % std::f32::consts::TAU;
    delta.min(std::f32::consts::TAU - delta)
}

#[test]
fn zero_amount_holds_phase() {
    let mut noise = SpectralNoise::from_config(&SpectralNoiseConfig {
        id: 1,
        amount: 0.0.into(),
        ..SpectralNoiseConfig::default()
    });
    let before = draw(&mut noise, 0, LEFT_CHANNEL, 8);
    let after = draw(&mut noise, 0, LEFT_CHANNEL, 8);

    assert_eq!(before, after);
}

#[test]
fn amount_scales_the_phase_turn() {
    let amount = 0.5;
    let mut noise = SpectralNoise::from_config(&SpectralNoiseConfig {
        id: 1,
        amount: amount.into(),
        ..SpectralNoiseConfig::default()
    });
    let before = draw(&mut noise, 0, LEFT_CHANNEL, 8);
    let after = draw(&mut noise, 0, LEFT_CHANNEL, 8);
    let limit = PI * amount;

    for (before, after) in before.iter().zip(&after).skip(1) {
        let turn = circular_distance(before.arg(), after.arg());
        assert!(
            turn <= limit + 1e-4,
            "phase jumped by {turn}, limit is {limit}"
        );
    }
}

#[test]
fn amount_is_full_at_and_above_the_limit() {
    assert!((rolloff_gain(0.0, 33.0) - 1.0).abs() < 1e-6);
    assert!((rolloff_gain(-2.0, MAX_ROLLOFF) - 1.0).abs() < 1e-6);
}

#[test]
fn amount_rolls_off_linearly_in_db_per_octave() {
    let one_octave = rolloff_gain(1.0, MIN_ROLLOFF);
    let two_octaves = rolloff_gain(2.0, MIN_ROLLOFF);
    let steep = rolloff_gain(1.0, MAX_ROLLOFF);

    assert!((one_octave - db_to_gain(-6.0)).abs() < 1e-5);
    assert!((two_octaves - db_to_gain(-12.0)).abs() < 1e-5);
    assert!((steep - db_to_gain(-60.0)).abs() < 1e-5);
}

#[test]
fn rolloff_steepens_the_falloff() {
    let gentle = rolloff_gain(2.0, MIN_ROLLOFF);
    let steep = rolloff_gain(2.0, MAX_ROLLOFF);

    assert!(steep < gentle);
}

#[test]
fn cutoff_sticks_to_frequency() {
    let octaves_below = |pitch: f32, harmonic: f32| 2.0 - (pitch - C4_PITCH) - harmonic.log2();
    let at_c4 = rolloff_gain(octaves_below(C4_PITCH, 2.0), MIN_ROLLOFF);
    let octave_up = rolloff_gain(octaves_below(C4_PITCH + 1.0, 1.0), MIN_ROLLOFF);

    assert!((at_c4 - octave_up).abs() < 1e-5);
    assert!(at_c4 < 1.0);
}

#[test]
fn cutoff_reduces_the_turn_below_the_frequency() {
    let amount = 1.0;
    let slope = MIN_ROLLOFF;
    let limit = 2.0;
    let mut noise = SpectralNoise::from_config(&SpectralNoiseConfig {
        id: 1,
        amount: amount.into(),
        cutoff: limit.into(),
        rolloff: slope.into(),
        ..SpectralNoiseConfig::default()
    });
    let before = draw(&mut noise, 0, LEFT_CHANNEL, 16);
    let after = draw(&mut noise, 0, LEFT_CHANNEL, 16);
    let low = circular_distance(before[1].arg(), after[1].arg());
    let low_limit = PI * amount * rolloff_gain(limit, slope);
    let high = circular_distance(before[8].arg(), after[8].arg());

    assert!(low <= low_limit + 1e-4, "low harmonic jumped by {low}");
    assert!(high <= PI * amount + 1e-4, "high harmonic jumped by {high}");
    assert!(low_limit < PI * amount);
}

#[test]
fn each_draw_turns_phase_and_keeps_magnitude() {
    let mut noise = noise_with(NoiseColor::Pink, StereoSample::ZERO, 0);
    let before = draw(&mut noise, 0, LEFT_CHANNEL, 8);
    let after = draw(&mut noise, 0, LEFT_CHANNEL, 8);

    assert_ne!(after[3], before[3]);
    assert!((after[3].norm() - before[3].norm()).abs() < 1e-5);

    for (before, after) in before.iter().zip(&after).skip(1) {
        let turn = circular_distance(before.arg(), after.arg());
        assert!(turn <= PI + 1e-4, "phase jumped by {turn}, limit is {PI}");
    }
}

#[test]
fn note_based_bandwidth_tracks_pitch() {
    let noise = noise_with(NoiseColor::White, StereoSample::ZERO, 0);
    let low = note_to_pitch(36.0);
    let high = note_to_pitch(96.0);

    assert!(noise.spectrum_length(low) > noise.spectrum_length(high));

    let frequency = pitch_to_freq(low).max(1.0);
    let expected =
        ((BAND_LIMIT_FREQUENCY / frequency).floor() as usize + 1).min(SPECTRAL_BUFFER_SIZE);

    assert_eq!(noise.spectrum_length(low), expected);
}

#[test]
fn fixed_bandwidth_ignores_pitch() {
    let noise = noise_with(NoiseColor::White, StereoSample::ZERO, 8);

    assert_eq!(noise.spectrum_length(note_to_pitch(36.0)), 9);
    assert_eq!(noise.spectrum_length(note_to_pitch(96.0)), 9);
}

#[test]
fn config_round_trips() {
    let noise = SpectralNoise::from_config(&SpectralNoiseConfig {
        id: 4,
        color: NoiseColor::Brown,
        bandwidth: 24,
        level: StereoSample::new(-12.0, -3.0),
        stereo: false,
        amount: 0.25.into(),
        cutoff: StereoSample::new(-1.0, 2.0),
        rolloff: StereoSample::new(12.0, 48.0),
        steal_phase: true,
    });
    let restored = SpectralNoise::from_config(&noise.get_config());

    assert_eq!(restored.get_config(), noise.get_config());
    assert!(!restored.get_config().stereo);
    assert_eq!(restored.get_config().amount, StereoSample::splat(0.25));
    assert_eq!(restored.get_config().cutoff, StereoSample::new(-1.0, 2.0));
    assert_eq!(restored.get_config().rolloff, StereoSample::new(12.0, 48.0));
    assert!(restored.get_config().steal_phase);
}

fn play(bandwidth: i32) -> Vec<Sample> {
    const NOISE_ID: ModuleId = 1;
    const OSC_ID: ModuleId = 2;

    let config = EngineConfig {
        engine: EngineParams::default(),
        modules: vec![
            ModuleConfig::SpectralNoise(Box::new(SpectralNoiseConfig {
                id: NOISE_ID,
                color: NoiseColor::White,
                bandwidth,
                ..SpectralNoiseConfig::default()
            })),
            ModuleConfig::Oscillator(Box::new(OscillatorConfig {
                id: OSC_ID,
                ..OscillatorConfig::default()
            })),
        ],
        links: vec![
            LinkConfig::direct(NOISE_ID, OSC_ID, Input::Spectrum),
            LinkConfig::direct(OSC_ID, OUTPUT_MODULE_ID, Input::Audio),
        ],
    };

    let mut engine = SynthEngine::try_new(&config, SAMPLE_RATE).expect("valid engine");

    engine.handle_note_on(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: None,
        },
        0,
    );

    let mut left = vec![0.0; 64];
    let mut right = vec![0.0; 64];
    let mut terminated = Vec::new();

    engine.process(64, false, &mut terminated, [&mut left[..], &mut right[..]]);
    engine.process(64, false, &mut terminated, [&mut left[..], &mut right[..]]);

    left
}

fn rms(samples: &[Sample]) -> Sample {
    (samples.iter().map(|s| s * s).sum::<Sample>() / samples.len() as Sample).sqrt()
}

#[test]
fn bandlimiting_changes_the_rendered_wave() {
    let narrow = play(8);
    let wide = play(64);

    assert!(rms(&wide) > 1e-3);

    let diff = narrow
        .iter()
        .zip(&wide)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0, Sample::max);

    assert!(diff > 0.05, "bandwidth 8 and 64 rendered the same wave");
}

#[test]
fn each_process_draws_a_new_spectrum() {
    const NOISE_ID: ModuleId = 1;
    const OSC_ID: ModuleId = 2;

    let config = EngineConfig {
        engine: EngineParams::default(),
        modules: vec![
            ModuleConfig::SpectralNoise(Box::new(SpectralNoiseConfig {
                id: NOISE_ID,
                color: NoiseColor::White,
                bandwidth: 32,
                ..SpectralNoiseConfig::default()
            })),
            ModuleConfig::Oscillator(Box::new(OscillatorConfig {
                id: OSC_ID,
                ..OscillatorConfig::default()
            })),
        ],
        links: vec![
            LinkConfig::direct(NOISE_ID, OSC_ID, Input::Spectrum),
            LinkConfig::direct(OSC_ID, OUTPUT_MODULE_ID, Input::Audio),
        ],
    };

    let mut engine = SynthEngine::try_new(&config, SAMPLE_RATE).expect("valid engine");

    engine.handle_note_on(
        Note {
            channel: 0,
            note: 60,
            velocity: 1.0,
            host_id: None,
        },
        0,
    );

    let mut first = vec![0.0; 64];
    let mut second = vec![0.0; 64];
    let mut right = vec![0.0; 64];
    let mut terminated = Vec::new();

    engine.process(64, false, &mut terminated, [&mut first[..], &mut right[..]]);
    engine.process(
        64,
        false,
        &mut terminated,
        [&mut second[..], &mut right[..]],
    );

    let diff = first
        .iter()
        .zip(&second)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0, Sample::max);

    assert!(diff > 0.05, "two process calls rendered the same wave");
}

fn held_phases(steal_phase: bool) -> SpectralNoise {
    SpectralNoise::from_config(&SpectralNoiseConfig {
        id: 1,
        amount: 0.0.into(),
        steal_phase,
        ..SpectralNoiseConfig::default()
    })
}

fn reset(voice_idx: usize, replaced_voice_idx: Option<usize>) -> VoiceEvent {
    VoiceEvent::Reset {
        voice_idx,
        replaced_voice_idx,
        prev_note: None,
        pitch: 0.0,
        velocity: 1.0,
        offset: 0,
    }
}

fn phases_differ(noise: &SpectralNoise, voice_a: usize, voice_b: usize) -> bool {
    noise.phases[LEFT_CHANNEL][voice_a]
        .iter()
        .zip(noise.phases[LEFT_CHANNEL][voice_b].iter())
        .skip(1)
        .any(|(a, b)| a != b)
}

#[test]
fn trigger_steals_phases_from_the_replaced_voice() {
    let mut noise = held_phases(true);
    let _ = draw(&mut noise, 0, LEFT_CHANNEL, 8);
    let _ = draw(&mut noise, 1, LEFT_CHANNEL, 8);
    assert!(phases_differ(&noise, 0, 1));

    SynthModule::process_events(&mut noise, &[reset(1, Some(0))]);
    let _ = draw(&mut noise, 1, LEFT_CHANNEL, 8);

    assert_eq!(
        noise.phases[LEFT_CHANNEL][1].as_ref(),
        noise.phases[LEFT_CHANNEL][0].as_ref()
    );
    assert_eq!(noise.phases[1][1].as_ref(), noise.phases[1][0].as_ref());
}

#[test]
fn trigger_randomizes_phases_when_steal_is_off() {
    let mut noise = held_phases(false);
    let _ = draw(&mut noise, 0, LEFT_CHANNEL, 8);
    let _ = draw(&mut noise, 1, LEFT_CHANNEL, 8);
    let before = noise.phases[LEFT_CHANNEL][0].clone();

    SynthModule::process_events(&mut noise, &[reset(0, Some(1))]);
    let _ = draw(&mut noise, 0, LEFT_CHANNEL, 8);

    assert!(
        before
            .iter()
            .zip(noise.phases[LEFT_CHANNEL][0].iter())
            .skip(1)
            .any(|(a, b)| a != b)
    );
    assert!(phases_differ(&noise, 0, 1));
}

#[test]
fn steal_without_a_replaced_voice_randomizes() {
    let mut noise = held_phases(true);
    let _ = draw(&mut noise, 0, LEFT_CHANNEL, 8);
    let before = noise.phases[LEFT_CHANNEL][0].clone();

    SynthModule::process_events(&mut noise, &[reset(0, None)]);
    let _ = draw(&mut noise, 0, LEFT_CHANNEL, 8);

    assert!(
        before
            .iter()
            .zip(noise.phases[LEFT_CHANNEL][0].iter())
            .skip(1)
            .any(|(a, b)| a != b)
    );
}
