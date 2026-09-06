use super::*;
use crate::utils::{db_to_gain_fast, from_st, power_scale};

const EPS: Sample = 1e-4;
const CUTOFF: Sample = 1.0;
const GAIN: Sample = 1.0;
const Q: Sample = BUTTERWORTH_Q;

fn assert_approx(a: Sample, b: Sample) {
    assert_approx_eps(a, b, EPS);
}

fn assert_approx_eps(a: Sample, b: Sample, eps: Sample) {
    assert!(
        (a - b).abs() < eps,
        "{a} ≈ {b} (diff {}, eps {eps})",
        (a - b).abs()
    );
}

fn assert_complex_eq(a: ComplexSample, b: ComplexSample) {
    assert_approx(a.re, b.re);
    assert_approx(a.im, b.im);
}

fn params(cutoff: Sample, resonance: Sample, q_limit_to: Sample) -> FilterParams {
    FilterParams {
        drive: 0.0,
        cutoff,
        resonance,
        q_limit_to,
        q_limit_curve: 0.0,
        linear_phase: false,
    }
}

fn default_params() -> FilterParams {
    params(0.0, 0.0, MAX_Q_LIMIT)
}

fn filter(filter_type: FilterType, p: FilterParams) -> SpectralFilter {
    SpectralFilter::new(filter_type, p)
}

fn cutoff_freq(p: FilterParams) -> Sample {
    filter(FilterType::LowPass12, p).cutoff_freq
}

fn mag<T: FilterImpl>(freq: Sample) -> Sample {
    mag_params::<T>(GAIN, CUTOFF, Q, freq)
}

fn mag_params<T: FilterImpl>(gain: Sample, cutoff: Sample, q: Sample, freq: Sample) -> Sample {
    T::new(gain, cutoff, q).at(freq).norm()
}

fn at<T: FilterImpl>(freq: Sample) -> ComplexSample {
    T::new(GAIN, CUTOFF, Q).at(freq)
}

// ---- FilterType ----

#[test]
fn filter_type_all_lists_every_variant_once() {
    assert_eq!(FilterType::ALL.len(), 12);

    for (i, ty) in FilterType::ALL.iter().enumerate() {
        assert_eq!(
            FilterType::ALL.iter().filter(|t| *t == ty).count(),
            1,
            "duplicate {} at {i}",
            ty.label()
        );
    }
}

#[test]
fn filter_type_labels() {
    let expected = [
        (FilterType::LowPass12, "Lowpass 12"),
        (FilterType::LowPass18, "Lowpass 18"),
        (FilterType::LowPass24, "Lowpass 24"),
        (FilterType::HighPass12, "Highpass 12"),
        (FilterType::HighPass18, "Highpass 18"),
        (FilterType::HighPass24, "Highpass 24"),
        (FilterType::BandPass6, "Bandpass 6"),
        (FilterType::BandPass12, "Bandpass 12"),
        (FilterType::BandPass18, "Bandpass 18"),
        (FilterType::BandPass24, "Bandpass 24"),
        (FilterType::Peaking, "Peaking"),
        (FilterType::Notch, "Notch"),
    ];

    for (ty, label) in expected {
        assert_eq!(ty.label(), label);
    }
}

#[test]
fn filter_type_default_is_lowpass12() {
    assert!(FilterType::default() == FilterType::LowPass12);
}

#[test]
fn filter_type_serde_roundtrip() {
    for ty in FilterType::ALL {
        let json = serde_json::to_string(&ty).unwrap();
        let parsed: FilterType = serde_json::from_str(&json).unwrap();
        assert!(parsed == ty);
    }
}

#[test]
fn filter_type_serde_bandpass_alias() {
    let parsed: FilterType = serde_json::from_str(r#""BandPass""#).unwrap();
    assert!(parsed == FilterType::BandPass6);
}

// ---- SpectralFilter::new cutoff / drive ----

#[test]
fn cutoff_is_octaves_of_the_fundamental() {
    assert_approx(cutoff_freq(params(0.0, 0.0, MAX_Q_LIMIT)), 1.0);
    assert_approx(cutoff_freq(params(1.0, 0.0, MAX_Q_LIMIT)), 2.0);
}

#[test]
fn cutoff_is_clamped() {
    let over = cutoff_freq(params(MAX_CUTOFF + 5.0, 0.0, MAX_Q_LIMIT));
    assert_approx(over, MAX_CUTOFF.exp2());

    let under = cutoff_freq(params(MIN_CUTOFF - 5.0, 0.0, MAX_Q_LIMIT));
    assert_approx(under, MIN_CUTOFF.exp2());
}

#[test]
fn drive_converts_to_linear_gain() {
    let mut p = default_params();
    p.drive = 6.0;
    assert_approx(filter(FilterType::LowPass12, p).gain, db_to_gain_fast(6.0));
}

#[test]
fn drive_is_clamped_to_max() {
    let mut p = default_params();
    p.drive = MAX_DRIVE + 20.0;
    assert_approx(
        filter(FilterType::LowPass12, p).gain,
        db_to_gain_fast(MAX_DRIVE),
    );
}

#[test]
fn linear_phase_flag_is_stored() {
    let mut p = default_params();
    assert!(!filter(FilterType::LowPass12, p).linear_phase);

    p.linear_phase = true;
    assert!(filter(FilterType::LowPass12, p).linear_phase);
}

// ---- Q mapping ----

fn q_of(p: FilterParams) -> Sample {
    filter(FilterType::LowPass12, p).q
}

#[test]
fn zero_resonance_is_butterworth_q() {
    assert_approx(q_of(params(0.0, 0.0, MAX_Q_LIMIT)), BUTTERWORTH_Q);
}

#[test]
fn max_resonance_maps_to_max_q() {
    // cutoff above q_limit_to so limiting does not apply
    assert_approx(q_of(params(8.0, MAX_RESONANCE, 2.0)), MAX_Q);
}

#[test]
fn min_resonance_maps_to_min_q() {
    assert_approx(q_of(params(0.0, MIN_RESONANCE, MAX_Q_LIMIT)), MIN_Q);
}

#[test]
fn positive_resonance_uses_cubic_curve() {
    let resonance: Sample = 0.5;
    let expected = BUTTERWORTH_Q + (MAX_Q - BUTTERWORTH_Q) * resonance.powf(3.0);
    // cutoff above q_limit_to so limiting does not apply
    assert_approx(q_of(params(8.0, resonance, 2.0)), expected);
}

#[test]
fn negative_resonance_interpolates_to_min_q() {
    let resonance = -0.5;
    let expected = MIN_Q + (BUTTERWORTH_Q - MIN_Q) * (1.0 + resonance);
    assert_approx(q_of(params(0.0, resonance, MAX_Q_LIMIT)), expected);
}

#[test]
fn resonance_is_clamped() {
    assert_approx(q_of(params(8.0, MAX_RESONANCE + 1.0, 2.0)), MAX_Q);
    assert_approx(q_of(params(0.0, MIN_RESONANCE - 1.0, MAX_Q_LIMIT)), MIN_Q);
}

#[test]
fn q_limit_skipped_when_limit_below_one_semitone() {
    let unlimited = q_of(params(8.0, 1.0, 2.0));
    let tiny_limit = q_of(params(0.0, 1.0, from_st(1.0) * 0.5));
    assert_approx(tiny_limit, unlimited);
    assert_approx(tiny_limit, MAX_Q);
}

#[test]
fn q_limit_skipped_when_resonance_not_above_butterworth() {
    let limited = q_of(params(0.0, 0.0, 2.0));
    assert_approx(limited, BUTTERWORTH_Q);
}

#[test]
fn q_limit_skipped_when_cutoff_above_limit() {
    let unlimited = q_of(params(3.0, 1.0, 2.0));
    assert_approx(unlimited, MAX_Q);
}

#[test]
fn q_limit_reduces_q_below_limit_frequency() {
    let unlimited = q_of(params(8.0, 1.0, 2.0));
    let fully_limited = q_of(params(0.0, 1.0, 2.0));
    let halfway = q_of(params(1.0, 1.0, 2.0));

    assert_approx(unlimited, MAX_Q);
    assert_approx(fully_limited, BUTTERWORTH_Q);
    assert!(halfway > fully_limited);
    assert!(halfway < unlimited);

    // q_limit_curve = 0 → linear power_scale
    let t = 1.0 / 2.0;
    let expected = BUTTERWORTH_Q + (MAX_Q - BUTTERWORTH_Q) * power_scale(t, 0.0);
    assert_approx(halfway, expected);
}

#[test]
fn q_limit_at_the_limit_frequency_is_unlimited() {
    // cutoff == q_limit_to → t = 1, so the curve does not reduce Q
    assert_approx(q_of(params(2.0, 1.0, 2.0)), MAX_Q);
}

#[test]
fn q_limit_curve_changes_the_shape() {
    let mut linear = params(1.0, 1.0, 2.0);
    linear.q_limit_curve = 0.0;
    let mut curved = linear;
    curved.q_limit_curve = 1.0;

    let t = 1.0 / 2.0;
    assert_approx(
        q_of(linear),
        BUTTERWORTH_Q + (MAX_Q - BUTTERWORTH_Q) * power_scale(t, 0.0),
    );
    assert_approx(
        q_of(curved),
        BUTTERWORTH_Q + (MAX_Q - BUTTERWORTH_Q) * power_scale(t, MAX_Q_LIMIT_POWER),
    );
    assert!(q_of(curved) < q_of(linear));
}

#[test]
fn q_limit_to_is_clamped() {
    let at_max = q_of(params(5.0, 1.0, MAX_Q_LIMIT));
    let above_max = q_of(params(5.0, 1.0, MAX_Q_LIMIT + 10.0));
    assert_approx(at_max, above_max);

    // Negative limit clamps to 0, which is below one semitone → no limiting
    let negative = q_of(params(0.0, 1.0, -1.0));
    assert_approx(negative, MAX_Q);
}

// ---- FilterImpl frequency responses ----

#[test]
fn lowpass12_dc_gain_and_cutoff_magnitude() {
    assert_approx(mag::<LowPass12>(0.0), GAIN);
    assert_approx(mag::<LowPass12>(CUTOFF), GAIN * Q);
    assert!(mag::<LowPass12>(10.0 * CUTOFF) < 0.02);
}

#[test]
fn lowpass12_cutoff_phase_is_minus_90_deg() {
    assert_approx(at::<LowPass12>(CUTOFF).arg(), -std::f32::consts::FRAC_PI_2);
}

#[test]
fn highpass12_dc_is_zero_and_high_freq_is_gain() {
    assert_approx(mag::<HighPass12>(0.0), 0.0);
    assert_approx(mag::<HighPass12>(CUTOFF), GAIN * Q);
    assert_approx_eps(mag::<HighPass12>(100.0 * CUTOFF), GAIN, 1e-3);
}

#[test]
fn highpass12_cutoff_phase_is_plus_90_deg() {
    assert_approx(at::<HighPass12>(CUTOFF).arg(), std::f32::consts::FRAC_PI_2);
}

#[test]
fn bandpass6_peaks_at_cutoff() {
    assert_approx(mag::<BandPass6>(0.0), 0.0);
    assert_approx(mag::<BandPass6>(CUTOFF), GAIN);
    assert!(mag::<BandPass6>(10.0 * CUTOFF) < 0.15);
    assert!(mag::<BandPass6>(0.1 * CUTOFF) < 0.15);
}

#[test]
fn peaking_is_unity_away_from_cutoff_and_gain_squared_at_cutoff() {
    let gain = 2.0;
    assert_approx(mag_params::<Peaking>(gain, CUTOFF, Q, 0.0), 1.0);
    assert_approx(mag_params::<Peaking>(gain, CUTOFF, Q, CUTOFF), gain * gain);
    assert_approx_eps(
        mag_params::<Peaking>(gain, CUTOFF, Q, 100.0 * CUTOFF),
        1.0,
        1e-3,
    );
}

#[test]
fn notch_nulls_cutoff_and_passes_elsewhere() {
    assert_approx(mag::<Notch>(0.0), GAIN);
    assert_approx(mag::<Notch>(CUTOFF), 0.0);
    assert_approx_eps(mag::<Notch>(100.0 * CUTOFF), GAIN, 1e-3);
}

#[test]
fn lowpass18_is_biquad_times_one_pole() {
    let freq = 1.7;
    let w = CUTOFF * TAU;
    let x = freq * TAU;
    let one_pole = w / ComplexSample::new(w, x);
    let expected = one_pole * at::<LowPass12>(freq);

    assert_complex_eq(at::<LowPass18>(freq), expected);
    assert_approx(mag::<LowPass18>(0.0), GAIN);
}

#[test]
fn highpass18_is_biquad_times_one_pole() {
    let freq = 1.7;
    let w = CUTOFF * TAU;
    let x = freq * TAU;
    let one_pole = ComplexSample::new(0.0, x) / ComplexSample::new(w, x);
    let expected = one_pole * at::<HighPass12>(freq);

    assert_complex_eq(at::<HighPass18>(freq), expected);
    assert_approx(mag::<HighPass18>(0.0), 0.0);
}

#[test]
fn lowpass24_is_butterworth_times_resonant() {
    let freq = 1.7;
    let gain = 0.5;
    let q = 4.0;
    let expected = LowPass12::new(1.0, CUTOFF, BUTTERWORTH_Q).at(freq)
        * LowPass12::new(gain, CUTOFF, q).at(freq);

    assert_complex_eq(LowPass24::new(gain, CUTOFF, q).at(freq), expected);
    assert_approx(mag_params::<LowPass24>(gain, CUTOFF, q, 0.0), gain);
}

#[test]
fn highpass24_is_butterworth_times_resonant() {
    let freq = 1.7;
    let gain = 0.5;
    let q = 4.0;
    let expected = HighPass12::new(1.0, CUTOFF, BUTTERWORTH_Q).at(freq)
        * HighPass12::new(gain, CUTOFF, q).at(freq);

    assert_complex_eq(HighPass24::new(gain, CUTOFF, q).at(freq), expected);
}

#[test]
fn bandpass_cascades() {
    let freq = 1.7;
    let gain = 0.5;
    let q = 4.0;
    let butterworth = BandPass6::new(1.0, CUTOFF, BUTTERWORTH_Q).at(freq);
    let resonant = BandPass6::new(gain, CUTOFF, q).at(freq);

    assert_complex_eq(
        BandPass12::new(gain, CUTOFF, q).at(freq),
        butterworth * resonant,
    );
    assert_complex_eq(
        BandPass18::new(gain, CUTOFF, q).at(freq),
        butterworth * butterworth * resonant,
    );
    assert_complex_eq(
        BandPass24::new(gain, CUTOFF, q).at(freq),
        butterworth * butterworth * butterworth * resonant,
    );

    assert_approx(mag_params::<BandPass12>(gain, CUTOFF, q, CUTOFF), gain);
    assert_approx(mag_params::<BandPass18>(gain, CUTOFF, q, CUTOFF), gain);
    assert_approx(mag_params::<BandPass24>(gain, CUTOFF, q, CUTOFF), gain);
}

#[test]
fn steeper_lowpass_attenuates_high_frequencies_more() {
    let freq = 4.0 * CUTOFF;
    let lp12 = mag::<LowPass12>(freq);
    let lp18 = mag::<LowPass18>(freq);
    let lp24 = mag::<LowPass24>(freq);

    assert!(lp18 < lp12);
    assert!(lp24 < lp18);
}

#[test]
fn steeper_highpass_attenuates_low_frequencies_more() {
    let freq = 0.25 * CUTOFF;
    let hp12 = mag::<HighPass12>(freq);
    let hp18 = mag::<HighPass18>(freq);
    let hp24 = mag::<HighPass24>(freq);

    assert!(hp18 < hp12);
    assert!(hp24 < hp18);
}

#[test]
fn steeper_bandpass_is_narrower() {
    let freq = 4.0 * CUTOFF;
    let bp6 = mag::<BandPass6>(freq);
    let bp12 = mag::<BandPass12>(freq);
    let bp18 = mag::<BandPass18>(freq);
    let bp24 = mag::<BandPass24>(freq);

    assert!(bp12 < bp6);
    assert!(bp18 < bp12);
    assert!(bp24 < bp18);
}

#[test]
fn higher_q_peaks_lowpass_near_cutoff() {
    let dull = mag_params::<LowPass12>(GAIN, CUTOFF, MIN_Q, CUTOFF);
    let butterworth = mag_params::<LowPass12>(GAIN, CUTOFF, BUTTERWORTH_Q, CUTOFF);
    let resonant = mag_params::<LowPass12>(GAIN, CUTOFF, MAX_Q, CUTOFF);

    assert!(dull < butterworth);
    assert!(butterworth < resonant);
    assert_approx(resonant, GAIN * MAX_Q);
}

// ---- SpectralFilter::response_at / apply_response ----

fn ones(len: usize) -> Vec<ComplexSample> {
    vec![ComplexSample::new(1.0, 0.0); len]
}

fn zeros(len: usize) -> Vec<ComplexSample> {
    vec![ComplexSample::new(0.0, 0.0); len]
}

#[test]
fn response_at_matches_filter_impl_for_every_type() {
    let freq = 2.5;
    let p = default_params();

    for ty in FilterType::ALL {
        let f = filter(ty, p);
        let response = f.response_at(freq);
        let expected = match ty {
            FilterType::LowPass12 => LowPass12::new(f.gain, f.cutoff_freq, f.q).at(freq),
            FilterType::LowPass18 => LowPass18::new(f.gain, f.cutoff_freq, f.q).at(freq),
            FilterType::LowPass24 => LowPass24::new(f.gain, f.cutoff_freq, f.q).at(freq),
            FilterType::HighPass12 => HighPass12::new(f.gain, f.cutoff_freq, f.q).at(freq),
            FilterType::HighPass18 => HighPass18::new(f.gain, f.cutoff_freq, f.q).at(freq),
            FilterType::HighPass24 => HighPass24::new(f.gain, f.cutoff_freq, f.q).at(freq),
            FilterType::BandPass6 => BandPass6::new(f.gain, f.cutoff_freq, f.q).at(freq),
            FilterType::BandPass12 => BandPass12::new(f.gain, f.cutoff_freq, f.q).at(freq),
            FilterType::BandPass18 => BandPass18::new(f.gain, f.cutoff_freq, f.q).at(freq),
            FilterType::BandPass24 => BandPass24::new(f.gain, f.cutoff_freq, f.q).at(freq),
            FilterType::Peaking => Peaking::new(f.gain, f.cutoff_freq, f.q).at(freq),
            FilterType::Notch => Notch::new(f.gain, f.cutoff_freq, f.q).at(freq),
        };
        assert_complex_eq(response, expected);
    }
}

#[test]
fn linear_phase_response_is_real_magnitude() {
    let mut p = default_params();
    p.linear_phase = true;
    p.resonance = 0.8;

    let freq = 2.5;
    for ty in FilterType::ALL {
        let mut min_phase = p;
        min_phase.linear_phase = false;
        let complex = filter(ty, min_phase).response_at(freq);
        let linear = filter(ty, p).response_at(freq);

        assert_approx(linear.im, 0.0);
        assert_approx(linear.re, complex.norm());
        assert!(linear.re >= 0.0);
    }
}

#[test]
fn apply_response_skips_dc_bin() {
    let input = ones(8);
    let mut output = vec![ComplexSample::new(42.0, -7.0); 8];
    filter(FilterType::LowPass12, default_params()).apply_response(&input, &mut output);

    assert_complex_eq(output[0], ComplexSample::new(42.0, -7.0));
}

#[test]
fn apply_response_matches_response_at() {
    let input = [
        ComplexSample::new(1.0, 0.0),
        ComplexSample::new(0.5, 0.25),
        ComplexSample::new(-1.0, 0.75),
        ComplexSample::new(0.0, 1.0),
    ];

    for ty in FilterType::ALL {
        for linear_phase in [false, true] {
            let mut p = default_params();
            p.linear_phase = linear_phase;
            p.resonance = 0.6;
            p.drive = 3.0;

            let f = filter(ty, p);
            let mut output = zeros(input.len());
            f.apply_response(&input, &mut output);

            assert_complex_eq(output[0], ComplexSample::new(0.0, 0.0));
            for i in 1..input.len() {
                assert_complex_eq(output[i], input[i] * f.response_at(i as Sample));
            }
        }
    }
}

#[test]
fn apply_response_zips_to_shortest_slice() {
    let input = ones(4);
    let mut output = zeros(2);
    filter(FilterType::Notch, default_params()).apply_response(&input, &mut output);

    assert_eq!(output.len(), 2);
    assert_complex_eq(output[0], ComplexSample::new(0.0, 0.0));
}
