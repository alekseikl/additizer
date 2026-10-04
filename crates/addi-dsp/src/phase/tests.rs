use wide::f32x4;

use super::{Phase, PhaseX4, WaveIndexScale};

const BITS: usize = 11;

#[test]
fn load_store_round_trips() {
    let phases = [1u32, 2, 3, u32::MAX].map(Phase::from_bits);
    let mut out = [Phase::ZERO; 4];

    PhaseX4::load(&phases).store(&mut out);

    assert_eq!(out, phases);
}

#[test]
fn from_normalized_matches_scalar_modulo_one_cycle() {
    // Values beyond [-0.5, 0.5] exercise the wrap path that scalar `Phase`
    // gets for free from the `i64 -> u32` truncation.
    let inputs = [
        -1.0f32, -0.75, -0.5, -0.25, 0.0, 0.125, 0.5, 0.75, 1.0, 1.3, -1.7,
    ];

    for chunk in inputs.chunks(4) {
        let mut arr = [0.0; 4];
        arr[..chunk.len()].copy_from_slice(chunk);

        let simd = PhaseX4::from_normalized(f32x4::new(arr));
        let mut out = [Phase::ZERO; 4];
        simd.store(&mut out);

        for (lane, &norm) in arr.iter().enumerate() {
            let expected = Phase::from_normalized(norm).bits();
            let actual = out[lane].bits();
            let diff = expected
                .wrapping_sub(actual)
                .min(actual.wrapping_sub(expected));

            // Truncation direction may differ by one LSB (2^-32 of a cycle)
            // and +0.5 exactly saturates one LSB short of 2^31.
            assert!(diff <= 1, "norm {norm}: expected {expected}, got {actual}");
        }
    }
}

#[test]
fn from_wrapped_matches_scalar_inside_half_cycle() {
    // `from_wrapped` does not modulo. `+0.5` saturates in the `i32` conversion,
    // so it stays on the `from_normalized` path.
    let inputs = [-0.5f32, -0.49, -0.25, 0.0, 0.125, 0.49];

    for chunk in inputs.chunks(4) {
        let mut arr = [0.0; 4];
        arr[..chunk.len()].copy_from_slice(chunk);

        let mut out = [Phase::ZERO; 4];
        PhaseX4::from_wrapped(f32x4::new(arr)).store(&mut out);

        for (lane, &norm) in arr.iter().enumerate() {
            assert_eq!(
                out[lane].bits(),
                Phase::from_normalized(norm).bits(),
                "norm {norm}"
            );
        }
    }
}

#[test]
fn wave_index_and_fraction_match_scalar() {
    let phases = [0u32, 0x7fff_ffff, 0x8000_0000, 0xffff_ffff].map(Phase::from_bits);
    let simd = PhaseX4::load(&phases);

    let idx = simd.wave_index::<BITS>();
    let frac = simd.wave_index_fraction::<BITS>().to_array();

    for lane in 0..4 {
        assert_eq!(idx[lane] as usize, phases[lane].wave_index::<BITS>());
        assert_eq!(frac[lane], phases[lane].wave_index_fraction::<BITS>());
    }
}

#[test]
fn wave_index_with_matches_const_bits() {
    let phases = [0u32, 0x0100_0000, 0x7fff_ffff, 0xffff_ffff].map(Phase::from_bits);
    let simd = PhaseX4::load(&phases);

    for bits in [9u32, 10, 11] {
        let scale = WaveIndexScale::from_bits(bits);
        let idx = simd.wave_index_with(scale);
        let frac = simd.wave_index_fraction_with(scale).to_array();

        let (expected_idx, expected_frac) = match bits {
            9 => (
                simd.wave_index::<9>(),
                simd.wave_index_fraction::<9>().to_array(),
            ),
            10 => (
                simd.wave_index::<10>(),
                simd.wave_index_fraction::<10>().to_array(),
            ),
            11 => (
                simd.wave_index::<11>(),
                simd.wave_index_fraction::<11>().to_array(),
            ),
            _ => unreachable!(),
        };

        assert_eq!(idx, expected_idx);
        assert_eq!(frac, expected_frac);
    }
}

#[test]
fn add_increment_matches_scalar() {
    let phases = [0u32, 1000, 0xffff_ff00, 0x8000_0000].map(Phase::from_bits);
    let incs = [1.5f32, -2.5, 1000.75, 123_456.0];

    let mut out = [Phase::ZERO; 4];
    (PhaseX4::load(&phases) + f32x4::new(incs)).store(&mut out);

    for lane in 0..4 {
        assert_eq!(out[lane], phases[lane] + incs[lane]);
    }
}

#[test]
fn add_phases_wraps() {
    let a = PhaseX4::splat(Phase::from_bits(u32::MAX));
    let b = PhaseX4::splat(Phase::from_bits(2));
    let mut out = [Phase::ZERO; 4];

    (a + b).store(&mut out);

    assert!(out.iter().all(|p| p.bits() == 1));
}
