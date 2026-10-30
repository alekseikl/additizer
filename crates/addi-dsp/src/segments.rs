use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use crate::{
    SIMD_LANES, Smp, WideSmp,
    fast_math::{fast_exp2_wide, fast_sin_pi_wide},
    power_scale,
    simd_utils::fill,
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

/// Breakpoint curve. Always holds two to [`MAX_POINTS`] points.
/// Deserialization and [`sanitize`](Self::sanitize) replace a shorter curve with [`Default`].
#[derive(Clone, Debug, Serialize)]
pub struct Segments {
    #[serde(rename = "segments")]
    points: Vec<SegPoint>,
    /// Loop start. Any point except the last, and before [`release_point`](Self::release_point)
    /// when that is set. The envelope repeats from here.
    #[serde(skip_serializing_if = "Option::is_none")]
    loop_point: Option<usize>,
    /// Sustain while a key is held, and the point note-off jumps to.
    /// The release segment then leaves from the level playing at that moment.
    #[serde(skip_serializing_if = "Option::is_none")]
    release_point: Option<usize>,
    /// Identity of this edit. Replaced on every change; clones keep it until edited.
    #[serde(skip)]
    revision: u64,
}

impl Default for Segments {
    fn default() -> Self {
        Self::from_points(&[(0.0, 0.0), (0.5, 1.0), (1.0, 0.0)], false)
    }
}

impl Segments {
    /// Build a curve from phase/level pairs. Every point shares the same `smooth` flag.
    ///
    /// # Panics
    ///
    /// Panics when `points` has fewer than two or more than [`MAX_POINTS`] entries.
    pub fn from_points(points: &[(Smp, Smp)], smooth: bool) -> Self {
        assert!(
            (MIN_POINTS..=MAX_POINTS).contains(&points.len()),
            "segments must have {MIN_POINTS} to {MAX_POINTS} points"
        );

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
            loop_point: None,
            release_point: None,
            revision: Self::next_revision(),
        }
    }

    pub fn num_points(&self) -> usize {
        debug_assert!(
            (MIN_POINTS..=MAX_POINTS).contains(&self.points.len()),
            "segments must have {MIN_POINTS} to {MAX_POINTS} points"
        );

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

    /// `(0, 0)` to `(1, 1)`, two points, unsmoothed, with power `0`.
    pub fn is_unity(&self) -> bool {
        let [start, end] = self.points.as_slice() else {
            return false;
        };

        start.x == 0.0
            && start.y == 0.0
            && end.x == 1.0
            && end.y == 1.0
            && start.power == 0.0
            && !start.smooth
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
        self.retain_markers();
        self.bump();
    }

    pub fn loop_point(&self) -> Option<SegPoint> {
        self.loop_point_idx().map(|index| self.points[index])
    }

    pub fn release_point(&self) -> Option<SegPoint> {
        self.release_point_idx().map(|index| self.points[index])
    }

    pub fn loop_point_idx(&self) -> Option<usize> {
        self.loop_point
    }

    pub fn release_point_idx(&self) -> Option<usize> {
        self.release_point
    }

    /// Samples in one pass of the loop at `phase_step`.
    pub fn loop_samples(&self, phase_step: Smp) -> usize {
        let Some(start) = self.loop_point_idx() else {
            return 0;
        };

        if phase_step < Smp::EPSILON {
            return 0;
        }

        let end = self.release_point_idx().unwrap_or(self.num_points() - 1);
        let width = (self.point(end).x - self.point(start).x).max(0.0);

        (width / phase_step) as usize
    }

    /// `None` clears the loop. A loop sits on any point except the last, and before the
    /// release point when one is set.
    pub fn set_loop_point(&mut self, index: Option<usize>) -> bool {
        if !index.is_none_or(|index| self.loop_index_ok(index)) || self.loop_point == index {
            return false;
        }

        self.loop_point = index;
        self.bump();
        true
    }

    /// `None` clears the release. Setting one at or before the loop drops the loop.
    pub fn set_release_point(&mut self, index: Option<usize>) -> bool {
        if !index.is_none_or(|index| index < self.num_points()) || self.release_point == index {
            return false;
        }

        if let Some(index) = index
            && self
                .loop_point
                .is_some_and(|loop_point| loop_point >= index)
        {
            self.loop_point = None;
        }

        self.release_point = index;
        self.bump();
        true
    }

    fn loop_index_ok(&self, index: usize) -> bool {
        index + 1 < self.num_points() && self.release_point.is_none_or(|release| index < release)
    }

    fn retain_markers(&mut self) {
        let n = self.num_points();

        self.release_point = self.release_point.filter(|&index| index < n);
        self.loop_point = self.loop_point.filter(|&index| self.loop_index_ok(index));
    }

    fn shift_inserted(&mut self, index: usize) {
        if let Some(point) = &mut self.loop_point
            && *point >= index
        {
            *point += 1;
        }

        if let Some(point) = &mut self.release_point
            && *point >= index
        {
            *point += 1;
        }
    }

    fn shift_removed(&mut self, index: usize) {
        self.loop_point = Self::shift_removed_index(self.loop_point, index);
        self.release_point = Self::shift_removed_index(self.release_point, index);
        self.retain_markers();
    }

    fn shift_removed_index(marker: Option<usize>, index: usize) -> Option<usize> {
        match marker {
            Some(point) if point == index => None,
            Some(point) if point > index => Some(point - 1),
            other => other,
        }
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
        self.shift_inserted(index);
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
        self.shift_removed(index);
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

        self.loop_point = self.loop_point.map(|index| n - 1 - index);
        self.release_point = self.release_point.map(|index| n - 1 - index);
        self.retain_markers();
        self.bump();
    }

    pub fn flip_vertical(&mut self) {
        for point in &mut self.points {
            point.y = 1.0 - point.y;
        }

        self.bump();
    }

    /// Phase on `[0, 1]` at `position` through the segment that starts at `index`.
    /// `position` runs from 0 at that point to 1 at the next. An index on or past
    /// the last point is the end of the curve.
    pub fn phase_at(&self, index: usize, position: Smp) -> Smp {
        let spans = self.num_points() - 1;

        if index >= spans {
            return 1.0;
        }

        let start = self.point(index).x;
        let end = self.point(index + 1).x;
        let width = (end - start).max(0.0);

        (start + position.clamp(0.0, 1.0) * width).clamp(0.0, 1.0)
    }

    /// Level at `phase` on `[0, 1]`. `start_level` replaces the first point.
    /// `release_level` replaces [`release_point`](Self::release_point) where the
    /// segment leaving it begins, including a release on the last point.
    pub fn value_at(
        &self,
        phase: Smp,
        start_level: Option<Smp>,
        release_level: Option<Smp>,
    ) -> Smp {
        let n = self.num_points();

        debug_assert!(phase.is_finite());

        let phase = phase.clamp(0.0, 1.0);
        let start_level = match start_level {
            Some(start_level) => {
                debug_assert!(start_level.is_finite());
                start_level
            }
            None => self.points[0].y,
        };
        let release = release_level.and_then(|level| {
            debug_assert!(level.is_finite());
            self.release_point.map(|index| (index, level))
        });

        if let Some((index, level)) = release
            && index + 1 == n
            && phase >= self.points[index].x
        {
            debug_assert!(level.is_finite());
            return level;
        }

        for (index, pair) in self.points.windows(2).enumerate() {
            // The release point belongs to the segment that leaves it, so a playhead
            // sitting on that point reads the overridden level rather than the one drawn.
            if let Some((rel, _)) = release
                && rel == index + 1
                && phase == pair[1].x
                && rel + 1 < n
            {
                continue;
            }

            if pair[0].x <= phase && pair[1].x >= phase {
                let mut start = pair[0];

                if let Some((rel, level)) = release
                    && rel == index
                {
                    debug_assert!(level.is_finite());
                    start.y = level;
                } else if index == 0 {
                    start.y = start_level;
                }

                return SegPoint::segment_level(phase, start, pair[1]);
            }
        }

        self.points[n - 1].y
    }

    /// Write one DSP cycle into `out` at phases `i / out.len()` on `[0, 1)`.
    pub fn render_waveform(&self, out: &mut [Smp]) {
        self.render_range(out, false);
    }

    /// Write `out.len()` samples on the closed interval `[0, 1]`.
    /// Sample `i` is at phase `i / (out.len() - 1)`, so the last sample is phase 1.
    pub fn render_closed(&self, out: &mut [Smp]) {
        self.render_range(out, true);
    }

    fn render_range(&self, out: &mut [Smp], closed: bool) {
        let len = out.len();

        if len < 2 {
            return;
        }

        debug_assert_eq!(self.points[0].x, 0.0);
        debug_assert_eq!(self.points[self.num_points() - 1].x, 1.0);

        let steps = if closed { len - 1 } else { len };
        let phase_step = (steps as Smp).recip();
        let mut from = 0;

        for pair in self.points.windows(2) {
            let mut to = Self::sample_end(pair[1].x, steps);

            if !closed {
                to = to.min(len);
            }

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
    ///
    /// Columns sample each segment. A vertical step is the two breakpoints that
    /// share a phase, so the drop is a span of the polyline.
    pub fn render_points(&self, pixels: usize, points: &mut Vec<[Smp; 2]>, scratch: &mut Vec<Smp>) {
        points.clear();

        let n = self.num_points();

        if pixels < 1 {
            return;
        }

        debug_assert_eq!(self.points[0].x, 0.0);
        debug_assert_eq!(self.points[n - 1].x, 1.0);

        let len = pixels + 1;

        scratch.resize(len, 0.0);

        let phase_step = (pixels as Smp).recip();
        let mut from = 0;

        for pair in self.points.windows(2) {
            if pair[1].x == pair[0].x {
                let top = [pair[0].x, pair[0].y];

                // The column sampled up to this phase has the level before the step.
                if let Some(last) = points.last_mut()
                    && last[0] == top[0]
                {
                    *last = top;
                } else {
                    points.push(top);
                }

                points.push([pair[1].x, pair[1].y]);
                continue;
            }

            Self::push_point(points, [pair[0].x, pair[0].y]);

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

            for (offset, &y) in scratch[from..to].iter().enumerate() {
                Self::push_point(points, [(from + offset) as Smp * phase_step, y]);
            }

            from = to;
        }

        Self::push_point(points, [self.points[n - 1].x, self.points[n - 1].y]);

        debug_assert_eq!(from, len);
    }

    fn push_point(points: &mut Vec<[Smp; 2]>, point: [Smp; 2]) {
        if points.last() != Some(&point) {
            points.push(point);
        }
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
        fill(self.out, || {
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

#[derive(Deserialize)]
struct RawSegments {
    #[serde(rename = "segments")]
    points: Vec<SegPoint>,
    #[serde(default)]
    loop_point: Option<usize>,
    #[serde(default)]
    release_point: Option<usize>,
}

impl<'de> Deserialize<'de> for Segments {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = RawSegments::deserialize(deserializer)?;
        let mut segments = Self {
            points: raw.points,
            loop_point: raw.loop_point,
            release_point: raw.release_point,
            revision: 0,
        };

        segments.sanitize();
        Ok(segments)
    }
}

#[cfg(test)]
mod tests;
