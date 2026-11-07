use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use crate::{Smp, utils::sanitize};

use super::curve::{self, Curve, CurveParam};

pub const MAX_POINTS: usize = 100;
pub const MAX_POWER: Smp = 20.0;

const MIN_POINTS: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SegPoint {
    pub x: Smp,
    pub y: Smp,
    #[serde(flatten)]
    pub curve: CurveParam,
}

impl Default for SegPoint {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            curve: CurveParam::default(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RelSegments {
    #[serde(rename = "segments")]
    points: Vec<SegPoint>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    loop_point: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    release_point: Option<usize>,
    #[serde(skip)]
    revision: u64,
}

impl Default for RelSegments {
    fn default() -> Self {
        Self::from_points(&[(0.0, 0.0), (0.5, 1.0), (1.0, 0.0)], false)
    }
}

impl RelSegments {
    /// Build a curve from phase/level pairs. Every point shares the same `smooth` flag.
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
                curve: CurveParam {
                    smooth,
                    ..CurveParam::default()
                },
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
            && start.curve == CurveParam::default()
    }

    pub fn power(&self, index: usize) -> Smp {
        self.points[index].curve.power
    }

    pub fn set_point(&mut self, index: usize, mut point: SegPoint) {
        let n = self.num_points();

        if index >= n {
            return;
        }

        point.x = sanitize(point.x, 0.0, 1.0);
        point.y = sanitize(point.y, 0.0, 1.0);
        point.curve.power = sanitize(point.curve.power, -MAX_POWER, MAX_POWER);

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

        self.points[index].curve.power = sanitize(power, -MAX_POWER, MAX_POWER);
        self.bump();
    }

    pub fn set_smooth(&mut self, index: usize, smooth: bool) {
        if index >= self.num_points() {
            return;
        }

        self.points[index].curve.smooth = smooth;
        self.bump();
    }

    pub fn reset_powers(&mut self) {
        for point in &mut self.points {
            point.curve.power = 0.0;
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
            point.curve.power = sanitize(point.curve.power, -MAX_POWER, MAX_POWER);
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

        point.curve.power = sanitize(point.curve.power, -MAX_POWER, MAX_POWER);

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
            let mut incoming = self.points[j].curve;
            let mut outgoing = self.points[i].curve;

            incoming.power = -incoming.power;
            outgoing.power = -outgoing.power;
            self.points[i].curve = incoming;
            self.points[j].curve = outgoing;
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

        for (index, &[left, right]) in self.points.array_windows().enumerate() {
            // The release point belongs to the segment that leaves it, so a playhead
            // sitting on that point reads the overridden level rather than the one drawn.
            if let Some((rel, _)) = release
                && rel == index + 1
                && phase == right.x
                && rel + 1 < n
            {
                continue;
            }

            if left.x <= phase && right.x >= phase {
                let mut start = left;

                if let Some((rel, level)) = release
                    && rel == index
                {
                    debug_assert!(level.is_finite());
                    start.y = level;
                } else if index == 0 {
                    start.y = start_level;
                }

                return SegPoint::segment_level(phase, start, right);
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

        for &[left, right] in self.points.array_windows() {
            let mut to = curve::sample_end(right.x, steps);

            if !closed {
                to = to.min(len);
            }

            if from >= to {
                continue;
            }

            let width = right.x - left.x;

            assert!(width > 0.0);

            let scale = width.recip();
            let phase = from as Smp * phase_step;

            Curve::render(
                &mut out[from..to],
                (phase - left.x) * scale,
                phase_step * scale,
                left.y..right.y,
                left.curve,
            );
            from = to;
        }

        debug_assert_eq!(from, len);
    }

    /// Polyline for `pixels` columns. Each point is `[phase, level]`.
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

        for &[left, right] in self.points.array_windows() {
            points.push([left.x, left.y]);

            let to = curve::sample_end(right.x, pixels);

            if from >= to {
                continue;
            }

            let scale = (right.x - left.x).recip();
            let phase = from as Smp * phase_step;

            Curve::render(
                &mut scratch[from..to],
                (phase - left.x) * scale,
                phase_step * scale,
                left.y..right.y,
                left.curve,
            );

            for (offset, &y) in scratch[from..to].iter().enumerate() {
                points.push([(from + offset) as Smp * phase_step, y]);
            }

            from = to;
        }

        points.push([self.points[n - 1].x, self.points[n - 1].y]);

        debug_assert_eq!(from, len);
    }
}

impl SegPoint {
    fn segment_level(phase: Smp, start: SegPoint, end: SegPoint) -> Smp {
        let width = end.x - start.x;

        if width <= 0.0 {
            return end.y;
        }

        let mut sample = [0.0];

        let _ = curve::render_segment(
            &mut sample,
            ((phase - start.x) / width).clamp(0.0, 1.0),
            0.0,
            start.y..end.y,
            start.curve,
        );
        sample[0]
    }
}

#[cfg(test)]
mod tests;
