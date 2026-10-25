use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use crate::{
    SIMD_LANES, Smp, WideSmp,
    fast_math::{fast_exp2_wide, fast_sin_pi_wide},
    power_scale,
    simd_utils::fill_wide,
    utils::sanitize,
};

pub const MAX_POINTS: usize = 100;
pub const MAX_POWER: Smp = 20.0;

const MIN_POINTS: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
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

/// Phase and level on a curve drawn for the UI.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CurvePoint {
    pub x: Smp,
    pub y: Smp,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Segments {
    segments: Vec<Segment>,
    /// Identity of this edit. Replaced on every change; clones keep it until edited.
    #[serde(skip, default = "Segments::next_revision")]
    revision: u64,
}

impl PartialEq for Segments {
    fn eq(&self, other: &Self) -> bool {
        self.segments == other.segments
    }
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

        Self {
            segments: curve,
            revision: Self::next_revision(),
        }
    }

    pub fn num_segments(&self) -> usize {
        self.segments.len()
    }

    /// Changes whenever the curve is edited. Reads leave it alone.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    fn next_revision() -> u64 {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        NEXT.fetch_add(1, Ordering::Relaxed)
    }

    fn bump(&mut self) {
        self.revision = Self::next_revision();
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
        self.bump();
    }

    pub fn set_power(&mut self, index: usize, power: Smp) {
        if index >= self.num_segments() {
            return;
        }

        self.segments[index].power = sanitize(power, -MAX_POWER, MAX_POWER);
        self.bump();
    }

    pub fn set_smooth(&mut self, index: usize, smooth: bool) {
        if index >= self.num_segments() {
            return;
        }

        self.segments[index].smooth = smooth;
        self.bump();
    }

    pub fn reset_powers(&mut self) {
        for point in &mut self.segments {
            point.power = 0.0;
        }

        self.bump();
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
        self.bump();
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
        self.bump();
        true
    }

    /// Drop an interior breakpoint. The phase `0` and phase `1` endpoints stay.
    pub fn remove_segment(&mut self, index: usize) -> bool {
        let n = self.num_segments();

        if n <= MIN_POINTS || index == 0 || index + 1 == n {
            return false;
        }

        self.segments.remove(index);
        self.bump();
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

        self.bump();
    }

    pub fn flip_vertical(&mut self) {
        for point in &mut self.segments {
            point.y = 1.0 - point.y;
        }

        self.bump();
    }

    /// Level at `phase`. A non-finite phase reads the start of the curve.
    pub fn value_at(&self, phase: Smp) -> Smp {
        let start_level = self
            .segments
            .first()
            .map(|segment| segment.y)
            .unwrap_or(0.0);

        self.value_from(phase, start_level)
    }

    /// Level at `phase`, with the first segment leaving `start_level`.
    ///
    /// A non-finite phase reads that start. A non-finite `start_level` uses the
    /// drawn start. Later segments are unchanged, and the first segment still
    /// arrives at its drawn end.
    pub fn value_from(&self, phase: Smp, start_level: Smp) -> Smp {
        let n = self.num_segments();

        if n == 0 {
            return 0.0;
        }

        let phase = if phase.is_finite() {
            phase.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let start_level = if start_level.is_finite() {
            start_level
        } else {
            self.segments[0].y
        };

        for (index, pair) in self.segments.windows(2).enumerate() {
            if pair[0].x <= phase && pair[1].x >= phase {
                let mut start = pair[0];

                if index == 0 {
                    start.y = start_level;
                }

                return Segment::segment_level(phase, start, pair[1]);
            }
        }

        self.segments[n - 1].y
    }

    /// Write one DSP cycle into `out` at phases `i / out.len()` on `[0, 1)`.
    pub fn render(&self, out: &mut [Smp]) {
        let len = out.len();
        let n = self.num_segments();

        if len < 2 || n < MIN_POINTS {
            return;
        }

        debug_assert_eq!(self.segments[0].x, 0.0);
        debug_assert_eq!(self.segments[n - 1].x, 1.0);

        let denom = len as Smp;
        let phase_step = denom.recip();
        let mut from = 0;

        for pair in self.segments.windows(2) {
            // The sample on this breakpoint belongs to the segment that ends here.
            let to = Self::sample_index(pair[1].x, denom, len) + 1;

            if from >= to {
                continue;
            }

            Curve::render(
                &mut out[from..to],
                from as Smp * phase_step,
                phase_step,
                pair[0],
                pair[1],
            );
            from = to;
        }

        debug_assert_eq!(from, len);
    }

    /// Polyline for `pixels` columns. `scratch` is resized to `pixels + 1` and holds
    /// the closed samples at phases `i / pixels`.
    pub fn render_points(
        &self,
        pixels: usize,
        points: &mut Vec<CurvePoint>,
        scratch: &mut Vec<Smp>,
    ) {
        points.clear();

        let n = self.num_segments();

        if pixels < 1 || n < MIN_POINTS {
            return;
        }

        debug_assert_eq!(self.segments[0].x, 0.0);
        debug_assert_eq!(self.segments[n - 1].x, 1.0);

        let len = pixels + 1;

        scratch.resize(len, 0.0);

        let denom = pixels as Smp;
        let phase_step = denom.recip();
        let mut from = 0;

        for pair in self.segments.windows(2) {
            points.push(CurvePoint {
                x: pair[0].x,
                y: pair[0].y,
            });

            let to = Self::sample_index(pair[1].x, denom, len) + 1;

            if from >= to {
                continue;
            }

            Curve::render(
                &mut scratch[from..to],
                from as Smp * phase_step,
                phase_step,
                pair[0],
                pair[1],
            );

            points.extend(
                scratch[from..to]
                    .iter()
                    .enumerate()
                    .map(|(offset, &y)| CurvePoint {
                        x: (from + offset) as Smp * phase_step,
                        y,
                    }),
            );

            from = to;
        }

        let end = self.segments[n - 1];

        points.push(CurvePoint { x: end.x, y: end.y });

        debug_assert_eq!(from, len);
    }

    fn sample_index(x: Smp, denom: Smp, len: usize) -> usize {
        (x * denom).floor().clamp(0.0, (len - 1) as Smp) as usize
    }
}

impl Segment {
    fn segment_level(phase: Smp, start: Segment, end: Segment) -> Smp {
        let width = end.x - start.x;

        if width <= 0.0 {
            return end.y;
        }

        let mut t = (phase - start.x) / width;

        if start.smooth {
            t = ((t - 0.5) * std::f32::consts::PI).sin().mul_add(0.5, 0.5);
        }

        power_scale(t, start.power).mul_add(end.y - start.y, start.y)
    }

    /// `Ok(position)` — this segment filled `out` and is still running.
    /// `Err(written)` — this segment ended after `written` samples (`0` if it had none left).
    ///
    /// `position` is normalized progress through the segment. `phase_step` is the curve
    /// advanced per sample. A non-positive step holds the current level. The step stays
    /// constant for the fill.
    pub fn render_segment(
        out: &mut [Smp],
        position: Smp,
        phase_step: Smp,
        start: Segment,
        end: Segment,
    ) -> Result<Smp, usize> {
        let width = end.x - start.x;

        if width <= 0.0 || position >= 1.0 {
            return Err(0);
        }

        let phase = start.x + position * width;

        if phase_step <= 0.0 {
            Curve::render(out, phase, 0.0, start, end);
            return Ok(position);
        }

        let remaining = (1.0 - position).max(0.0) * width;
        let n = ((remaining / phase_step) as usize).min(out.len());

        if n == 0 {
            return Err(0);
        }

        Curve::render(&mut out[..n], phase, phase_step, start, end);

        let position = (position + n as Smp * phase_step / width).min(1.0);

        if n < out.len() || position >= 1.0 {
            Err(n)
        } else {
            Ok(position)
        }
    }
}

struct Curve<'a> {
    out: &'a mut [Smp],
    t: WideSmp,
    step: WideSmp,
    origin: WideSmp,
    delta: WideSmp,
    power: PowerScale,
}

impl Curve<'_> {
    /// Fill `out` along the segment from `start` to `end`.
    ///
    /// Sample `i` is read at phase `phase + i * phase_step`. That phase is scaled into
    /// the segment's progress once; each sample then advances the progress. A
    /// non-positive width fills `end.y`.
    fn render(out: &mut [Smp], phase: Smp, phase_step: Smp, start: Segment, end: Segment) {
        let width = end.x - start.x;

        if width <= 0.0 {
            out.fill(end.y);
            return;
        }

        let span_scale = width.recip();
        let t_step = WideSmp::splat(phase_step * span_scale);
        let mut curve = Curve {
            out,
            t: WideSmp::splat((phase - start.x) * span_scale)
                + WideSmp::new([0.0, 1.0, 2.0, 3.0]) * t_step,
            step: t_step * WideSmp::splat(SIMD_LANES as Smp),
            origin: WideSmp::splat(start.y),
            delta: WideSmp::splat(end.y - start.y),
            power: PowerScale::new(start.power),
        };

        match (start.smooth, curve.power.curved) {
            (false, false) => curve.fill::<false, false>(),
            (false, true) => curve.fill::<false, true>(),
            (true, false) => curve.fill::<true, false>(),
            (true, true) => curve.fill::<true, true>(),
        }
    }

    #[inline(always)]
    fn fill<const SMOOTH: bool, const CURVED: bool>(&mut self) {
        fill_wide(self.out, || {
            let progress = if SMOOTH {
                fast_sin_pi_wide(self.t - WideSmp::HALF).mul_add(WideSmp::HALF, WideSmp::HALF)
            } else {
                self.t
            };

            self.t += self.step;

            self.power
                .scale::<CURVED>(progress)
                .mul_add(self.delta, self.origin)
        });
    }
}

/// [`power_scale`](crate::power_scale) for each lane.
///
/// `scale` is `1 / (exp(power) - 1)`, computed once and applied with a multiply.
struct PowerScale {
    curved: bool,
    power_log2e: WideSmp,
    scale: WideSmp,
}

impl PowerScale {
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

    #[inline(always)]
    fn scale<const CURVED: bool>(&self, t: WideSmp) -> WideSmp {
        if !CURVED {
            return t;
        }

        let exp = fast_exp2_wide(t * self.power_log2e);

        (exp - WideSmp::ONE) * self.scale
    }
}

#[cfg(test)]
mod tests;
