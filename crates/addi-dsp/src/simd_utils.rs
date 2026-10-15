//! Lane-wise walks over sample buffers.

use wide::f32x4;

use crate::Smp;

#[cfg(test)]
mod tests;

/// Width of an `f32x4` vector.
pub const SIMD_LANES: usize = 4;

/// Map `samples` through `map` in groups of four and write the results to `out`.
///
/// `samples` and `out` must have the same length. A trailing partial group is
/// zero-padded. Only the lanes that correspond to real input samples are stored.
#[inline(always)]
pub fn map_x4(samples: &[Smp], out: &mut [Smp], mut map: impl FnMut(f32x4) -> f32x4) {
    assert!(samples.len() == out.len());

    let (in_chunks, in_rem) = samples.as_chunks::<SIMD_LANES>();
    let (out_chunks, out_rem) = out.as_chunks_mut::<SIMD_LANES>();

    for (out, input) in out_chunks.iter_mut().zip(in_chunks) {
        *out = map(f32x4::new(*input)).to_array();
    }

    let n = in_rem.len();

    if n > 0 {
        let mut lanes = [0.0; SIMD_LANES];
        lanes[..n].copy_from_slice(&in_rem[..n]);
        let mapped = map(f32x4::new(lanes)).to_array();
        out_rem[..n].copy_from_slice(&mapped[..n]);
    }
}

/// [`map_x4`] over two buffers, passing each group to `map` as a pair of lanes.
///
/// `a`, `b`, and `out` must have the same length. A trailing partial group is
/// zero-padded. Only the lanes that correspond to real samples are stored.
#[inline(always)]
pub fn zip_map_x4(
    a: &[Smp],
    b: &[Smp],
    out: &mut [Smp],
    mut map: impl FnMut(f32x4, f32x4) -> f32x4,
) {
    assert!(a.len() == b.len() && a.len() == out.len());

    let (a_chunks, a_rem) = a.as_chunks::<SIMD_LANES>();
    let (b_chunks, b_rem) = b.as_chunks::<SIMD_LANES>();
    let (out_chunks, out_rem) = out.as_chunks_mut::<SIMD_LANES>();

    for ((out, a), b) in out_chunks.iter_mut().zip(a_chunks).zip(b_chunks) {
        *out = map(f32x4::new(*a), f32x4::new(*b)).to_array();
    }

    let n = a_rem.len();

    if n > 0 {
        let mut a_lanes = [0.0; SIMD_LANES];
        let mut b_lanes = [0.0; SIMD_LANES];
        a_lanes[..n].copy_from_slice(&a_rem[..n]);
        b_lanes[..n].copy_from_slice(&b_rem[..n]);
        let mapped = map(f32x4::new(a_lanes), f32x4::new(b_lanes)).to_array();
        out_rem[..n].copy_from_slice(&mapped[..n]);
    }
}

/// Fill `out` with successive groups of four lanes from `fill`.
///
/// `fill` is called once per group. A trailing partial group is produced as
/// four lanes; only the lanes that fit in `out` are stored.
#[inline(always)]
pub fn fill_x4(out: &mut [Smp], mut fill: impl FnMut() -> f32x4) {
    let (chunks, rem) = out.as_chunks_mut::<SIMD_LANES>();

    for chunk in chunks {
        *chunk = fill().to_array();
    }

    let n = rem.len();

    if n > 0 {
        let filled = fill().to_array();
        rem.copy_from_slice(&filled[..n]);
    }
}

/// [`map_x4`] over one buffer, writing each group back in place.
#[inline(always)]
pub fn map_x4_in_place(samples: &mut [Smp], mut map: impl FnMut(f32x4) -> f32x4) {
    let (chunks, rem) = samples.as_chunks_mut::<SIMD_LANES>();

    for chunk in chunks {
        *chunk = map(f32x4::new(*chunk)).to_array();
    }

    let n = rem.len();

    if n > 0 {
        let mut lanes = [0.0; SIMD_LANES];
        lanes[..n].copy_from_slice(rem);
        let mapped = map(f32x4::new(lanes)).to_array();
        rem.copy_from_slice(&mapped[..n]);
    }
}
