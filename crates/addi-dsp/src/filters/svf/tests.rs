use std::f32::consts::{PI, TAU};

use crate::{
    ComplexSample, WideSmp,
    consts::C4_PITCH,
    filters::control::MAX_PRE_Q,
    math::{freq_to_c4_pitch, pitch_to_freq},
};

use super::*;

const SAMPLE_RATE: Smp = 48_000.0;
const CUTOFF: Smp = 1_000.0;
/// Butterworth Q. A 2-pole low-pass is −6 dB at the cutoff.
const FLAT_Q: Smp = 0.5;

#[test]
fn process_clamps_frequency_ratio_to_the_tan_limit() {
    let k = [FLAT_Q.recip()];
    let gain = [1.0];
    let input = [1.0];

    let run = |ratio: Smp| {
        let cutoff = [freq_to_c4_pitch(ratio * SAMPLE_RATE)];
        let mut state = SvfState::new(SvfType::LowPass12);
        let mut output = [0.0];
        let pre_k = [MAX_PRE_Q.recip()];

        process_lane(&mut state, &input, &cutoff, &k, &pre_k, &gain, &mut output);

        output[0]
    };

    assert_eq!(run(0.6), run(1.0));
    assert_ne!(run(0.01), run(1.0));
}

#[test]
fn cutoff_to_g_stays_close_to_libm_tan() {
    let freq_mult = SAMPLE_RATE.recip();
    let freq_mult_x4 = WideSmp::splat(freq_mult);

    for i in 0..=200 {
        let cutoff = -4.0 + i as Smp * 11.0 / 200.0;
        let g = LaneInputs::cutoff_to_g_x4(WideSmp::splat(cutoff), freq_mult_x4).to_array()[0];
        let ratio = (pitch_to_freq(C4_PITCH + cutoff) * freq_mult).min(super::MAX_FREQ_RATIO);
        let expected = (ratio * PI).tan();
        let rel = ((g - expected) / expected).abs();
        let limit = if ratio > 0.25 { 1e-4 } else { 1e-6 };

        assert!(
            rel < limit,
            "cutoff {cutoff} ratio {ratio}: {g} vs {expected} ({rel})"
        );
    }
}

#[test]
fn process_lanes_match_one_sample_at_a_time() {
    let n = 7;
    let input: Vec<_> = (0..n).map(|i| (i as Smp * 0.37).sin()).collect();
    // Spans the direct polynomial and the reciprocal side of `fast_tan_pi_wide`,
    // plus one cutoff past the clamp.
    let cutoff = [-2.0, 0.5, 2.0, 4.0, 6.0, 6.8, 9.0];
    let k = [0.5, 0.2, 1.0, 0.05, 2.0, 0.7, 0.3];
    let pre_k = [2.0, 1.0, 2.0, 1.5, 2.0, 0.8, 1.2];
    let gain = [0.25, 1.0, 4.0, 1e-8, 0.01, 2.5, 0.5];

    for filter_type in SvfType::ALL {
        let mut block = SvfState::new(filter_type);
        let mut step = SvfState::new(filter_type);
        let mut out_block = [0.0; 7];
        let mut out_step = [0.0; 7];

        process_lane(
            &mut block,
            &input,
            &cutoff,
            &k,
            &pre_k,
            &gain,
            &mut out_block,
        );

        for i in 0..n {
            process_lane(
                &mut step,
                &input[i..i + 1],
                &cutoff[i..i + 1],
                &k[i..i + 1],
                &pre_k[i..i + 1],
                &gain[i..i + 1],
                &mut out_step[i..i + 1],
            );
        }

        assert_eq!(
            out_block, out_step,
            "{filter_type:?} chunked process diverged from per-sample process"
        );
    }
}

/// One sample through lane 0. The other lanes stay silent.
fn tick(state: &mut SvfState, cutoff: Smp, k: Smp, gain: Smp, input: Smp) -> Smp {
    let audio = [input];
    let cutoff = [cutoff];
    let k = [k];
    let pre_k = [MAX_PRE_Q.recip()];
    let gain = [gain];
    let zero = [0.0];
    let mut out = [0.0];
    let mut sink_a = [0.0];
    let mut sink_b = [0.0];
    let mut sink_c = [0.0];

    state.process(
        SAMPLE_RATE,
        1,
        LaneInputs {
            audio: [&audio, &zero, &zero, &zero],
            cutoff: [&cutoff, &zero, &zero, &zero],
            k: [&k, &zero, &zero, &zero],
            pre_k: [&pre_k, &zero, &zero, &zero],
            gain: [&gain, &zero, &zero, &zero],
        },
        [&mut out, &mut sink_a, &mut sink_b, &mut sink_c],
    );

    out[0]
}

fn process_lane(
    state: &mut SvfState,
    audio: &[Smp],
    cutoff: &[Smp],
    k: &[Smp],
    pre_k: &[Smp],
    gain: &[Smp],
    output: &mut [Smp],
) {
    let zeros = vec![0.0; audio.len()];
    let mut sink_a = vec![0.0; audio.len()];
    let mut sink_b = vec![0.0; audio.len()];
    let mut sink_c = vec![0.0; audio.len()];

    state.process(
        SAMPLE_RATE,
        audio.len(),
        LaneInputs {
            audio: [audio, &zeros, &zeros, &zeros],
            cutoff: [cutoff, &zeros, &zeros, &zeros],
            k: [k, &zeros, &zeros, &zeros],
            pre_k: [pre_k, &zeros, &zeros, &zeros],
            gain: [gain, &zeros, &zeros, &zeros],
        },
        [output, &mut sink_a, &mut sink_b, &mut sink_c],
    );
}

/// Steady-state amplitude of lane 0 for a unit sine at `freq`.
fn measured_gain(filter_type: SvfType, q: Smp, gain: Smp, freq: Smp) -> Smp {
    let cutoff = freq_to_c4_pitch(CUTOFF);
    let k = q.recip();
    let mut state = SvfState::new(filter_type);
    let settle = SAMPLE_RATE as usize;
    let measure = SAMPLE_RATE as usize / 4;
    let mut sum_sq = 0.0f64;

    for n in 0..settle + measure {
        let x = (TAU * freq * n as Smp / SAMPLE_RATE).sin();
        let y = tick(&mut state, cutoff, k, gain, x);

        if n >= settle {
            sum_sq += (y as f64) * (y as f64);
        }
    }

    ((sum_sq / measure as f64).sqrt() * std::f64::consts::SQRT_2) as Smp
}

fn predicted_gain(filter_type: SvfType, q: Smp, gain: Smp, freq: Smp) -> Smp {
    let w = (PI * freq / SAMPLE_RATE).tan();
    let g = (PI * CUTOFF / SAMPLE_RATE).tan();

    let response = SvfResponse {
        filter_type,
        q,
        pre_q: MAX_PRE_Q,
        cutoff: g,
        gain,
    };

    response.at(w).norm()
}

fn analog_gain(filter_type: SvfType, q: Smp, freq: Smp) -> Smp {
    analog_gain_with(filter_type, q, 1.0, freq)
}

fn analog_gain_with(filter_type: SvfType, q: Smp, gain: Smp, freq: Smp) -> Smp {
    let response = SvfResponse {
        filter_type,
        q,
        pre_q: MAX_PRE_Q,
        cutoff: CUTOFF,
        gain,
    };

    response.at(freq).norm()
}

fn assert_close(measured: Smp, predicted: Smp, ctx: &str) {
    let tolerance = (predicted * 0.03).max(2e-3);

    assert!(
        (measured - predicted).abs() <= tolerance,
        "{ctx}: measured {measured} vs predicted {predicted}"
    );
}

#[test]
fn tick_matches_bilinear_response_for_all_types() {
    for filter_type in SvfType::ALL {
        for q in [FLAT_Q, 1.0, 4.0] {
            for freq in [250.0, 1_000.0, 4_000.0] {
                assert_close(
                    measured_gain(filter_type, q, 1.0, freq),
                    predicted_gain(filter_type, q, 1.0, freq),
                    &format!("{filter_type:?} q={q} f={freq}"),
                );
            }
        }
    }
}

#[test]
fn analog_prototype_dc_and_cutoff_gains() {
    for filter_type in [SvfType::LowPass12, SvfType::LowPass18, SvfType::LowPass24] {
        assert!(
            (analog_gain(filter_type, FLAT_Q, 0.0) - 1.0).abs() < 1e-6,
            "{filter_type:?}"
        );
    }

    for filter_type in [
        SvfType::HighPass12,
        SvfType::HighPass18,
        SvfType::HighPass24,
    ] {
        assert!(
            analog_gain(filter_type, FLAT_Q, 0.0) < 1e-6,
            "{filter_type:?}"
        );
    }

    // Normalized band-pass stages are unity at the cutoff for any Q.
    for filter_type in [
        SvfType::BandPass6,
        SvfType::BandPass12,
        SvfType::BandPass18,
        SvfType::BandPass24,
    ] {
        for q in [0.5, 4.0, 16.0] {
            assert!(
                (analog_gain(filter_type, q, CUTOFF) - 1.0).abs() < 1e-5,
                "{filter_type:?} q={q}"
            );
        }
    }
}

#[test]
fn lowpass24_cutoff_gain_is_the_product_of_the_stage_qs() {
    let q = 2.0;
    let pre_q = 0.5;
    let response = SvfResponse {
        filter_type: SvfType::LowPass24,
        q,
        pre_q,
        cutoff: CUTOFF,
        gain: 1.0,
    };

    assert!((response.at(CUTOFF).norm() - q * pre_q).abs() < 1e-5);
}

#[test]
fn zero_resonance_lowpass_is_minus_6db_at_cutoff() {
    // |H(fc)| of a 2-pole low-pass equals its Q, so Q = 0.5 is −6 dB.
    let gain = analog_gain(SvfType::LowPass12, FLAT_Q, CUTOFF);

    assert!((gain - FLAT_Q).abs() < 1e-5, "{gain}");
}

#[test]
fn steeper_types_roll_off_faster() {
    let far = 8.0 * CUTOFF;
    let gain = |filter_type| analog_gain(filter_type, FLAT_Q, far);

    assert!(gain(SvfType::LowPass12) > gain(SvfType::LowPass18));
    assert!(gain(SvfType::LowPass18) > gain(SvfType::LowPass24));
    assert!(gain(SvfType::BandPass6) > gain(SvfType::BandPass12));
    assert!(gain(SvfType::BandPass12) > gain(SvfType::BandPass18));
    assert!(gain(SvfType::BandPass18) > gain(SvfType::BandPass24));

    let near = CUTOFF / 8.0;
    let gain = |filter_type| analog_gain(filter_type, FLAT_Q, near);

    assert!(gain(SvfType::HighPass12) > gain(SvfType::HighPass18));
    assert!(gain(SvfType::HighPass18) > gain(SvfType::HighPass24));
}

fn assert_impulse_decays(filter_type: SvfType, gain: Smp) {
    let cutoff = freq_to_c4_pitch(CUTOFF);
    let k = 16.0f32.recip();
    let mut state = SvfState::new(filter_type);
    let mut peak_early = 0.0f32;
    let mut peak_late = 0.0f32;

    for n in 0..SAMPLE_RATE as usize {
        let x = if n == 0 { 1.0 } else { 0.0 };
        let y = tick(&mut state, cutoff, k, gain, x);

        assert!(y.is_finite(), "{filter_type:?} gain={gain} at {n}");

        if n < 2_000 {
            peak_early = peak_early.max(y.abs());
        } else if n >= 40_000 {
            peak_late = peak_late.max(y.abs());
        }
    }

    assert!(
        peak_late < peak_early * 0.01,
        "{filter_type:?} gain={gain}: early {peak_early} late {peak_late}"
    );
}

#[test]
fn high_resonance_impulse_decays_and_stays_finite() {
    for filter_type in SvfType::ALL {
        assert_impulse_decays(filter_type, 1.0);

        if matches!(
            filter_type,
            SvfType::Peaking
                | SvfType::LowShelf12
                | SvfType::LowShelf24
                | SvfType::HighShelf12
                | SvfType::HighShelf24
        ) {
            assert_impulse_decays(filter_type, 4.0);
            assert_impulse_decays(filter_type, 0.25);
        }
    }
}

#[test]
fn fresh_state_has_no_memory() {
    let cutoff = freq_to_c4_pitch(CUTOFF);
    let k = 0.5;
    let mut state = SvfState::new(SvfType::LowPass24);

    for _ in 0..100 {
        tick(&mut state, cutoff, k, 1.0, 1.0);
    }

    state = SvfState::new(SvfType::LowPass24);

    assert_eq!(tick(&mut state, cutoff, k, 1.0, 0.0), 0.0);
}

#[test]
fn notch_passes_away_from_cutoff_and_nulls_it() {
    assert!((analog_gain(SvfType::Notch, FLAT_Q, 0.0) - 1.0).abs() < 1e-6);
    assert!(analog_gain(SvfType::Notch, 4.0, CUTOFF) < 1e-6);
    assert!((analog_gain(SvfType::Notch, FLAT_Q, 100.0 * CUTOFF) - 1.0).abs() < 1e-3);
}

#[test]
fn peaking_is_unity_away_from_cutoff_and_gain_at_cutoff() {
    for gain in [0.25, 4.0] {
        assert!(
            (analog_gain_with(SvfType::Peaking, 2.0, gain, 0.0) - 1.0).abs() < 1e-6,
            "dc gain={gain}"
        );
        assert!(
            (analog_gain_with(SvfType::Peaking, 2.0, gain, CUTOFF) - gain).abs() < 1e-5,
            "cutoff gain={gain}"
        );
        assert!(
            (analog_gain_with(SvfType::Peaking, 2.0, gain, 100.0 * CUTOFF) - 1.0).abs() < 1e-3,
            "high gain={gain}"
        );
    }
}

#[test]
fn peaking_tick_matches_bilinear_response() {
    for gain in [0.25, 4.0] {
        for q in [FLAT_Q, 1.0, 4.0] {
            for freq in [250.0, 1_000.0, 4_000.0] {
                assert_close(
                    measured_gain(SvfType::Peaking, q, gain, freq),
                    predicted_gain(SvfType::Peaking, q, gain, freq),
                    &format!("peaking gain={gain} q={q} f={freq}"),
                );
            }
        }
    }
}

fn settled_dc(filter_type: SvfType, gain: Smp) -> Smp {
    let cutoff = freq_to_c4_pitch(CUTOFF);
    let mut state = SvfState::new(filter_type);
    let mut output = 0.0;

    for _ in 0..8_000 {
        output = tick(&mut state, cutoff, 0.5, gain, 1.0);
    }

    output
}

#[test]
fn peaking_dc_stays_unity_while_drive_scales_the_other_types() {
    assert!((settled_dc(SvfType::Peaking, 4.0) - 1.0).abs() < 1e-3);
    assert!((settled_dc(SvfType::Notch, 4.0) - 4.0).abs() < 1e-2);
    assert!((settled_dc(SvfType::LowPass12, 4.0) - 4.0).abs() < 1e-2);
}

fn analog_h(filter_type: SvfType, q: Smp, gain: Smp, freq: Smp) -> ComplexSample {
    SvfResponse {
        filter_type,
        q,
        pre_q: MAX_PRE_Q,
        cutoff: CUTOFF,
        gain,
    }
    .at(freq)
}

#[test]
fn lowshelf_boosts_dc_and_is_unity_at_high_freq() {
    for filter_type in [SvfType::LowShelf12, SvfType::LowShelf24] {
        for gain in [0.25, 4.0] {
            assert!(
                (analog_gain_with(filter_type, 2.0, gain, 0.0) - gain).abs() < 1e-5,
                "{filter_type:?} dc gain={gain}"
            );
            assert!(
                (analog_gain_with(filter_type, 2.0, gain, CUTOFF) - gain.sqrt()).abs() < 1e-5,
                "{filter_type:?} cutoff gain={gain}"
            );
            assert!(
                (analog_gain_with(filter_type, 2.0, gain, 100.0 * CUTOFF) - 1.0).abs() < 1e-3,
                "{filter_type:?} high gain={gain}"
            );
        }
    }
}

#[test]
fn highshelf_is_unity_at_dc_and_boosts_high_freq() {
    for filter_type in [SvfType::HighShelf12, SvfType::HighShelf24] {
        for gain in [0.25, 4.0] {
            assert!(
                (analog_gain_with(filter_type, 2.0, gain, 0.0) - 1.0).abs() < 1e-5,
                "{filter_type:?} dc gain={gain}"
            );
            assert!(
                (analog_gain_with(filter_type, 2.0, gain, CUTOFF) - gain.sqrt()).abs() < 1e-5,
                "{filter_type:?} cutoff gain={gain}"
            );
            assert!(
                (analog_gain_with(filter_type, 2.0, gain, 100.0 * CUTOFF) - gain).abs() < 1e-3,
                "{filter_type:?} high gain={gain}"
            );
        }
    }
}

#[test]
fn shelves_are_identity_at_unity_gain() {
    for filter_type in [
        SvfType::LowShelf12,
        SvfType::LowShelf24,
        SvfType::HighShelf12,
        SvfType::HighShelf24,
    ] {
        for freq in [0.0, 0.25 * CUTOFF, CUTOFF, 4.0 * CUTOFF] {
            let h = analog_h(filter_type, 2.0, 1.0, freq);

            assert!((h.re - 1.0).abs() < 1e-6, "{filter_type:?} f={freq}");
            assert!(h.im.abs() < 1e-6, "{filter_type:?} f={freq}");
        }
    }
}

#[test]
fn shelf_boost_and_cut_are_inverses() {
    let freq = 1.7 * CUTOFF;
    let gain: Smp = 4.0;
    let cut = gain.recip();

    for filter_type in [
        SvfType::LowShelf12,
        SvfType::LowShelf24,
        SvfType::HighShelf12,
        SvfType::HighShelf24,
    ] {
        let product =
            analog_h(filter_type, 2.0, gain, freq) * analog_h(filter_type, 2.0, cut, freq);

        assert!(
            (product.re - 1.0).abs() < 1e-5 && product.im.abs() < 1e-5,
            "{filter_type:?} {product}"
        );
    }
}

#[test]
fn low_and_high_shelf_product_is_flat_gain() {
    let freq = 1.7 * CUTOFF;
    let gain = 4.0;

    for (low, high) in [
        (SvfType::LowShelf12, SvfType::HighShelf12),
        (SvfType::LowShelf24, SvfType::HighShelf24),
    ] {
        let product = analog_h(low, 2.0, gain, freq) * analog_h(high, 2.0, gain, freq);

        assert!((product.re - gain).abs() < 1e-4, "{low:?} {product}");
        assert!(product.im.abs() < 1e-4, "{low:?} {product}");
    }
}

#[test]
fn shelf_tick_matches_bilinear_response() {
    for filter_type in [
        SvfType::LowShelf12,
        SvfType::LowShelf24,
        SvfType::HighShelf12,
        SvfType::HighShelf24,
    ] {
        for gain in [0.25, 4.0] {
            for q in [FLAT_Q, 1.0, 4.0] {
                for freq in [250.0, 1_000.0, 4_000.0] {
                    assert_close(
                        measured_gain(filter_type, q, gain, freq),
                        predicted_gain(filter_type, q, gain, freq),
                        &format!("{filter_type:?} gain={gain} q={q} f={freq}"),
                    );
                }
            }
        }
    }
}

#[test]
fn shelf_dc_follows_the_shelf_gain() {
    for filter_type in [SvfType::LowShelf12, SvfType::LowShelf24] {
        assert!(
            (settled_dc(filter_type, 4.0) - 4.0).abs() < 1e-2,
            "{filter_type:?}"
        );
        assert!(
            (settled_dc(filter_type, 0.25) - 0.25).abs() < 1e-3,
            "{filter_type:?}"
        );
    }

    for filter_type in [SvfType::HighShelf12, SvfType::HighShelf24] {
        assert!(
            (settled_dc(filter_type, 4.0) - 1.0).abs() < 1e-3,
            "{filter_type:?}"
        );
        assert!(
            (settled_dc(filter_type, 0.25) - 1.0).abs() < 1e-3,
            "{filter_type:?}"
        );
    }
}

#[test]
fn steeper_shelves_transition_faster() {
    let gain = 10.0;
    let below = 0.5 * CUTOFF;
    let above = 2.0 * CUTOFF;

    assert!(
        analog_gain_with(SvfType::LowShelf24, FLAT_Q, gain, above)
            < analog_gain_with(SvfType::LowShelf12, FLAT_Q, gain, above)
    );
    assert!(
        analog_gain_with(SvfType::LowShelf24, FLAT_Q, gain, below)
            > analog_gain_with(SvfType::LowShelf12, FLAT_Q, gain, below)
    );
    assert!(
        analog_gain_with(SvfType::HighShelf24, FLAT_Q, gain, below)
            < analog_gain_with(SvfType::HighShelf12, FLAT_Q, gain, below)
    );
    assert!(
        analog_gain_with(SvfType::HighShelf24, FLAT_Q, gain, above)
            > analog_gain_with(SvfType::HighShelf12, FLAT_Q, gain, above)
    );
}

#[test]
fn types_round_trip_through_serde() {
    for filter_type in SvfType::ALL {
        let json = serde_json::to_string(&filter_type).unwrap();
        let back: SvfType = serde_json::from_str(&json).unwrap();

        assert_eq!(back, filter_type);
    }
}

#[test]
fn default_state_is_lowpass12() {
    assert_eq!(SvfType::default(), SvfType::LowPass12);

    let cutoff = freq_to_c4_pitch(CUTOFF);
    let k = 0.5;
    let mut state = SvfState::default();
    let mut lowpass = SvfState::new(SvfType::LowPass12);

    for input in [0.0, 1.0, -1.0, 0.3] {
        assert_eq!(
            tick(&mut state, cutoff, k, 1.0, input),
            tick(&mut lowpass, cutoff, k, 1.0, input)
        );
    }
}

fn drive(state: &mut SvfState) {
    let cutoff = freq_to_c4_pitch(CUTOFF);

    for _ in 0..32 {
        tick(state, cutoff, 0.25, 2.0, 1.0);
    }
}

fn tick_sequence(state: &mut SvfState) -> [Smp; 4] {
    let cutoff = freq_to_c4_pitch(CUTOFF);
    let mut output = [0.0; 4];

    for (out, input) in output.iter_mut().zip([0.0, 1.0, -0.5, 0.25]) {
        *out = tick(state, cutoff, 0.25, 2.0, input);
    }

    output
}

#[test]
fn set_type_keeps_integrator_state_when_the_type_is_unchanged() {
    for filter_type in SvfType::ALL {
        let mut driven = SvfState::new(filter_type);

        drive(&mut driven);

        let mut kept = driven;
        let mut untouched = driven;
        let mut fresh = SvfState::new(filter_type);

        kept.set_type(filter_type);

        let kept_out = tick_sequence(&mut kept);
        let untouched_out = tick_sequence(&mut untouched);
        let fresh_out = tick_sequence(&mut fresh);

        assert_eq!(kept_out, untouched_out, "{filter_type:?}");
        assert!(
            kept_out
                .iter()
                .zip(fresh_out)
                .any(|(kept, fresh)| (kept - fresh).abs() > 1e-4),
            "{filter_type:?} lost its state"
        );
    }
}

#[test]
fn set_type_clears_integrator_state_when_the_type_changes() {
    let types = SvfType::ALL;

    for (i, &filter_type) in types.iter().enumerate() {
        let next = types[(i + 1) % types.len()];
        let mut state = SvfState::new(filter_type);

        drive(&mut state);
        state.set_type(next);

        let mut fresh = SvfState::new(next);

        assert_eq!(
            tick_sequence(&mut state),
            tick_sequence(&mut fresh),
            "{filter_type:?} -> {next:?}"
        );
    }
}

#[test]
fn reset_zeros_one_slot() {
    let n = 16;
    let audio = [0.2; 16];
    let cutoff = [1.0; 16];
    let k = [0.5; 16];
    let pre_k = [2.0; 16];
    let gain = [1.0; 16];
    let audio_refs = [&audio[..]; SIMD_LANES];
    let cutoff_refs = [&cutoff[..]; SIMD_LANES];
    let k_refs = [&k[..]; SIMD_LANES];
    let pre_k_refs = [&pre_k[..]; SIMD_LANES];
    let gain_refs = [&gain[..]; SIMD_LANES];

    let mut state = SvfState::new(SvfType::LowPass24);
    let mut out = [[0.0; 16]; SIMD_LANES];
    {
        let [o0, o1, o2, o3] = out.each_mut();
        state.process(
            SAMPLE_RATE,
            n,
            LaneInputs {
                audio: audio_refs,
                cutoff: cutoff_refs,
                k: k_refs,
                pre_k: pre_k_refs,
                gain: gain_refs,
            },
            [o0, o1, o2, o3],
        );
    }

    state.reset(0);

    let mut cleared = [[0.0; 1]; SIMD_LANES];
    let silent = [0.0; 1];
    let silent_refs = [&silent[..]; SIMD_LANES];
    let [o0, o1, o2, o3] = cleared.each_mut();

    state.process(
        SAMPLE_RATE,
        1,
        LaneInputs {
            audio: silent_refs,
            cutoff: cutoff_refs,
            k: k_refs,
            pre_k: pre_k_refs,
            gain: gain_refs,
        },
        [o0, o1, o2, o3],
    );

    assert!(cleared[0][0].abs() < 1e-6);
    assert!(cleared[1][0].abs() < 1e-6);
    assert!(cleared[2][0].abs() > 1e-5 || cleared[3][0].abs() > 1e-5);
}
