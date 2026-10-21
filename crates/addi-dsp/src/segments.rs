use serde::{Deserialize, Serialize};

use crate::{
    SIMD_LANES, Smp, WideSmp,
    fast_math::{fast_exp2_wide, fast_sin_pi_wide},
    simd_utils::fill_wide,
    utils::sanitize,
};

pub const MAX_POINTS: usize = 100;
pub const MAX_POWER: Smp = 20.0;

const MIN_POINTS: usize = 2;

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Segment {
    pub x: Smp,
    pub y: Smp,
    pub power: Smp,
    pub smooth: bool,
}

impl Default for Segment {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            power: 0.0,
            smooth: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Segments {
    segments: Vec<Segment>,
}

impl Default for Segments {
    fn default() -> Self {
        Self::from_points(&[(0.0, 0.0), (0.5, 1.0), (1.0, 0.0)], false)
    }
}

impl Segments {
    /// Build a curve from phase/level pairs. Every segment shares the same `smooth` flag.
    pub fn from_points(points: &[(Smp, Smp)], smooth: bool) -> Self {
        debug_assert!(points.len() >= MIN_POINTS && points.len() <= MAX_POINTS);

        let mut curve = Vec::with_capacity(MAX_POINTS);

        for &(x, y) in points {
            curve.push(Segment {
                x,
                y,
                smooth,
                ..Segment::default()
            });
        }

        Self { segments: curve }
    }

    pub fn num_segments(&self) -> usize {
        self.segments.len()
    }

    pub fn segment(&self, index: usize) -> Segment {
        self.segments[index]
    }

    pub fn segments(&self) -> &[Segment] {
        &self.segments
    }

    pub fn power(&self, index: usize) -> Smp {
        self.segments[index].power
    }

    pub fn set_segment(&mut self, index: usize, mut segment: Segment) {
        let n = self.num_segments();

        if index >= n {
            return;
        }

        segment.x = sanitize(segment.x, 0.0, 1.0);
        segment.y = sanitize(segment.y, 0.0, 1.0);
        segment.power = sanitize(segment.power, -MAX_POWER, MAX_POWER);

        if index == 0 {
            segment.x = 0.0;
        } else if index + 1 == n {
            segment.x = 1.0;
        }

        self.segments[index] = segment;
    }

    pub fn set_power(&mut self, index: usize, power: Smp) {
        if index >= self.num_segments() {
            return;
        }

        self.segments[index].power = sanitize(power, -MAX_POWER, MAX_POWER);
    }

    pub fn set_smooth(&mut self, index: usize, smooth: bool) {
        if index >= self.num_segments() {
            return;
        }

        self.segments[index].smooth = smooth;
    }

    pub fn reset_powers(&mut self) {
        for point in &mut self.segments {
            point.power = 0.0;
        }
    }

    /// Clamp every breakpoint into range. Fewer than two points becomes the default curve.
    pub fn sanitize(&mut self) {
        self.segments.truncate(MAX_POINTS);

        if self.segments.len() < MIN_POINTS {
            *self = Self::default();
            return;
        }

        for segment in &mut self.segments {
            segment.x = sanitize(segment.x, 0.0, 1.0);
            segment.y = sanitize(segment.y, 0.0, 1.0);
            segment.power = sanitize(segment.power, -MAX_POWER, MAX_POWER);
        }

        let n = self.segments.len();
        self.segments[0].x = 0.0;
        self.segments[n - 1].x = 1.0;
    }

    /// Insert `segment` so it stays in phase order. Fails when the curve is full.
    pub fn add_segment(&mut self, index: usize, mut segment: Segment) -> bool {
        let n = self.num_segments();

        if n >= MAX_POINTS || index > n {
            return false;
        }

        segment.x = sanitize(segment.x, 0.0, 1.0);
        segment.y = sanitize(segment.y, 0.0, 1.0);

        segment.power = sanitize(segment.power, -MAX_POWER, MAX_POWER);

        if index == 0 {
            segment.x = 0.0;
        }

        if index == n {
            segment.x = 1.0;
        }

        self.segments.insert(index, segment);
        true
    }

    /// Drop an interior breakpoint. The phase `0` and phase `1` endpoints stay.
    pub fn remove_segment(&mut self, index: usize) -> bool {
        let n = self.num_segments();

        if n <= MIN_POINTS || index == 0 || index + 1 == n {
            return false;
        }

        self.segments.remove(index);
        true
    }

    pub fn flip_horizontal(&mut self) {
        let n = self.num_segments();

        for i in 0..n.div_ceil(2) {
            let tmp_x = 1.0 - self.segments[i].x;
            let tmp_y = self.segments[i].y;
            let j = n - i - 1;

            self.segments[i].x = 1.0 - self.segments[j].x;
            self.segments[i].y = self.segments[j].y;
            self.segments[j].x = tmp_x;
            self.segments[j].y = tmp_y;
        }

        for i in 0..n / 2 {
            let j = n - i - 2;
            let power = self.segments[i].power;
            let smooth = self.segments[i].smooth;

            self.segments[i].power = -self.segments[j].power;
            self.segments[i].smooth = self.segments[j].smooth;
            self.segments[j].power = -power;
            self.segments[j].smooth = smooth;
        }
    }

    pub fn flip_vertical(&mut self) {
        for point in &mut self.segments {
            point.y = 1.0 - point.y;
        }
    }

    /// Write one cycle into `out`. Fewer than two samples are left unchanged.
    ///
    /// `closed` is false for phases `i / out.len()` on `[0, 1)`.
    /// `closed` is true for phases `i / (out.len() - 1)` on `[0, 1]`.
    pub fn render(&self, out: &mut [Smp], closed: bool) {
        let len = out.len();

        if len < 2 {
            return;
        }

        let n = self.num_segments();

        if n < MIN_POINTS {
            return;
        }

        debug_assert_eq!(self.segments[0].x, 0.0);
        debug_assert_eq!(self.segments[n - 1].x, 1.0);

        let denom = (len - usize::from(closed)) as Smp;
        let mut from = 0;

        for pair in self.segments.windows(2) {
            // The sample on this breakpoint belongs to the segment that ends here.
            let last = (pair[1].x * denom)
                .floor()
                .clamp(0.0, (len - 1) as Smp) as usize;
            let to = last + 1;

            if from >= to {
                continue;
            }

            render_segment(&mut out[from..to], from, denom, pair[0], pair[1]);
            from = to;
        }

        debug_assert_eq!(from, len);
    }
}

fn render_segment(out: &mut [Smp], start_index: usize, denom: Smp, start: Segment, end: Segment) {
    let width = end.x - start.x;

    if width <= 0.0 {
        out.fill(end.y);
        return;
    }

    let start_x = WideSmp::splat(start.x);
    let span_scale = WideSmp::splat(width.recip());
    let origin = WideSmp::splat(start.y);
    let delta = WideSmp::splat(end.y - start.y);
    let smooth = start.smooth;
    let power = PowerLanes::new(start.power);
    let phase_scale = WideSmp::splat(denom.recip());
    let step = phase_scale * WideSmp::splat(SIMD_LANES as Smp);
    let mut phase =
        (WideSmp::splat(start_index as Smp) + WideSmp::new([0.0, 1.0, 2.0, 3.0])) * phase_scale;

    fill_wide(out, || {
        let t = ((phase - start_x) * span_scale)
            .fast_max(WideSmp::ZERO)
            .fast_min(WideSmp::ONE);

        let t = if smooth {
            fast_sin_pi_wide(t - WideSmp::HALF).mul_add(WideSmp::HALF, WideSmp::HALF)
        } else {
            t
        };

        phase += step;

        power.scale(t).mul_add(delta, origin)
    });
}

/// [`power_scale`](crate::power_scale) for each lane.
///
/// `scale` is `1 / (exp(power) - 1)`, computed once and applied with a multiply.
struct PowerLanes {
    curved: bool,
    power_log2e: WideSmp,
    scale: WideSmp,
}

impl PowerLanes {
    fn new(power: Smp) -> Self {
        // Same cutoff as `power_scale`: a flat slope stays linear.
        if power.abs() < 0.005 {
            return Self {
                curved: false,
                power_log2e: WideSmp::ZERO,
                scale: WideSmp::ONE,
            };
        }

        let power_log2e = WideSmp::splat(power * std::f32::consts::LOG2_E);
        let denom = (fast_exp2_wide(power_log2e) - WideSmp::ONE).to_array()[0];
        let scale = WideSmp::splat(denom.recip());

        Self {
            curved: true,
            power_log2e,
            scale,
        }
    }

    fn scale(&self, t: WideSmp) -> WideSmp {
        if !self.curved {
            return t.fast_max(WideSmp::ZERO).fast_min(WideSmp::ONE);
        }

        let exp = fast_exp2_wide(t * self.power_log2e);

        ((exp - WideSmp::ONE) * self.scale)
            .fast_max(WideSmp::ZERO)
            .fast_min(WideSmp::ONE)
    }
}

#[cfg(test)]
mod tests;
