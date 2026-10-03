use super::*;

/// Column whose harmonic range contains `harmonic`.
fn column_of(harmonic: usize, last_harmonic: usize, columns: usize) -> usize {
    column_ranges(last_harmonic, columns)
        .position(|range| range.contains(&harmonic))
        .unwrap_or_else(|| panic!("harmonic {harmonic} has no column"))
}

#[test]
fn column_ranges_tile_the_harmonics() {
    for (last, columns) in [(1_usize, 2_usize), (4, 9), (16, 9), (255, 9), (8, 300)] {
        let ranges: Vec<_> = column_ranges(last, columns).collect();

        assert_eq!(ranges.len(), columns);
        assert_eq!(ranges[0].start, DC_OFFSET);
        assert_eq!(ranges[columns - 1].end, last + 1);

        for pair in ranges.windows(2) {
            assert_eq!(pair[0].end, pair[1].start, "last {last} columns {columns}");
        }
    }
}

#[test]
fn bins_span_the_width_on_a_log2_scale() {
    let columns = 9;

    for last in [4_usize, 16, 255] {
        assert_eq!(column_of(1, last, columns), 0);
        assert_eq!(column_of(last, last, columns), columns - 1);
    }

    assert_eq!(column_of(2, 16, columns), 2);
    assert_eq!(column_of(4, 16, columns), 4);
}

#[test]
fn single_harmonic_fills_the_first_column() {
    assert_eq!(column_of(1, 1, 4), 0);

    let mut bins = FrequencyBins::default();
    let audible = ComplexSample::from_polar(1.0 / PI, 0.0);

    assert!(bins.accumulate_columns(&[ComplexSample::ZERO, audible], 4));
    assert!((bins.columns[0].level - ZERO_DB_LEVEL).abs() < 1e-5);
}

#[test]
fn unity_amplitude_sits_at_zero_db() {
    let harmonic = 3;
    let bin = ComplexSample::from_polar(1.0 / (harmonic as f32 * PI), 0.4);

    let level = level_from_bin(harmonic, bin);

    assert!((level - ZERO_DB_LEVEL).abs() < 1e-5, "level {level}");
}

#[test]
fn levels_clamp_to_the_harmonic_editor_db_range() {
    assert_eq!(level_from_bin(1, ComplexSample::ZERO), 0.0);
    assert_eq!(
        level_from_bin(2, ComplexSample::from_polar(1.0e6, 0.0)),
        1.0
    );
}

#[test]
fn silent_spectrum_uses_placeholder_bars_at_zero_db() {
    let mut bins = FrequencyBins::default();

    assert!(!bins.accumulate_columns(&[], 4));
    assert!(!bins.accumulate_columns(&[ComplexSample::ZERO; 8], 4));

    let harmonic = 2;
    let audible = ComplexSample::from_polar(1.0 / (harmonic as f32 * PI), 0.0);
    assert!(bins.accumulate_columns(&[ComplexSample::ZERO, ComplexSample::ZERO, audible], 4));

    let columns = 9;

    assert_eq!(PLACEHOLDER_COLOR, Color32::from_rgb(36, 38, 50));
    assert_eq!(column_of(DC_OFFSET, PLACEHOLDER_BARS, columns), 0);
    assert_eq!(
        column_of(PLACEHOLDER_BARS, PLACEHOLDER_BARS, columns),
        columns - 1
    );
}

#[test]
fn shared_column_keeps_the_louder_bin() {
    let column_count = 4;
    let last = 8;
    let quiet_h = 3;
    let loud_h = 4;
    let col = column_of(quiet_h, last, column_count);

    assert_eq!(col, column_of(loud_h, last, column_count));

    let mut spectrum = [ComplexSample::ZERO; 9];
    let quiet = ComplexSample::from_polar(1.0 / (quiet_h as f32 * PI), 0.0);
    let loud = ComplexSample::from_polar(10.0 / (loud_h as f32 * PI), 0.2);
    spectrum[quiet_h] = quiet;
    spectrum[loud_h] = loud;

    assert_eq!(loudest_harmonic(&spectrum, DC_OFFSET..9), Some(loud_h));
    assert_eq!(loudest_harmonic(&spectrum, 5..9), None);

    let mut bins = FrequencyBins::default();
    bins.accumulate_columns(&spectrum, column_count);

    assert!((bins.columns[col].level - level_from_bin(loud_h, loud)).abs() < 1e-6);
    assert!((bins.columns[col].phase - phase_turns(loud_h, loud)).abs() < 1e-5);
}

#[test]
fn sawtooth_phase_uses_the_zero_colors() {
    let harmonic = 3;
    let phase = sawtooth_phase(harmonic);
    let bin = ComplexSample::from_polar(1.0, phase * TAU);

    let attenuated = attenuated_color(phase_turns(harmonic, bin));

    assert_eq!(attenuated, Color32::from(PHASE_0_COLOR));
}

#[test]
fn phase_color_hits_green_red_and_yellow() {
    let harmonic = 4;
    let cases = [
        (0.25, PHASE_90_COLOR),
        (0.5, PHASE_180_COLOR),
        (0.75, PHASE_270_COLOR),
    ];

    for (turn, expected) in cases {
        let phase = sawtooth_phase(harmonic) + turn;
        let bin = ComplexSample::from_polar(1.0, phase * TAU);
        let attenuated = attenuated_color(phase_turns(harmonic, bin));

        assert_eq!(attenuated, Color32::from(expected), "turn {turn}");
    }
}

#[test]
fn phase_color_interpolates_between_stops() {
    let mid = attenuated_color(0.125);
    let expected = Color32::from(PHASE_0_COLOR).lerp_to_gamma(PHASE_90_COLOR.into(), 0.5);

    assert_eq!(mid, expected);

    let mid = attenuated_color(0.625);
    let expected = Color32::from(PHASE_180_COLOR).lerp_to_gamma(PHASE_270_COLOR.into(), 0.5);

    assert_eq!(mid, expected);

    let mid = attenuated_color(0.875);
    let expected = Color32::from(PHASE_270_COLOR).lerp_to_gamma(PHASE_0_COLOR.into(), 0.5);

    assert_eq!(mid, expected);
}

#[test]
fn close_columns_slope_toward_the_next() {
    let current = 0.40;
    let next = 0.44;
    let plot_height = 40.0;

    assert!(within_slope_height(current, next, plot_height));
    assert!(within_slope_height(current, current, plot_height));
    assert!(!within_slope_height(current, 0.0, plot_height));
    assert!(!within_slope_height(current, 0.50, plot_height));

    let bottom = 100.0;
    let left_y = bottom - current * plot_height;
    let right_y = bottom - next * plot_height;
    let zero_y = (left_y + right_y) * 0.5;

    assert!((slope_cross_t(left_y, right_y, zero_y) - 0.5).abs() < 1e-5);
}

#[test]
fn feather_is_centered_on_the_top_edge() {
    let top = 40.0;
    let floor = 100.0;
    let feather = 0.5;

    let (outer, inner) = feather_ys(top, floor, feather);

    assert!((outer - (top - feather * 0.5)).abs() < 1e-6);
    assert!((inner - (top + feather * 0.5)).abs() < 1e-6);

    let (outer, inner) = feather_ys(floor - 0.1, floor, feather);

    assert!((inner - floor).abs() < 1e-6);
    assert!(outer < floor - 0.1);
}

#[test]
fn bar_crossing_zero_db_splits_into_two_colors() {
    let rect = Rect::from_min_size(Pos2::ZERO, egui::vec2(10.0, 100.0));
    let mut bars = BarMesh::new(rect, 0.0);
    let attenuated = attenuated_color(0.0);
    let below = ZERO_DB_LEVEL - 0.01;
    let above = ZERO_DB_LEVEL + 0.01;

    bars.column(0.0, below, below, attenuated);
    let attenuated_only = bars.mesh.vertices.len();
    assert_eq!(attenuated_only, 4);

    bars.column(1.0, above, above, attenuated);
    let amplified_with_base = bars.mesh.vertices.len() - attenuated_only;
    assert_eq!(amplified_with_base, 8);

    bars.column(2.0, below, above, attenuated);
    let crossing = bars.mesh.vertices.len() - attenuated_only - amplified_with_base;
    assert_eq!(crossing, 12);

    let amplified: Color32 = AMPLIFIED_COLOR.into();
    assert!(bars.mesh.vertices.iter().any(|v| v.color == amplified));
    assert!(bars.mesh.vertices.iter().any(|v| v.color == attenuated));
    assert!(bars.mesh.vertices.iter().all(|v| rect.contains(v.pos)));
}
