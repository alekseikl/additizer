use crate::{SIMD_LANES, WideSmp};

fn successive(start: f32) -> WideSmp {
    WideSmp::new(std::array::from_fn(|lane| start + lane as f32))
}

use super::{fill, map, map_in_place, zip_map, zip_map_in_place};

#[test]
fn map_chunks_and_zeroes_remainder() {
    let mut out = [0.0; 5];
    map(&[1.0, 2.0, 3.0, 4.0, 5.0], &mut out, |lanes| {
        lanes + WideSmp::splat(10.0)
    });
    assert_eq!(out, [11.0, 12.0, 13.0, 14.0, 15.0]);

    let mut out = [0.0; 2];
    map(&[1.0, 2.0], &mut out, |lanes| {
        WideSmp::splat(lanes.reduce_add())
    });
    assert_eq!(out, [3.0, 3.0]);

    let mut out = [];
    map(&[], &mut out, |lanes| lanes);
}

#[test]
fn zip_map_pairs_lanes_and_zeroes_remainder() {
    let mut out = [0.0; 5];
    zip_map(
        &[1.0, 2.0, 3.0, 4.0, 5.0],
        &[10.0, 20.0, 30.0, 40.0, 50.0],
        &mut out,
        |a, b| a + b,
    );
    assert_eq!(out, [11.0, 22.0, 33.0, 44.0, 55.0]);

    let mut out = [0.0; 2];
    zip_map(&[1.0, 2.0], &[3.0, 4.0], &mut out, |a, b| {
        WideSmp::splat((a + b).reduce_add())
    });
    assert_eq!(out, [10.0, 10.0]);

    let mut out = [];
    zip_map(&[], &[], &mut out, |a, _b| a);
}

#[test]
fn fill_writes_groups_and_partial_tail() {
    let mut out = [0.0; 5];
    let mut n = 0.0;
    fill(&mut out, || {
        let lanes = successive(n);
        n += SIMD_LANES as f32;
        lanes
    });
    assert_eq!(out, [0.0, 1.0, 2.0, 3.0, 4.0]);
    assert_eq!(n, out.len().div_ceil(SIMD_LANES) as f32 * SIMD_LANES as f32);

    let mut out = [0.0; 2];
    n = 0.0;
    fill(&mut out, || {
        let lanes = successive(n);
        n += SIMD_LANES as f32;
        lanes
    });
    assert_eq!(out, [0.0, 1.0]);
    assert_eq!(n, SIMD_LANES as f32);

    let mut out = [];
    fill(&mut out, || panic!("empty output"));
}

#[test]
fn map_in_place_matches_map() {
    let samples = [1.0, 2.0, 3.0, 4.0, 5.0];
    let mut separate = [0.0; 5];
    let mut inplace = samples;

    let apply = |lanes: WideSmp| lanes + WideSmp::splat(10.0);

    map(&samples, &mut separate, apply);
    map_in_place(&mut inplace, apply);

    assert_eq!(inplace, separate);
}

#[test]
fn zip_map_in_place_matches_zip_map() {
    let a = [1.0, 2.0, 3.0, 4.0, 5.0];
    let b = [10.0, 20.0, 30.0, 40.0, 50.0];
    let mut separate = [0.0; 5];
    let mut inplace = a;

    let apply = |a: WideSmp, b: WideSmp| a + b;

    zip_map(&a, &b, &mut separate, apply);
    zip_map_in_place(&mut inplace, &b, apply);

    assert_eq!(inplace, separate);
}
