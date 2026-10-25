use super::*;

fn phase_inc(timebase: Timebase, rate: Smp, tempo: Smp, phase_scale: Smp) -> Smp {
    let mut rates = [rate];

    timebase.map_phase_inc(&mut rates, tempo, phase_scale);
    rates[0]
}

fn assert_hz(timebase: Timebase, rate: Smp, tempo: Smp, expected: Smp) {
    let got = phase_inc(timebase, rate, tempo, 1.0);

    assert!(
        (got - expected).abs() < 1e-4,
        "{timebase:?} rate {rate} @ {tempo}: {got} != {expected}"
    );
}

#[test]
fn one_pass_per_unit_at_120_bpm() {
    let cases = [
        (Timebase::Seconds, 1.0),
        (Timebase::Bar, 0.5),
        (Timebase::Half, 1.0),
        (Timebase::Quarter, 2.0),
        (Timebase::Eighth, 4.0),
        (Timebase::Sixteenth, 8.0),
        (Timebase::ThirtySecond, 16.0),
        (Timebase::DottedHalf, 2.0 / 3.0),
        (Timebase::DottedQuarter, 4.0 / 3.0),
        (Timebase::DottedEighth, 8.0 / 3.0),
        (Timebase::DottedSixteenth, 16.0 / 3.0),
        (Timebase::DottedThirtySecond, 32.0 / 3.0),
        (Timebase::TripletHalf, 1.5),
        (Timebase::TripletQuarter, 3.0),
        (Timebase::TripletEighth, 6.0),
        (Timebase::TripletSixteenth, 12.0),
        (Timebase::TripletThirtySecond, 24.0),
        (Timebase::Hold, 0.0),
    ];

    assert_eq!(cases.len(), Timebase::ALL.len());

    for (timebase, hz) in cases {
        assert_hz(timebase, 1.0, 120.0, hz);
    }
}

#[test]
fn labels_follow_the_menu() {
    let labels: Vec<_> = Timebase::ALL
        .iter()
        .map(|timebase| timebase.label())
        .collect();

    assert_eq!(
        labels,
        [
            "Seconds",
            "Bar",
            "Half note",
            "Quarter note",
            "8th note",
            "16th note",
            "32nd note",
            "Dotted half note",
            "Dotted quarter note",
            "Dotted 8th note",
            "Dotted 16th note",
            "Dotted 32nd note",
            "Triplet half note",
            "Triplet quarter note",
            "Triplet 8th note",
            "Triplet 16th note",
            "Triplet 32nd note",
            "Hold",
        ]
    );
}

#[test]
fn seconds_is_a_period_and_ignores_tempo() {
    assert_hz(Timebase::Seconds, 1.0, 120.0, 1.0);
    assert_hz(Timebase::Seconds, 2.0, 90.0, 0.5);
    assert_hz(Timebase::Seconds, 0.5, 60.0, 2.0);
    // Shorter than the 0.01 floor, so the pass is 100 Hz.
    assert_hz(Timebase::Seconds, 0.0, 120.0, 100.0);
    assert_hz(Timebase::Seconds, -2.0, 120.0, 100.0);
    assert_hz(Timebase::Hold, 4.0, 200.0, 0.0);
    assert_hz(Timebase::Quarter, 1.0, 90.0, 1.5);
}

#[test]
fn one_second_advances_once_per_sample_rate() {
    let sample_rate: Smp = 48_000.0;
    let mut once = [1.0];
    let mut twice = [2.0];

    Timebase::Seconds.map_phase_inc(&mut once, 140.0, sample_rate.recip());
    Timebase::Seconds.map_phase_inc(&mut twice, 140.0, sample_rate.recip());

    assert!((once[0] * sample_rate - 1.0).abs() < 1e-3);
    assert!((twice[0] * sample_rate - 0.5).abs() < 1e-3);
}
