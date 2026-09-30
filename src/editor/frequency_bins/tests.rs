use super::*;

#[test]
fn bins_span_the_width_on_a_log2_scale() {
    let columns = 9;

    for last in [4_usize, 16, 255] {
        let log2_last = (last as f32).log2();

        assert_eq!(FrequencyBins::column_for_harmonic(1, log2_last, columns), 0);
        assert_eq!(
            FrequencyBins::column_for_harmonic(last, log2_last, columns),
            columns - 1
        );
    }

    let log2_16 = 16_f32.log2();
    assert_eq!(FrequencyBins::column_for_harmonic(2, log2_16, columns), 2);
    assert_eq!(FrequencyBins::column_for_harmonic(4, log2_16, columns), 4);
}

#[test]
fn unity_amplitude_sits_at_zero_db() {
    let harmonic = 3;
    let bin = ComplexSample::from_polar(1.0 / (harmonic as f32 * PI), 0.4);

    let level = FrequencyBins::level_from_bin(harmonic, bin);

    assert!((level - ZERO_DB_LEVEL).abs() < 1e-5, "level {level}");
}

#[test]
fn levels_clamp_to_the_harmonic_editor_db_range() {
    assert_eq!(FrequencyBins::level_from_bin(1, ComplexSample::ZERO), 0.0);
    assert_eq!(
        FrequencyBins::level_from_bin(2, ComplexSample::from_polar(1.0e6, 0.0)),
        1.0
    );
}

#[test]
fn silent_spectrum_uses_placeholder_bars_at_zero_db() {
    let mut bins = FrequencyBins {
        columns: vec![Column::default(); 4],
    };

    assert!(!bins.accumulate_columns(&[]));
    assert!(!bins.accumulate_columns(&[ComplexSample::ZERO; 8]));

    let harmonic = 2;
    let audible = ComplexSample::from_polar(1.0 / (harmonic as f32 * PI), 0.0);
    assert!(bins.accumulate_columns(&[ComplexSample::ZERO, ComplexSample::ZERO, audible],));

    let columns = 9;
    let log2_last = (PLACEHOLDER_BARS as f32).log2();

    assert_eq!(PLACEHOLDER_COLOR, Color32::from_rgb(36, 38, 50));
    assert_eq!(
        FrequencyBins::column_for_harmonic(DC_OFFSET, log2_last, columns),
        0
    );
    assert_eq!(
        FrequencyBins::column_for_harmonic(PLACEHOLDER_BARS, log2_last, columns),
        columns - 1
    );
}

#[test]
fn shared_column_keeps_the_louder_bin() {
    let column_count = 4;
    let last = 8;
    let log2_last = (last as f32).log2();
    let quiet_h = 3;
    let loud_h = 4;
    let col = FrequencyBins::column_for_harmonic(quiet_h, log2_last, column_count);

    assert_eq!(
        col,
        FrequencyBins::column_for_harmonic(loud_h, log2_last, column_count)
    );

    let mut bins = FrequencyBins {
        columns: vec![Column::default(); 4],
    };
    let mut spectrum = [ComplexSample::ZERO; 9];
    let quiet = ComplexSample::from_polar(1.0 / (quiet_h as f32 * PI), 0.0);
    let loud = ComplexSample::from_polar(10.0 / (loud_h as f32 * PI), 0.2);
    spectrum[quiet_h] = quiet;
    spectrum[loud_h] = loud;

    bins.accumulate_columns(&spectrum);

    assert!((bins.columns[col].level - FrequencyBins::level_from_bin(loud_h, loud)).abs() < 1e-6);
    assert!((bins.columns[col].hue - FrequencyBins::attenuated_hue(loud_h, loud)).abs() < 1e-5);
}

#[test]
fn sawtooth_phase_keeps_the_attenuated_hue() {
    let harmonic = 3;
    let phase = crate::synth_engine::harmonic_editor::sawtooth_phase(harmonic);
    let bin = ComplexSample::from_polar(1.0, phase * std::f32::consts::TAU);

    let hue = FrequencyBins::attenuated_hue(harmonic, bin);

    assert!((hue - ATTENUATED_COLOR.h).abs() < 1e-5, "hue {hue}");
}

#[test]
fn close_columns_slope_toward_the_next() {
    let current = 0.40;
    let next = 0.44;
    let plot_height = 40.0;

    assert!(FrequencyBins::within_slope_height(
        current,
        next,
        plot_height
    ));
    assert!(!FrequencyBins::within_slope_height(
        current,
        0.0,
        plot_height
    ));
    assert!(!FrequencyBins::within_slope_height(
        current,
        current,
        plot_height
    ));
    assert!(!FrequencyBins::within_slope_height(
        current,
        0.50,
        plot_height
    ));

    let bottom = 100.0;
    let left_y = bottom - current * plot_height;
    let right_y = bottom - next * plot_height;
    let zero_y = (left_y + right_y) * 0.5;

    assert!((FrequencyBins::slope_cross_t(left_y, right_y, zero_y) - 0.5).abs() < 1e-5);
}

#[test]
fn feather_is_centered_on_the_top_edge() {
    let top = 40.0;
    let floor = 100.0;
    let feather = 0.5;

    let (outer, inner) = FrequencyBins::feather_ys(top, floor, feather);

    assert!((outer - (top - feather * 0.5)).abs() < 1e-6);
    assert!((inner - (top + feather * 0.5)).abs() < 1e-6);

    let (outer, inner) = FrequencyBins::feather_ys(floor - 0.1, floor, feather);

    assert!((inner - floor).abs() < 1e-6);
    assert!(outer < floor - 0.1);
}

#[test]
fn hue_turns_with_the_phase_offset_from_sawtooth() {
    let harmonic = 4;
    let turn = 0.3;
    let phase = crate::synth_engine::harmonic_editor::sawtooth_phase(harmonic) + turn;
    let bin = ComplexSample::from_polar(1.0, phase * std::f32::consts::TAU);

    let hue = FrequencyBins::attenuated_hue(harmonic, bin);
    let expected = (ATTENUATED_COLOR.h + turn).rem_euclid(1.0);

    assert!(
        (hue - expected).abs() < 1e-5,
        "hue {hue} expected {expected}"
    );
}
