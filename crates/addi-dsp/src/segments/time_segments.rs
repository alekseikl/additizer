use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use crate::{
    Smp,
    utils::{SmpUtils, sanitize},
};

use super::curve::{self, Curve, CurveParam};
use super::rel_segments::MAX_POWER;

pub const MAX_POINTS: usize = 100;
pub const MAX_SEGMENTS: usize = MAX_POINTS - 1;
/// Longest curve, in seconds from the first point.
pub const MAX_TIME: Smp = 60.0;

const MIN_POINTS: usize = 2;

/// One breakpoint. `duration` is the seconds until the next breakpoint.
/// The last point's duration is `0`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct TimePoint {
    pub duration: Smp,
    pub y: Smp,
    #[serde(flatten)]
    pub curve: CurveParam,
}

impl Default for TimePoint {
    fn default() -> Self {
        Self {
            duration: 0.0,
            y: 0.0,
            curve: CurveParam::default(),
        }
    }
}

impl TimePoint {
    /// Level at `position` along the segment from `start` to `end`.
    /// `position` is 0 at `start` and 1 at `end`. A zero-length segment is `end.y`.
    pub fn level_between(start: Self, end: Self, position: Smp) -> Smp {
        if start.duration <= 0.0 {
            return end.y;
        }

        let mut sample = [0.0];

        let _ = curve::render_segment(
            &mut sample,
            position.clamp(0.0, 1.0),
            0.0,
            start.y..end.y,
            start.curve,
        );
        sample[0]
    }

    fn clamp_shape(&mut self) {
        self.y = sanitize(self.y, 0.0, 1.0);
        self.curve.power = sanitize(self.curve.power, -MAX_POWER, MAX_POWER);
    }
}

/// Breakpoint curve in seconds.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TimeSegments {
    points: Vec<TimePoint>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    loop_point: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    release_point: Option<usize>,
    #[serde(skip)]
    revision: u64,
}

impl Default for TimeSegments {
    fn default() -> Self {
        let attack = 0.004;
        let hold = 1.0 - attack;
        let release = 0.2;

        Self {
            points: vec![
                TimePoint {
                    duration: attack,
                    ..TimePoint::default()
                },
                TimePoint {
                    duration: hold,
                    y: 1.0,
                    ..TimePoint::default()
                },
                TimePoint {
                    duration: release,
                    y: 1.0,
                    curve: CurveParam {
                        power: -5.0,
                        ..CurveParam::default()
                    },
                },
                TimePoint::default(),
            ],
            loop_point: None,
            release_point: Some(2),
            revision: Self::next_revision(),
        }
    }
}

impl TimeSegments {
    pub fn num_points(&self) -> usize {
        debug_assert!(
            (MIN_POINTS..=MAX_POINTS).contains(&self.points.len()),
            "time segments must have {MIN_POINTS} to {MAX_POINTS} points"
        );

        self.points.len()
    }

    pub fn num_segments(&self) -> usize {
        self.num_points() - 1
    }

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

    pub fn point(&self, index: usize) -> TimePoint {
        self.points[index]
    }

    pub fn points(&self) -> &[TimePoint] {
        &self.points
    }

    /// Seconds from the start of the curve to point `index`.
    pub fn time(&self, index: usize) -> Smp {
        self.points
            .array_windows()
            .take(index)
            .map(|&[point, _]| point.duration)
            .sum()
    }

    /// Seconds from point `index` to the next point. The last point has no segment.
    pub fn duration(&self, index: usize) -> Smp {
        if index >= self.num_segments() {
            return 0.0;
        }

        self.points[index].duration
    }

    pub fn total_time(&self) -> Smp {
        self.time(self.num_points() - 1)
    }

    pub fn power(&self, index: usize) -> Smp {
        self.points[index].curve.power
    }

    /// Replace breakpoint `index`. `point.duration` is the seconds until the next
    /// breakpoint, and a change there shifts every later point. The last duration stays 0.
    pub fn set_point(&mut self, index: usize, mut point: TimePoint) {
        let n = self.num_points();

        if index >= n {
            return;
        }

        point.clamp_shape();
        point.duration = if index + 1 == n {
            0.0
        } else {
            self.clamp_duration(index, point.duration)
        };
        self.points[index] = point;
        self.bump();
    }

    /// Place point `index` at `time` seconds from the start.
    ///
    /// That changes the previous point's duration. Every other duration stays, so the
    /// total length changes with this point. The first point stays at 0.
    pub fn set_time(&mut self, index: usize, time: Smp) {
        if index == 0 || index >= self.num_points() {
            return;
        }

        let previous = self.time(index - 1);

        self.set_duration(index - 1, time - previous);
    }

    /// Set the seconds from point `index` to the next point. Later durations stay,
    /// so the total length changes. The last point has no segment to set.
    pub fn set_duration(&mut self, index: usize, duration: Smp) {
        if index >= self.num_segments() {
            return;
        }

        self.points[index].duration = self.clamp_duration(index, duration);
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

    fn clamp_duration(&self, index: usize, duration: Smp) -> Smp {
        let others: Smp = (0..self.num_segments())
            .filter(|other| *other != index)
            .map(|other| self.points[other].duration.max(0.0))
            .sum();
        let max = (MAX_TIME - others).max(0.0);

        sanitize(duration, 0.0, max)
    }

    /// Clamp every breakpoint into range. Fewer than two points becomes the default curve.
    /// Durations stay non-negative, the last duration is 0, and the total is at most [`MAX_TIME`].
    pub fn sanitize(&mut self) {
        self.points.truncate(MAX_POINTS);

        if self.points.len() < MIN_POINTS {
            *self = Self::default();
            return;
        }

        let n = self.points.len();
        let mut used = 0.0;

        for (index, point) in self.points.iter_mut().enumerate() {
            point.clamp_shape();

            if index + 1 == n {
                point.duration = 0.0;
            } else {
                let room = (MAX_TIME - used).max(0.0);

                point.duration = sanitize(point.duration, 0.0, room);
                used += point.duration;
            }
        }

        self.retain_markers();
        self.bump();
    }

    pub fn loop_point(&self) -> Option<TimePoint> {
        self.loop_point_idx().map(|index| self.points[index])
    }

    pub fn release_point(&self) -> Option<TimePoint> {
        self.release_point_idx().map(|index| self.points[index])
    }

    pub fn loop_point_idx(&self) -> Option<usize> {
        self.loop_point
    }

    pub fn release_point_idx(&self) -> Option<usize> {
        self.release_point
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

    /// `None` clears the release. A release sits on an inner point. Setting one at or
    /// before the loop drops the loop.
    pub fn set_release_point(&mut self, index: Option<usize>) -> bool {
        if !index.is_none_or(|index| self.release_index_ok(index)) || self.release_point == index {
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

    fn release_index_ok(&self, index: usize) -> bool {
        index > 0 && index + 1 < self.num_points()
    }

    fn retain_markers(&mut self) {
        self.release_point = self
            .release_point
            .filter(|&index| self.release_index_ok(index));
        self.loop_point = self.loop_point.filter(|&index| self.loop_index_ok(index));
    }

    fn shift_inserted(&mut self, index: usize) {
        for marker in [&mut self.loop_point, &mut self.release_point] {
            if let Some(point) = marker
                && *point >= index
            {
                *point += 1;
            }
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

    /// Insert a breakpoint at absolute `time` seconds.
    pub fn add_point(&mut self, index: usize, time: Smp, mut point: TimePoint) -> bool {
        let n = self.num_points();

        if n >= MAX_POINTS || index == 0 || index > n {
            return false;
        }

        point.clamp_shape();

        let previous = self.time(index - 1);
        let next = if index == n {
            MAX_TIME
        } else {
            self.time(index)
        };
        let time = sanitize(time, 0.0, MAX_TIME).clamp(previous, next.max(previous));

        self.points[index - 1].duration = time - previous;
        point.duration = if index == n {
            0.0
        } else {
            (next - time).max(0.0)
        };

        if index == n {
            self.points.push(point);
        } else {
            self.points.insert(index, point);
        }

        self.shift_inserted(index);
        self.bump();
        true
    }

    /// Drop an interior breakpoint.
    pub fn remove_point(&mut self, index: usize) -> bool {
        let n = self.num_points();

        if n <= MIN_POINTS || index == 0 || index + 1 == n {
            return false;
        }

        let removed = self.points[index].duration.max(0.0);

        self.points[index - 1].duration =
            (self.points[index - 1].duration.max(0.0) + removed).min(MAX_TIME);
        self.points.remove(index);
        self.shift_removed(index);
        self.bump();
        true
    }

    /// Seconds on the stored curve at `position` through the segment that starts at `index`.
    /// An index on or past the last segment is the end of the curve.
    pub fn time_at(&self, index: usize, position: Smp) -> Smp {
        if index >= self.num_segments() {
            return self.total_time();
        }

        self.time(index) + position * self.duration(index)
    }

    /// Level at `time` seconds. Times past the end hold the last level.
    pub fn value_at(&self, time: Smp) -> Smp {
        let n = self.points.len();
        let time = time.finite_or_zero();

        if time <= 0.0 {
            return self.points[0].y;
        }

        let mut start = 0.0;

        for &[left, right] in self.points.array_windows() {
            let duration = left.duration;
            let end = start + duration;

            if duration <= 0.0 {
                continue;
            }

            if time <= end {
                let position = (time - start) / duration;

                return TimePoint::level_between(left, right, position);
            }

            start = end;
        }

        self.points[n - 1].y
    }

    /// Polyline for `pixels` columns across `time_span` seconds. Each point is `[seconds, level]`.
    pub fn render_points(
        &self,
        pixels: usize,
        time_span: Smp,
        points: &mut Vec<[Smp; 2]>,
        scratch: &mut Vec<Smp>,
    ) {
        points.clear();

        let n = self.num_points();

        if pixels < 1 || time_span <= 0.0 {
            return;
        }

        debug_assert_eq!(self.points[n - 1].duration, 0.0);

        scratch.resize(pixels + 1, 0.0);

        let mut from = 0;
        let mut start = 0.0;

        for &[left, right] in self.points.array_windows() {
            let duration = left.duration;
            let end = start + duration;

            points.push([start, left.y]);

            if duration <= 0.0 {
                points.push([end, right.y]);
                start = end;
                continue;
            }

            let to = curve::sample_end(end / time_span, pixels);

            if from >= to {
                start = end;
                continue;
            }

            // One column is `time_span / pixels` seconds, including past the window.
            let seconds_per_sample = time_span / pixels as Smp;
            let scale = duration.recip();

            Curve::render(
                &mut scratch[from..to],
                (from as Smp * seconds_per_sample - start) * scale,
                seconds_per_sample * scale,
                left.y..right.y,
                left.curve,
            );

            for (offset, &y) in scratch[from..to].iter().enumerate() {
                let time = (from + offset) as Smp * seconds_per_sample;

                points.push([time, y]);
            }

            from = to;
            start = end;
        }

        points.push([start, self.points[n - 1].y]);

        debug_assert_eq!(
            from,
            curve::sample_end(self.total_time() / time_span, pixels)
        );
        debug_assert_eq!(start, self.total_time());
    }
}

#[cfg(test)]
mod tests;
