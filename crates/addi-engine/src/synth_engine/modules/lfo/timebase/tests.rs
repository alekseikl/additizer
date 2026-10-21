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
fn one_cycle_per_unit_at_120_bpm() {
    let cases = [
        (Timebase::Hertz, 1.0),
        (Timebase::Kilohertz, 1_000.0),
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
            "Hertz",
            "Kilohertz",
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
fn musical_rate_is_a_period_and_absolute_rate_is_a_frequency() {
    // A quarter note at 120 bpm is 2 Hz for one cycle.
    assert_hz(Timebase::Quarter, 1.0, 120.0, 2.0);
    assert_hz(Timebase::Quarter, 2.5, 120.0, 0.8);
    assert_hz(Timebase::Quarter, 0.5, 120.0, 4.0);
    assert_hz(Timebase::Quarter, -0.5, 120.0, 200.0);
    assert_hz(Timebase::Bar, 4.0, 120.0, 0.125);
    // A quarter at 120 bpm is 2 Hz, so the 0.01 floor is 200 Hz.
    assert_hz(Timebase::Quarter, 0.0, 120.0, 200.0);
    assert_hz(Timebase::Quarter, 0.005, 120.0, 200.0);
    assert_hz(Timebase::Quarter, -0.005, 120.0, 200.0);
    assert_hz(Timebase::Quarter, 0.01, 120.0, 200.0);

    assert_hz(Timebase::Hertz, 2.5, 90.0, 2.5);
    assert_hz(Timebase::Kilohertz, -0.25, 90.0, -250.0);
    assert_hz(Timebase::Hold, -40.0, 120.0, 0.0);
}

#[test]
fn tempo_scales_musical_units_only() {
    assert_hz(Timebase::Eighth, 1.0, 60.0, 2.0);
    assert_hz(Timebase::Bar, 1.0, 60.0, 0.25);
    assert_hz(Timebase::Quarter, 1.0, 90.0, 1.5);
    assert_hz(Timebase::Hertz, 3.0, 60.0, 3.0);
    assert_hz(Timebase::Kilohertz, 0.01, 200.0, 10.0);
    assert_hz(Timebase::Hold, 3.0, 200.0, 0.0);
}

#[test]
fn phase_scale_multiplies_the_increment() {
    let tempo = 120.0;
    let phase_scale = 48_000.0;

    for timebase in [Timebase::Hertz, Timebase::Quarter, Timebase::Hold] {
        for rate in [2.0, 0.0, -0.5, 0.02] {
            let unit = phase_inc(timebase, rate, tempo, 1.0);
            let scaled = phase_inc(timebase, rate, tempo, phase_scale);
            let scale = scaled.abs().max(1.0);

            assert!(
                (scaled - unit * phase_scale).abs() / scale < 1e-5,
                "{timebase:?} rate {rate}: {scaled} != {}",
                unit * phase_scale
            );
        }
    }
}

#[test]
fn dotted_is_slower_and_triplets_are_faster() {
    let tempo = 96.0;

    for (plain, dotted, triplet) in [
        (Timebase::Half, Timebase::DottedHalf, Timebase::TripletHalf),
        (
            Timebase::Quarter,
            Timebase::DottedQuarter,
            Timebase::TripletQuarter,
        ),
        (
            Timebase::Eighth,
            Timebase::DottedEighth,
            Timebase::TripletEighth,
        ),
        (
            Timebase::Sixteenth,
            Timebase::DottedSixteenth,
            Timebase::TripletSixteenth,
        ),
        (
            Timebase::ThirtySecond,
            Timebase::DottedThirtySecond,
            Timebase::TripletThirtySecond,
        ),
    ] {
        let plain_hz = plain.hz_per_unit(tempo);

        assert_hz(dotted, 1.0, tempo, plain_hz * 2.0 / 3.0);
        assert_hz(triplet, 1.0, tempo, plain_hz * 1.5);
    }
}
