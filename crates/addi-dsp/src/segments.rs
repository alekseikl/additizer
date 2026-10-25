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
pub struct SegPoint {
    pub x: Smp,
    pub y: Smp,
    pub power: Smp,
    pub smooth: bool,
}

impl Default for SegPoint {
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
    #[serde(rename = "segments")]
    points: Vec<SegPoint>,
    /// Identity of this edit. Replaced on every change; clones keep it until edited.
    #[serde(skip, default = "Segments::next_revision")]
    revision: u64,
}

impl PartialEq for Segments {
    fn eq(&self, other: &Self) -> bool {
        self.points == other.points
    }
}

impl Default for Segments {
    fn default() -> Self {
        Self::from_points(&[(0.0, 0.0), (0.5, 1.0), (1.0, 0.0)], false)
    }
}

impl Segments {
    /// Build a curve from phase/level pairs. Every point shares the same `smooth` flag.
    pub fn from_points(points: &[(Smp, Smp)], smooth: bool) -> Self {
        debug_assert!(points.len() >= MIN_POINTS && points.len() <= MAX_POINTS);

        let mut curve = Vec::with_capacity(MAX_POINTS);

        for &(x, y) in points {
            curve.push(SegPoint {
                x,
                y,
                smooth,
                ..SegPoint::default()
            });
        }

        Self {
            points: curve,
            revision: Self::next_revision(),
        }
    }

    pub fn num_points(&self) -> usize {
        self.points.len()
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

    pub fn point(&self, index: usize) -> SegPoint {
        self.points[index]
    }

    pub fn points(&self) -> &[SegPoint] {
        &self.points
    }

    pub fn power(&self, index: usize) -> Smp {
        self.points[index].power
    }

    pub fn set_point(&mut self, index: usize, mut point: SegPoint) {
        let n = self.num_points();

        if index >= n {
            return;
        }

        point.x = sanitize(point.x, 0.0, 1.0);
        point.y = sanitize(point.y, 0.0, 1.0);
        point.power = sanitize(point.power, -MAX_POWER, MAX_POWER);

        if index == 0 {
            point.x = 0.0;
        } else if index + 1 == n {
            point.x = 1.0;
        }

        self.points[index] = point;
        self.bump();
    }

    pub fn set_power(&mut self, index: usize, power: Smp) {
        if index >= self.num_points() {
            return;
        }

        self.points[index].power = sanitize(power, -MAX_POWER, MAX_POWER);
        self.bump();
    }

    pub fn set_smooth(&mut self, index: usize, smooth: bool) {
        if index >= self.num_points() {
            return;
        }

        self.points[index].smooth = smooth;
        self.bump();
    }

    pub fn reset_powers(&mut self) {
        for point in &mut self.points {
            point.power = 0.0;
        }

        self.bump();
    }

    /// Clamp every breakpoint into range. Fewer than two points becomes the default curve.
    pub fn sanitize(&mut self) {
        self.points.truncate(MAX_POINTS);

        if self.points.len() < MIN_POINTS {
            *self = Self::default();
            return;
        }

        for point in &mut self.points {
            point.x = sanitize(point.x, 0.0, 1.0);
            point.y = sanitize(point.y, 0.0, 1.0);
            point.power = sanitize(point.power, -MAX_POWER, MAX_POWER);
        }

        let n = self.points.len();
        self.points[0].x = 0.0;
        self.points[n - 1].x = 1.0;
        self.bump();
    }

    /// Insert `point` so it stays in phase order. Fails when the curve is full.
    pub fn add_point(&mut self, index: usize, mut point: SegPoint) -> bool {
        let n = self.num_points();

        if n >= MAX_POINTS || index > n {
            return false;
        }

        point.x = sanitize(point.x, 0.0, 1.0);
        point.y = sanitize(point.y, 0.0, 1.0);

        point.power = sanitize(point.power, -MAX_POWER, MAX_POWER);

        if index == 0 {
            point.x = 0.0;
        }

        if index == n {
            point.x = 1.0;
        }

        self.points.insert(index, point);
        self.bump();
        true
    }

    /// Drop an interior breakpoint. The phase `0` and phase `1` endpoints stay.
    pub fn remove_point(&mut self, index: usize) -> bool {
        let n = self.num_points();

        if n <= MIN_POINTS || index == 0 || index + 1 == n {
            return false;
        }

        self.points.remove(index);
        self.bump();
        true
    }

    pub fn flip_horizontal(&mut self) {
        let n = self.num_points();

        for i in 0..n.div_ceil(2) {
            let tmp_x = 1.0 - self.points[i].x;
            let tmp_y = self.points[i].y;
            let j = n - i - 1;

            self.points[i].x = 1.0 - self.points[j].x;
            self.points[i].y = self.points[j].y;
            self.points[j].x = tmp_x;
            self.points[j].y = tmp_y;
        }

        for i in 0..n / 2 {
            let j = n - i - 2;
            let power = self.points[i].power;
            let smooth = self.points[i].smooth;

            self.points[i].power = -self.points[j].power;
            self.points[i].smooth = self.points[j].smooth;
            self.points[j].power = -power;
            self.points[j].smooth = smooth;
        }

        self.bump();
    }

    pub fn flip_vertical(&mut self) {
        for point in &mut self.points {
            point.y = 1.0 - point.y;
        }

        self.bump();
    }

    /// Level at `phase` on `[0, 1]`.
    pub fn value_at(&self, phase: Smp, start_level: Option<Smp>) -> Smp {
        let n = self.num_points();

        if n == 0 {
            return 0.0;
        }

        debug_assert!(phase.is_finite());

        let phase = phase.clamp(0.0, 1.0);
        let start_level = match start_level {
            Some(start_level) => {
                debug_assert!(start_level.is_finite());
                start_level
            }
            None => self.points[0].y,
        };

        for (index, pair) in self.points.windows(2).enumerate() {
            if pair[0].x <= phase && pair[1].x >= phase {
                let mut start = pair[0];

                if index == 0 {
                    start.y = start_level;
                }

                return SegPoint::segment_level(phase, start, pair[1]);
            }
        }

        self.points[n - 1].y
    }

    /// Write one DSP cycle into `out` at phases `i / out.len()` on `[0, 1)`.
    pub fn render_waveform(&self, out: &mut [Smp]) {
        let len = out.len();
        let n = self.num_points();

        if len < 2 || n < MIN_POINTS {
            return;
        }

        debug_assert_eq!(self.points[0].x, 0.0);
        debug_assert_eq!(self.points[n - 1].x, 1.0);

        let phase_step = (len as Smp).recip();
        let mut from = 0;

        for pair in self.points.windows(2) {
            let to = Self::sample_end(pair[1].x, len).min(len);

            if from >= to {
                continue;
            }

            Curve::render_phase(
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

    /// Polyline for `pixels` columns. Each point is `[phase, level]`.
    pub fn render_points(&self, pixels: usize, points: &mut Vec<[Smp; 2]>, scratch: &mut Vec<Smp>) {
        points.clear();

        let n = self.num_points();

        if pixels < 1 || n < MIN_POINTS {
            return;
        }

        debug_assert_eq!(self.points[0].x, 0.0);
        debug_assert_eq!(self.points[n - 1].x, 1.0);

        let len = pixels + 1;

        scratch.resize(len, 0.0);

        let phase_step = (pixels as Smp).recip();
        let mut from = 0;

        for pair in self.points.windows(2) {
            points.push([pair[0].x, pair[0].y]);

            let to = Self::sample_end(pair[1].x, pixels);

            if from >= to {
                continue;
            }

            Curve::render_phase(
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
                    .map(|(offset, &y)| [(from + offset) as Smp * phase_step, y]),
            );

            from = to;
        }

        let end = self.points[n - 1];

        points.push([end.x, end.y]);

        debug_assert_eq!(from, len);
    }

    /// Exclusive end index of the span that finishes at `x` on a grid of `steps` intervals.
    fn sample_end(x: Smp, steps: usize) -> usize {
        if x <= 0.0 {
            return 0;
        }

        ((x * steps as Smp).floor() + 1.0).min(steps as Smp + 1.0) as usize
    }
}

impl SegPoint {
    fn segment_level(phase: Smp, start: SegPoint, end: SegPoint) -> Smp {
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
    pub fn render_segment(
        out: &mut [Smp],
        position: Smp,
        phase_step: Smp,
        start: SegPoint,
        end: SegPoint,
    ) -> Result<Smp, usize> {
        let width = end.x - start.x;

        if width <= 0.0 || position >= 1.0 {
            return Err(0);
        }

        if phase_step <= 0.0 {
            Curve::render(out, position, 0.0, start, end);
            return Ok(position);
        }

        let t_step = phase_step * width.recip();
        let remaining = (1.0 - position).max(0.0) * width;
        let n = ((remaining / phase_step) as usize).min(out.len());

        if n == 0 {
            return Err(0);
        }

        Curve::render(&mut out[..n], position, t_step, start, end);

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
    /// Fill `out` along the segment from `start` to `end`, starting at curve phase `phase`.
    fn render_phase(out: &mut [Smp], phase: Smp, phase_step: Smp, start: SegPoint, end: SegPoint) {
        let width = end.x - start.x;

        assert!(width > 0.0);

        let span_scale = width.recip();

        Self::render(
            out,
            (phase - start.x) * span_scale,
            phase_step * span_scale,
            start,
            end,
        );
    }

    /// Fill `out` along the segment from `start` to `end`.
    ///
    /// Sample `i` is read at progress `t + i * t_step`. `t` is 0 at `start` and 1 at
    /// `end`, so the fill does not depend on where the segment sits on the curve.
    fn render(out: &mut [Smp], t: Smp, t_step: Smp, start: SegPoint, end: SegPoint) {
        let t_step = WideSmp::splat(t_step);
        let mut curve = Curve {
            out,
            t: WideSmp::splat(t) + WideSmp::new([0.0, 1.0, 2.0, 3.0]) * t_step,
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

struct PowerScale {
    curved: bool,
    power_log2e: WideSmp,
    scale: WideSmp,
}

impl PowerScale {
    fn new(power: Smp) -> Self {
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
