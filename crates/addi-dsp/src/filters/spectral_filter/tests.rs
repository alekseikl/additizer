use super::*;
use wide::f32x4;

use crate::filters::control::{
    MAX_PRE_Q, MAX_RESONANCE, MAX_RESONANCE_Q, MIN_PRE_Q, MIN_RESONANCE, NEGATIVE_RESONANCE_Q,
    ZERO_RESONANCE_Q, q_from_resonance,
};

/// Same bounds as the spectral filter module's Q-rolloff parameter.
const MIN_Q_ROLLOFF: Sample = 3.0;
const MAX_Q_ROLLOFF: Sample = 48.0;

const EPS: Sample = 1e-4;
const CUTOFF: Sample = 1.0;
const GAIN: Sample = 1.0;
const Q: Sample = ZERO_RESONANCE_Q;

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

fn params(cutoff: Sample, q: Sample, q_cutoff: Sample) -> FilterParams {
    FilterParams {
        drive: 0.0,
        cutoff,
        q,
        pre_q: MAX_PRE_Q,
        q_cutoff,
        q_rolloff: MIN_Q_ROLLOFF,
        linear_phase: false,
    }
}

fn default_params() -> FilterParams {
    params(0.0, ZERO_RESONANCE_Q, 8.0)
}

fn filter(filter_type: FilterType, p: FilterParams) -> SpectralFilter {
    SpectralFilter::new(filter_type, p)
}

fn cutoff_freq(p: FilterParams) -> Sample {
    filter(FilterType::LowPass12, p).cutoff_freq
}

/// Lane 0 of [`FilterLanes::at_x4`] at a single frequency.
trait AtFreq: FilterLanes {
    fn at(&self, freq: Sample) -> ComplexSample {
        let response = self.at_x4(f32x4::splat(freq));
        let re = response.re.to_array();
        let im = response.im.to_array();
        ComplexSample::new(re[0], im[0])
    }
}

impl<T: FilterLanes> AtFreq for T {}

fn mag<T: FilterLanes>(freq: Sample) -> Sample {
    mag_params::<T>(GAIN, CUTOFF, Q, freq)
}

fn mag_params<T: FilterLanes>(gain: Sample, cutoff: Sample, q: Sample, freq: Sample) -> Sample {
    T::new(gain, cutoff, q, MAX_PRE_Q).at(freq).norm()
}

fn at<T: FilterLanes>(freq: Sample) -> ComplexSample {
    T::new(GAIN, CUTOFF, Q, MAX_PRE_Q).at(freq)
}

// ---- FilterType ----

#[test]
fn filter_type_all_lists_every_variant_once() {
    assert_eq!(FilterType::ALL.len(), 16);

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
        (FilterType::LowShelf12, "Lowshelf 12"),
        (FilterType::LowShelf24, "Lowshelf 24"),
        (FilterType::HighPass12, "Highpass 12"),
        (FilterType::HighPass18, "Highpass 18"),
        (FilterType::HighPass24, "Highpass 24"),
        (FilterType::HighShelf12, "Highshelf 12"),
        (FilterType::HighShelf24, "Highshelf 24"),
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

#[test]
fn filter_type_serde_shelf_aliases() {
    let low: FilterType = serde_json::from_str(r#""LowShelf""#).unwrap();
    let high: FilterType = serde_json::from_str(r#""HighShelf""#).unwrap();
    assert!(low == FilterType::LowShelf12);
    assert!(high == FilterType::HighShelf12);
}

// ---- SpectralFilter::new cutoff / drive ----

#[test]
fn cutoff_is_octaves_of_the_fundamental() {
    assert_approx(cutoff_freq(params(0.0, 0.0, 8.0)), 1.0);
    assert_approx(cutoff_freq(params(1.0, 0.0, 8.0)), 2.0);
}

#[test]
fn cutoff_is_not_clamped() {
    assert_approx(cutoff_freq(params(12.0, 0.0, 12.0)), 12.0f32.exp2());
    assert_approx(cutoff_freq(params(-9.0, 0.0, 0.0)), (-9.0f32).exp2());
}

#[test]
fn drive_converts_to_linear_gain() {
    let mut p = default_params();
    p.drive = 6.0;
    assert_approx(filter(FilterType::LowPass12, p).gain, db_to_gain_fast(6.0));
}

#[test]
fn drive_is_not_clamped() {
    let mut p = default_params();
    p.drive = 40.0;
    assert_approx(filter(FilterType::LowPass12, p).gain, db_to_gain_fast(40.0));

    p.drive = -80.0;
    assert_approx(
        filter(FilterType::LowPass12, p).gain,
        db_to_gain_fast(-80.0),
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
fn zero_resonance_q_is_one_half() {
    let q = q_from_resonance(0.0);

    assert_approx(q.resonant, ZERO_RESONANCE_Q);
    assert_approx(q.pre_stage, MIN_PRE_Q);
}

#[test]
fn max_resonance_maps_to_max_q() {
    let q = q_from_resonance(MAX_RESONANCE);

    assert_approx(q.resonant, MAX_RESONANCE_Q);
    assert_approx(q.pre_stage, MAX_PRE_Q);
}

#[test]
fn negative_resonance_maps_to_negative_q() {
    let q = q_from_resonance(MIN_RESONANCE);

    assert_approx(q.resonant, NEGATIVE_RESONANCE_Q);
    assert_approx(q.pre_stage, MIN_PRE_Q);
}

#[test]
fn positive_resonance_uses_cubic_curve() {
    let resonance: Sample = 0.5;
    let curve = resonance.powf(3.0);
    let q = q_from_resonance(resonance);

    assert_approx(
        q.resonant,
        ZERO_RESONANCE_Q + (MAX_RESONANCE_Q - ZERO_RESONANCE_Q) * curve,
    );
    assert_approx(
        q.pre_stage,
        MIN_PRE_Q + (MAX_PRE_Q - MIN_PRE_Q) * resonance.sqrt(),
    );
}

#[test]
fn negative_resonance_uses_linear_curve() {
    let resonance = -0.5;
    let q = q_from_resonance(resonance);
    let expected =
        NEGATIVE_RESONANCE_Q + (ZERO_RESONANCE_Q - NEGATIVE_RESONANCE_Q) * (1.0 + resonance);

    assert_approx(q.resonant, expected);
    assert_approx(q.pre_stage, MIN_PRE_Q);
}

#[test]
fn resonance_is_clamped() {
    let above = q_from_resonance(MAX_RESONANCE + 1.0);
    let below = q_from_resonance(MIN_RESONANCE - 1.0);

    assert_approx(above.resonant, MAX_RESONANCE_Q);
    assert_approx(above.pre_stage, MAX_PRE_Q);
    assert_approx(below.resonant, NEGATIVE_RESONANCE_Q);
    assert_approx(below.pre_stage, MIN_PRE_Q);
}

#[test]
fn q_limit_skipped_at_or_below_zero_resonance() {
    let limited = q_of(params(0.0, ZERO_RESONANCE_Q, 2.0));
    assert_approx(limited, ZERO_RESONANCE_Q);
}

#[test]
fn q_limit_skipped_when_cutoff_above_limit() {
    let unlimited = q_of(params(3.0, MAX_RESONANCE_Q, 2.0));
    assert_approx(unlimited, MAX_RESONANCE_Q);
}

fn limited_q(cutoff: Sample, q_cutoff: Sample, db_per_oct: Sample) -> Sample {
    let octaves_below = q_cutoff - cutoff;
    ZERO_RESONANCE_Q
        + (MAX_RESONANCE_Q - ZERO_RESONANCE_Q) * db_to_gain(-db_per_oct * octaves_below)
}

#[test]
fn q_limit_reduces_q_below_limit_frequency() {
    let unlimited = q_of(params(8.0, MAX_RESONANCE_Q, 2.0));
    let one_below = q_of(params(1.0, MAX_RESONANCE_Q, 2.0));
    let two_below = q_of(params(0.0, MAX_RESONANCE_Q, 2.0));

    assert_approx(unlimited, MAX_RESONANCE_Q);
    assert_approx(one_below, limited_q(1.0, 2.0, MIN_Q_ROLLOFF));
    assert_approx(two_below, limited_q(0.0, 2.0, MIN_Q_ROLLOFF));
    assert!(two_below < one_below);
    assert!(two_below > ZERO_RESONANCE_Q);
}

#[test]
fn q_limit_approaches_zero_resonance_far_below_limit() {
    let mut p = params(-4.0, MAX_RESONANCE_Q, 8.0);
    p.q_rolloff = MAX_Q_ROLLOFF;
    let far_below = q_of(p);
    assert!((far_below - ZERO_RESONANCE_Q).abs() < 1e-3);
}

#[test]
fn q_limit_at_the_limit_frequency_is_unlimited() {
    assert_approx(q_of(params(2.0, MAX_RESONANCE_Q, 2.0)), MAX_RESONANCE_Q);
}

#[test]
fn q_rolloff_steepens_the_drop() {
    let mut shallow = params(1.0, MAX_RESONANCE_Q, 2.0);
    shallow.q_rolloff = MIN_Q_ROLLOFF;
    let mut steep = shallow;
    steep.q_rolloff = MAX_Q_ROLLOFF;

    assert_approx(q_of(shallow), limited_q(1.0, 2.0, MIN_Q_ROLLOFF));
    assert_approx(q_of(steep), limited_q(1.0, 2.0, MAX_Q_ROLLOFF));
    assert!(q_of(steep) < q_of(shallow));
}

#[test]
fn raw_q_is_used_when_cutoff_is_above_the_limit() {
    assert_approx(q_of(params(8.0, 4.0, 2.0)), 4.0);
}

#[test]
fn raw_q_is_not_clamped() {
    assert_approx(
        q_of(params(8.0, MAX_RESONANCE_Q + 10.0, 2.0)),
        MAX_RESONANCE_Q + 10.0,
    );
    assert_approx(q_of(params(8.0, 0.001, 2.0)), 0.001);
}

#[test]
fn raw_q_is_limited_below_the_limit_frequency() {
    let q = 4.0;
    let p = params(0.0, q, 2.0);

    let octaves_below = 2.0;
    let expected =
        ZERO_RESONANCE_Q + (q - ZERO_RESONANCE_Q) * db_to_gain(-MIN_Q_ROLLOFF * octaves_below);
    assert_approx(q_of(p), expected);
}

#[test]
fn q_limit_applies_when_limit_is_negative() {
    let unlimited = q_of(params(0.0, MAX_RESONANCE_Q, -1.0));
    let limited = q_of(params(-2.0, MAX_RESONANCE_Q, -1.0));

    assert_approx(unlimited, MAX_RESONANCE_Q);
    assert_approx(limited, limited_q(-2.0, -1.0, MIN_Q_ROLLOFF));
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
    // Q = 0.5 is a wide band-pass; a decade from the cutoff it is still well down.
    assert!(mag::<BandPass6>(10.0 * CUTOFF) < 0.25);
    assert!(mag::<BandPass6>(0.1 * CUTOFF) < 0.25);
}

#[test]
fn peaking_is_unity_away_from_cutoff_and_gain_at_cutoff() {
    let gain = 2.0;
    assert_approx(mag_params::<Peaking>(gain, CUTOFF, Q, 0.0), 1.0);
    assert_approx(mag_params::<Peaking>(gain, CUTOFF, Q, CUTOFF), gain);
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
fn lowshelf_boosts_dc_and_is_unity_at_high_freq() {
    let gain = 2.0;
    for mag_at in [mag_params::<LowShelf12>, mag_params::<LowShelf24>] {
        assert_approx(mag_at(gain, CUTOFF, Q, 0.0), gain);
        assert_approx_eps(mag_at(gain, CUTOFF, Q, 100.0 * CUTOFF), 1.0, 1e-3);
    }
}

#[test]
fn highshelf_is_unity_at_dc_and_boosts_high_freq() {
    let gain = 2.0;
    for mag_at in [mag_params::<HighShelf12>, mag_params::<HighShelf24>] {
        assert_approx(mag_at(gain, CUTOFF, Q, 0.0), 1.0);
        assert_approx_eps(mag_at(gain, CUTOFF, Q, 100.0 * CUTOFF), gain, 1e-3);
    }
}

#[test]
fn shelves_are_identity_at_unity_gain() {
    for freq in [0.0, 0.25 * CUTOFF, CUTOFF, 4.0 * CUTOFF] {
        let identity = ComplexSample::new(1.0, 0.0);
        assert_complex_eq(
            LowShelf12::new(1.0, CUTOFF, Q, MAX_PRE_Q).at(freq),
            identity,
        );
        assert_complex_eq(
            LowShelf24::new(1.0, CUTOFF, Q, MAX_PRE_Q).at(freq),
            identity,
        );
        assert_complex_eq(
            HighShelf12::new(1.0, CUTOFF, Q, MAX_PRE_Q).at(freq),
            identity,
        );
        assert_complex_eq(
            HighShelf24::new(1.0, CUTOFF, Q, MAX_PRE_Q).at(freq),
            identity,
        );
    }
}

#[test]
fn shelf_boost_and_cut_are_inverses() {
    let freq = 1.7;
    let gain: Sample = 2.0;
    let cut = gain.recip();
    let identity = ComplexSample::new(1.0, 0.0);

    assert_complex_eq(
        LowShelf12::new(gain, CUTOFF, Q, MAX_PRE_Q).at(freq)
            * LowShelf12::new(cut, CUTOFF, Q, MAX_PRE_Q).at(freq),
        identity,
    );
    assert_complex_eq(
        LowShelf24::new(gain, CUTOFF, Q, MAX_PRE_Q).at(freq)
            * LowShelf24::new(cut, CUTOFF, Q, MAX_PRE_Q).at(freq),
        identity,
    );
    assert_complex_eq(
        HighShelf12::new(gain, CUTOFF, Q, MAX_PRE_Q).at(freq)
            * HighShelf12::new(cut, CUTOFF, Q, MAX_PRE_Q).at(freq),
        identity,
    );
    assert_complex_eq(
        HighShelf24::new(gain, CUTOFF, Q, MAX_PRE_Q).at(freq)
            * HighShelf24::new(cut, CUTOFF, Q, MAX_PRE_Q).at(freq),
        identity,
    );
}

#[test]
fn low_and_high_shelf_product_is_flat_gain() {
    let freq = 1.7;
    let gain = 2.0;
    let expected = ComplexSample::new(gain, 0.0);

    assert_complex_eq(
        LowShelf12::new(gain, CUTOFF, Q, MAX_PRE_Q).at(freq)
            * HighShelf12::new(gain, CUTOFF, Q, MAX_PRE_Q).at(freq),
        expected,
    );
    assert_complex_eq(
        LowShelf24::new(gain, CUTOFF, Q, MAX_PRE_Q).at(freq)
            * HighShelf24::new(gain, CUTOFF, Q, MAX_PRE_Q).at(freq),
        expected,
    );
}

#[test]
fn higher_q_shelf_has_sharper_transition() {
    let gain = 2.0;
    let below = 0.25 * CUTOFF;
    let above = 4.0 * CUTOFF;

    let res_below = mag_params::<LowShelf12>(gain, CUTOFF, MAX_RESONANCE_Q, below);
    let neutral_below = mag_params::<LowShelf12>(gain, CUTOFF, ZERO_RESONANCE_Q, below);
    let res_above = mag_params::<LowShelf12>(gain, CUTOFF, MAX_RESONANCE_Q, above);
    let neutral_above = mag_params::<LowShelf12>(gain, CUTOFF, ZERO_RESONANCE_Q, above);

    assert!(res_below > neutral_below);
    assert!(res_above < neutral_above);
}

#[test]
fn lowshelf24_is_fixed_times_resonant() {
    let freq = 1.7;
    let gain: Sample = 0.5;
    let q = 4.0;
    let g12 = gain.sqrt();
    let pre_q = 0.75;
    let expected = LowShelf12::new(g12, CUTOFF, pre_q, pre_q).at(freq)
        * LowShelf12::new(g12, CUTOFF, q, pre_q).at(freq);

    assert_complex_eq(LowShelf24::new(gain, CUTOFF, q, pre_q).at(freq), expected);
    assert_approx(mag_params::<LowShelf24>(gain, CUTOFF, q, 0.0), gain);
}

#[test]
fn highshelf24_is_fixed_times_resonant() {
    let freq = 1.7;
    let gain: Sample = 0.5;
    let q = 4.0;
    let g12 = gain.sqrt();
    let pre_q = 0.75;
    let expected = HighShelf12::new(g12, CUTOFF, pre_q, pre_q).at(freq)
        * HighShelf12::new(g12, CUTOFF, q, pre_q).at(freq);

    assert_complex_eq(HighShelf24::new(gain, CUTOFF, q, pre_q).at(freq), expected);
}

#[test]
fn steeper_lowshelf24_transitions_faster_than_12() {
    let gain = 10.0;
    let above = 2.0 * CUTOFF;
    let below = 0.5 * CUTOFF;

    assert!(
        mag_params::<LowShelf24>(gain, CUTOFF, Q, above)
            < mag_params::<LowShelf12>(gain, CUTOFF, Q, above)
    );
    assert!(
        mag_params::<LowShelf24>(gain, CUTOFF, Q, below)
            > mag_params::<LowShelf12>(gain, CUTOFF, Q, below)
    );
}

#[test]
fn steeper_highshelf24_transitions_faster_than_12() {
    let gain = 10.0;
    let below = 0.5 * CUTOFF;
    let above = 2.0 * CUTOFF;

    assert!(
        mag_params::<HighShelf24>(gain, CUTOFF, Q, below)
            < mag_params::<HighShelf12>(gain, CUTOFF, Q, below)
    );
    assert!(
        mag_params::<HighShelf24>(gain, CUTOFF, Q, above)
            > mag_params::<HighShelf12>(gain, CUTOFF, Q, above)
    );
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
fn lowpass24_is_fixed_times_resonant() {
    let freq = 1.7;
    let gain = 0.5;
    let q = 4.0;
    let pre_q = 0.75;
    let expected = LowPass12::new(1.0, CUTOFF, pre_q, pre_q).at(freq)
        * LowPass12::new(gain, CUTOFF, q, pre_q).at(freq);

    assert_complex_eq(LowPass24::new(gain, CUTOFF, q, pre_q).at(freq), expected);
    assert_approx(mag_params::<LowPass24>(gain, CUTOFF, q, 0.0), gain);
}

#[test]
fn highpass24_is_fixed_times_resonant() {
    let freq = 1.7;
    let gain = 0.5;
    let q = 4.0;
    let pre_q = 0.75;
    let expected = HighPass12::new(1.0, CUTOFF, pre_q, pre_q).at(freq)
        * HighPass12::new(gain, CUTOFF, q, pre_q).at(freq);

    assert_complex_eq(HighPass24::new(gain, CUTOFF, q, pre_q).at(freq), expected);
}

#[test]
fn bandpass_cascades() {
    let freq = 1.7;
    let gain = 0.5;
    let q = 4.0;
    let pre_q = 0.75;
    let neutral = BandPass6::new(1.0, CUTOFF, pre_q, pre_q).at(freq);
    let resonant = BandPass6::new(gain, CUTOFF, q, pre_q).at(freq);

    assert_complex_eq(
        BandPass12::new(gain, CUTOFF, q, pre_q).at(freq),
        neutral * resonant,
    );
    assert_complex_eq(
        BandPass18::new(gain, CUTOFF, q, pre_q).at(freq),
        neutral * neutral * resonant,
    );
    assert_complex_eq(
        BandPass24::new(gain, CUTOFF, q, pre_q).at(freq),
        neutral * neutral * neutral * resonant,
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
    let dull = mag_params::<LowPass12>(GAIN, CUTOFF, NEGATIVE_RESONANCE_Q, CUTOFF);
    let neutral = mag_params::<LowPass12>(GAIN, CUTOFF, ZERO_RESONANCE_Q, CUTOFF);
    let resonant = mag_params::<LowPass12>(GAIN, CUTOFF, MAX_RESONANCE_Q, CUTOFF);

    assert!(dull < neutral);
    assert!(neutral < resonant);
    assert_approx(resonant, GAIN * MAX_RESONANCE_Q);
}

// ---- SpectralFilter::response_at_freqs / apply_response ----

fn ones(len: usize) -> Vec<ComplexSample> {
    vec![ComplexSample::new(1.0, 0.0); len]
}

fn zeros(len: usize) -> Vec<ComplexSample> {
    vec![ComplexSample::new(0.0, 0.0); len]
}

fn eval_at(f: &SpectralFilter, freq: Sample) -> ComplexSample {
    let mut out = ComplexSample::new(0.0, 0.0);
    f.response_at_freqs(&[freq], std::slice::from_mut(&mut out));
    out
}

#[test]
fn response_at_freqs_matches_filter_impl_for_every_type() {
    let freq = 2.5;
    let p = default_params();

    for ty in FilterType::ALL {
        let f = filter(ty, p);
        let response = eval_at(&f, freq);
        let expected = match ty {
            FilterType::LowPass12 => LowPass12::new(f.gain, f.cutoff_freq, f.q, f.pre_q).at(freq),
            FilterType::LowPass18 => LowPass18::new(f.gain, f.cutoff_freq, f.q, f.pre_q).at(freq),
            FilterType::LowPass24 => LowPass24::new(f.gain, f.cutoff_freq, f.q, f.pre_q).at(freq),
            FilterType::LowShelf12 => LowShelf12::new(f.gain, f.cutoff_freq, f.q, f.pre_q).at(freq),
            FilterType::LowShelf24 => LowShelf24::new(f.gain, f.cutoff_freq, f.q, f.pre_q).at(freq),
            FilterType::HighPass12 => HighPass12::new(f.gain, f.cutoff_freq, f.q, f.pre_q).at(freq),
            FilterType::HighPass18 => HighPass18::new(f.gain, f.cutoff_freq, f.q, f.pre_q).at(freq),
            FilterType::HighPass24 => HighPass24::new(f.gain, f.cutoff_freq, f.q, f.pre_q).at(freq),
            FilterType::HighShelf12 => {
                HighShelf12::new(f.gain, f.cutoff_freq, f.q, f.pre_q).at(freq)
            }
            FilterType::HighShelf24 => {
                HighShelf24::new(f.gain, f.cutoff_freq, f.q, f.pre_q).at(freq)
            }
            FilterType::BandPass6 => BandPass6::new(f.gain, f.cutoff_freq, f.q, f.pre_q).at(freq),
            FilterType::BandPass12 => BandPass12::new(f.gain, f.cutoff_freq, f.q, f.pre_q).at(freq),
            FilterType::BandPass18 => BandPass18::new(f.gain, f.cutoff_freq, f.q, f.pre_q).at(freq),
            FilterType::BandPass24 => BandPass24::new(f.gain, f.cutoff_freq, f.q, f.pre_q).at(freq),
            FilterType::Peaking => Peaking::new(f.gain, f.cutoff_freq, f.q, f.pre_q).at(freq),
            FilterType::Notch => Notch::new(f.gain, f.cutoff_freq, f.q, f.pre_q).at(freq),
        };
        assert_complex_eq(response, expected);
    }
}

#[test]
fn linear_phase_response_is_real_magnitude() {
    let mut p = default_params();
    p.linear_phase = true;
    p.q = q_from_resonance(0.8).resonant;

    let freq = 2.5;
    for ty in FilterType::ALL {
        let mut min_phase = p;
        min_phase.linear_phase = false;
        let complex = eval_at(&filter(ty, min_phase), freq);
        let linear = eval_at(&filter(ty, p), freq);

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
fn apply_response_matches_response_at_freqs() {
    let input = [
        ComplexSample::new(1.0, 0.0),
        ComplexSample::new(0.5, 0.25),
        ComplexSample::new(-1.0, 0.75),
        ComplexSample::new(0.0, 1.0),
        ComplexSample::new(0.25, -0.5),
        ComplexSample::new(-0.5, 0.5),
        ComplexSample::new(0.75, 0.1),
    ];
    let freqs: Vec<Sample> = (1..input.len()).map(|i| i as Sample).collect();

    for ty in FilterType::ALL {
        for linear_phase in [false, true] {
            let mut p = default_params();
            p.linear_phase = linear_phase;
            p.q = q_from_resonance(0.6).resonant;
            p.drive = 3.0;

            let f = filter(ty, p);
            let mut output = zeros(input.len());
            f.apply_response(&input, &mut output);

            let mut responses = zeros(freqs.len());
            f.response_at_freqs(&freqs, &mut responses);

            assert_complex_eq(output[0], ComplexSample::new(0.0, 0.0));
            for i in 1..input.len() {
                assert_complex_eq(output[i], input[i] * responses[i - 1]);
            }
        }
    }
}

#[test]
fn response_at_freqs_zips_to_shortest_slice() {
    let freqs = [1.0, 2.0, 3.0];
    let mut out = [ComplexSample::new(0.0, 0.0); 2];
    let f = filter(FilterType::Notch, default_params());
    f.response_at_freqs(&freqs, &mut out);

    assert_eq!(out.len(), 2);
    assert_complex_eq(out[0], eval_at(&f, 1.0));
    assert_complex_eq(out[1], eval_at(&f, 2.0));
}

#[test]
fn apply_response_in_place_matches_apply_response() {
    let input = [
        ComplexSample::new(1.0, 0.0),
        ComplexSample::new(0.5, 0.25),
        ComplexSample::new(-1.0, 0.75),
        ComplexSample::new(0.0, 1.0),
        ComplexSample::new(0.25, -0.5),
        ComplexSample::new(-0.5, 0.5),
        ComplexSample::new(0.75, 0.1),
    ];

    for ty in FilterType::ALL {
        for linear_phase in [false, true] {
            let mut p = default_params();
            p.linear_phase = linear_phase;
            p.q = q_from_resonance(0.6).resonant;
            p.drive = 3.0;

            let f = filter(ty, p);
            let mut copied = input;
            f.apply_response(&input, &mut copied);

            let mut in_place = input;
            f.apply_response_in_place(&mut in_place);

            for (a, b) in copied.iter().zip(in_place) {
                assert_complex_eq(*a, b);
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
