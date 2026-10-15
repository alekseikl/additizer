use wide::f32x4;

use super::{fill_x4, map_x4, map_x4_in_place, zip_map_x4};

#[test]
fn map_x4_chunks_and_zeroes_remainder() {
    let mut out = [0.0; 5];
    map_x4(&[1.0, 2.0, 3.0, 4.0, 5.0], &mut out, |lanes| {
        lanes + f32x4::splat(10.0)
    });
    assert_eq!(out, [11.0, 12.0, 13.0, 14.0, 15.0]);

    let mut out = [0.0; 2];
    map_x4(&[1.0, 2.0], &mut out, |lanes| {
        f32x4::splat(lanes.reduce_add())
    });
    assert_eq!(out, [3.0, 3.0]);

    let mut out = [];
    map_x4(&[], &mut out, |lanes| lanes);
}

#[test]
fn zip_map_x4_pairs_lanes_and_zeroes_remainder() {
    let mut out = [0.0; 5];
    zip_map_x4(
        &[1.0, 2.0, 3.0, 4.0, 5.0],
        &[10.0, 20.0, 30.0, 40.0, 50.0],
        &mut out,
        |a, b| a + b,
    );
    assert_eq!(out, [11.0, 22.0, 33.0, 44.0, 55.0]);

    let mut out = [0.0; 2];
    zip_map_x4(&[1.0, 2.0], &[3.0, 4.0], &mut out, |a, b| {
        f32x4::splat((a + b).reduce_add())
    });
    assert_eq!(out, [10.0, 10.0]);

    let mut out = [];
    zip_map_x4(&[], &[], &mut out, |a, _b| a);
}

#[test]
fn fill_x4_writes_groups_and_partial_tail() {
    let mut out = [0.0; 5];
    let mut n = 0.0;
    fill_x4(&mut out, || {
        let lanes = f32x4::new([n, n + 1.0, n + 2.0, n + 3.0]);
        n += 4.0;
        lanes
    });
    assert_eq!(out, [0.0, 1.0, 2.0, 3.0, 4.0]);
    assert_eq!(n, 8.0);

    let mut out = [0.0; 2];
    n = 0.0;
    fill_x4(&mut out, || {
        let lanes = f32x4::new([n, n + 1.0, n + 2.0, n + 3.0]);
        n += 4.0;
        lanes
    });
    assert_eq!(out, [0.0, 1.0]);
    assert_eq!(n, 4.0);

    let mut out = [];
    fill_x4(&mut out, || panic!("empty output"));
}

#[test]
fn map_x4_in_place_matches_map_x4() {
    let samples = [1.0, 2.0, 3.0, 4.0, 5.0];
    let mut separate = [0.0; 5];
    let mut inplace = samples;

    let map = |lanes: f32x4| lanes + f32x4::splat(10.0);

    map_x4(&samples, &mut separate, map);
    map_x4_in_place(&mut inplace, map);

    assert_eq!(inplace, separate);
}
