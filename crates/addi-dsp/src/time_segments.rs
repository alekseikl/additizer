use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use crate::{
    Smp,
    segments::{MAX_POWER, SegPoint},
    utils::sanitize,
};

pub const MAX_POINTS: usize = 100;
pub const MAX_SEGMENTS: usize = MAX_POINTS - 1;
/// Longest curve, in seconds from the first point.
pub const MAX_TIME: Smp = 60.0;
/// Shortest view, so a short curve still has room to drag.
pub const MIN_VIEW: Smp = 1.0;
/// The curve fills this fraction of the view. The rest is room to lengthen it.
const VIEW_FILL: Smp = 0.8;

const MIN_POINTS: usize = 2;

/// One breakpoint. `duration` is the seconds until the next breakpoint.
/// The last point's duration is `0`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct TimePoint {
    pub duration: Smp,
    pub y: Smp,
    pub power: Smp,
    pub smooth: bool,
}

impl Default for TimePoint {
    fn default() -> Self {
        Self {
            duration: 0.0,
            y: 0.0,
            power: 0.0,
            smooth: false,
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

        let (start, end) = local_segment(start, end, start.duration);
        let mut sample = [0.0];

        let _ = SegPoint::render_segment(&mut sample, position.clamp(0.0, 1.0), 0.0, start, end);
        sample[0]
    }

    /// `Ok(position)` — this segment filled `out` and is still running.
    /// `Err(written)` — this segment ended after `written` samples (`0` if it had none left).
    ///
    /// `duration` is the seconds this segment lasts. `seconds_per_sample` is `1 / sample_rate`.
    /// Playback can stretch `duration` past the stored `start.duration`.
    pub fn render_segment(
        out: &mut [Smp],
        position: Smp,
        seconds_per_sample: Smp,
        start: Self,
        end: Self,
        duration: Smp,
    ) -> Result<Smp, usize> {
        if duration <= 0.0 {
            return Err(0);
        }

        let (start, end) = local_segment(start, end, duration);

        SegPoint::render_segment(out, position, seconds_per_sample, start, end)
    }
}

fn local_segment(start: TimePoint, end: TimePoint, duration: Smp) -> (SegPoint, SegPoint) {
    (
        SegPoint {
            x: 0.0,
            y: start.y,
            power: start.power,
            smooth: start.smooth,
        },
        SegPoint {
            x: duration,
            y: end.y,
            ..SegPoint::default()
        },
    )
}

/// Breakpoint curve in seconds. Always holds two to [`MAX_POINTS`] points.
/// The first point is at time 0. Each point's duration is the seconds until the next
/// point, and the last duration is 0. Those durations sum to the curve's length.
/// Deserialization and [`sanitize`](Self::sanitize) replace a shorter curve with [`Default`].
#[derive(Clone, Debug, Serialize)]
pub struct TimeSegments {
    points: Vec<TimePoint>,
    /// Loop start. Any point except the last, and before [`release_point`](Self::release_point)
    /// when that is set. The envelope repeats from here.
    #[serde(skip_serializing_if = "Option::is_none")]
    loop_point: Option<usize>,
    /// Sustain while a key is held, and the point note-off jumps to.
    /// The release segment then leaves from the level playing at that moment.
    #[serde(skip_serializing_if = "Option::is_none")]
    release_point: Option<usize>,
    #[serde(skip)]
    revision: u64,
}

impl Default for TimeSegments {
    fn default() -> Self {
        Self::from_points(&[(0.0, 1.0), (1.0, 0.0)], false)
    }
}

impl TimeSegments {
    /// Build a curve from absolute `(seconds, level)` pairs. Every point shares `smooth`.
    /// Each time is seconds from the start. The gaps are stored as durations.
    ///
    /// # Panics
    ///
    /// Panics when `points` has fewer than two or more than [`MAX_POINTS`] entries.
    pub fn from_points(points: &[(Smp, Smp)], smooth: bool) -> Self {
        assert!(
            (MIN_POINTS..=MAX_POINTS).contains(&points.len()),
            "time segments must have {MIN_POINTS} to {MAX_POINTS} points"
        );

        let times = absolute_times(points.iter().map(|&(time, _)| time));
        let mut curve = Vec::with_capacity(MAX_POINTS);

        for (index, &(_, y)) in points.iter().enumerate() {
            curve.push(TimePoint {
                duration: duration_at(&times, index),
                y,
                smooth,
                ..TimePoint::default()
            });
        }

        let mut segments = Self {
            points: curve,
            loop_point: None,
            release_point: None,
            revision: Self::next_revision(),
        };

        segments.sanitize();
        segments
    }

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
            .iter()
            .take(index)
            .map(|point| point.duration.max(0.0))
            .sum()
    }

    /// Seconds from point `index` to the next point. The last point has no segment.
    pub fn duration(&self, index: usize) -> Smp {
        if index >= self.num_segments() {
            return 0.0;
        }

        self.points[index].duration.max(0.0)
    }

    pub fn total_time(&self) -> Smp {
        self.time(self.num_points() - 1)
    }

    pub fn power(&self, index: usize) -> Smp {
        self.points[index].power
    }

    /// Replace breakpoint `index`. `point.duration` is the seconds until the next
    /// breakpoint, and a change there shifts every later point. The last duration stays 0.
    pub fn set_point(&mut self, index: usize, mut point: TimePoint) {
        let n = self.num_points();

        if index >= n {
            return;
        }

        point.y = sanitize(point.y, 0.0, 1.0);
        point.power = sanitize(point.power, -MAX_POWER, MAX_POWER);
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
            point.y = sanitize(point.y, 0.0, 1.0);
            point.power = sanitize(point.power, -MAX_POWER, MAX_POWER);

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

    /// Insert a breakpoint at absolute `time` seconds. The start point stays put.
    /// Fails when the curve is full or `index` is the start. `point.duration` is ignored.
    ///
    /// A point inserted between two breakpoints splits that segment, so later durations
    /// and the total length stay put. A point appended after the end lengthens the
    /// previous point's duration.
    pub fn add_point(&mut self, index: usize, time: Smp, mut point: TimePoint) -> bool {
        let n = self.num_points();

        if n >= MAX_POINTS || index == 0 || index > n {
            return false;
        }

        point.y = sanitize(point.y, 0.0, 1.0);
        point.power = sanitize(point.power, -MAX_POWER, MAX_POWER);

        let previous = self.time(index - 1);

        if index == n {
            let end = MAX_TIME.max(previous);
            let time = sanitize(time, 0.0, MAX_TIME).clamp(previous, end);

            self.points[index - 1].duration = time - previous;
            point.duration = 0.0;
            self.points.push(point);
        } else {
            let next = self.time(index);
            let time = sanitize(time, 0.0, MAX_TIME).clamp(previous, next);

            self.points[index - 1].duration = time - previous;
            point.duration = (next - time).max(0.0);
            self.points.insert(index, point);
        }

        self.shift_inserted(index);
        self.bump();
        true
    }

    /// Drop an interior breakpoint. The first and last points stay.
    /// The previous segment takes the removed duration, so later points keep their time.
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

    pub fn flip_horizontal(&mut self) {
        let n = self.num_points();

        for i in 0..n / 2 {
            let j = n - 1 - i;
            let y = self.points[i].y;

            self.points[i].y = self.points[j].y;
            self.points[j].y = y;
        }

        let segments = self.num_segments();

        for i in 0..segments / 2 {
            let j = segments - 1 - i;
            let duration = self.points[i].duration;

            self.points[i].duration = self.points[j].duration;
            self.points[j].duration = duration;
        }

        self.points[n - 1].duration = 0.0;

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

    /// Seconds on the stored curve at `position` through the segment that starts at `index`.
    /// An index on or past the last segment is the end of the curve.
    pub fn time_at(&self, index: usize, position: Smp) -> Smp {
        if index >= self.num_segments() {
            return self.total_time();
        }

        self.time(index) + position.clamp(0.0, 1.0) * self.duration(index)
    }

    /// Level at `time` seconds. Times past the end hold the last level.
    pub fn value_at(&self, time: Smp) -> Smp {
        let n = self.num_points();
        let time = if time.is_finite() { time } else { 0.0 };

        if time <= 0.0 {
            return self.points[0].y;
        }

        let mut start = 0.0;

        for index in 0..n - 1 {
            let duration = self.duration(index);
            let end = start + duration;

            if duration <= 0.0 {
                continue;
            }

            if time <= end {
                let position = (time - start) / duration;

                return TimePoint::level_between(
                    self.point(index),
                    self.point(index + 1),
                    position,
                );
            }

            start = end;
        }

        self.points[n - 1].y
    }

    /// Polyline for `pixels` columns across `span` seconds. Each point is `[seconds, level]`.
    ///
    /// A vertical step is the two breakpoints that share a time.
    pub fn render_points(
        &self,
        pixels: usize,
        span: Smp,
        points: &mut Vec<[Smp; 2]>,
        scratch: &mut Vec<Smp>,
    ) {
        points.clear();

        let n = self.num_points();

        if pixels < 1 || span <= 0.0 {
            return;
        }

        scratch.resize(pixels + 1, 0.0);

        let mut from = 0;
        let mut start = 0.0;

        for index in 0..n - 1 {
            let duration = self.duration(index);
            let end = start + duration;
            let left = self.point(index);
            let right = self.point(index + 1);
            let x0 = start / span;
            let x1 = end / span;

            if duration <= 0.0 || x1 <= x0 {
                Self::push_point(points, [start, left.y]);
                Self::push_point(points, [end, right.y]);
                start = end;
                continue;
            }

            Self::push_point(points, [start, left.y]);

            let to = sample_end(x1.min(1.0), pixels);

            if from >= to {
                start = end;
                continue;
            }

            // Columns are real seconds. A segment that runs past the window keeps its
            // length; only the visible part is drawn. Scaling by the visible column
            // count would play the rest of the curve early and leave it flat.
            let seconds_per_sample = span / pixels as Smp;
            let position =
                ((from as Smp * seconds_per_sample - start) / duration).clamp(0.0, 1.0);
            let count = to - from;
            let written = match TimePoint::render_segment(
                &mut scratch[from..to],
                position,
                seconds_per_sample,
                left,
                right,
                duration,
            ) {
                Ok(_) => count,
                Err(written) => written,
            };

            for (offset, &y) in scratch[from..from + written].iter().enumerate() {
                let time = (from + offset) as Smp * seconds_per_sample;

                Self::push_point(points, [time, y]);
            }

            from = to;
            start = end;
        }

        Self::push_point(points, [self.total_time(), self.points[n - 1].y]);
    }

    fn push_point(points: &mut Vec<[Smp; 2]>, point: [Smp; 2]) {
        if points.last() != Some(&point) {
            points.push(point);
        }
    }
}

/// Seconds shown across an editor. The curve fills 80% of the view, at least [`MIN_VIEW`],
/// and never more than [`MAX_TIME`].
pub fn view_span(total: Smp) -> Smp {
    (total / VIEW_FILL).clamp(MIN_VIEW, MAX_TIME)
}

/// Exclusive end index of the span that finishes at normalized `x` on `steps` intervals.
fn sample_end(x: Smp, steps: usize) -> usize {
    if x <= 0.0 {
        return 0;
    }

    ((x * steps as Smp).floor() + 1.0).min(steps as Smp + 1.0) as usize
}

#[derive(Deserialize)]
struct RawTimePoint {
    #[serde(default)]
    time: Option<Smp>,
    #[serde(default)]
    duration: Option<Smp>,
    #[serde(default)]
    y: Smp,
    #[serde(default)]
    power: Smp,
    #[serde(default)]
    smooth: bool,
}

#[derive(Deserialize)]
struct RawTimeSegments {
    points: Vec<RawTimePoint>,
    #[serde(default)]
    loop_point: Option<usize>,
    #[serde(default)]
    release_point: Option<usize>,
}

impl<'de> Deserialize<'de> for TimeSegments {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = RawTimeSegments::deserialize(deserializer)?;
        let mut segments = Self {
            points: points_from_raw(raw.points),
            loop_point: raw.loop_point,
            release_point: raw.release_point,
            revision: 0,
        };

        segments.sanitize();
        Ok(segments)
    }
}

/// Absolute times, pinned at 0, clamped, and kept in order.
fn absolute_times(times: impl IntoIterator<Item = Smp>) -> Vec<Smp> {
    let mut ordered = Vec::new();
    let mut previous = 0.0;

    for (index, time) in times.into_iter().enumerate() {
        let time = if index == 0 {
            0.0
        } else {
            sanitize(time, 0.0, MAX_TIME).max(previous)
        };

        previous = time;
        ordered.push(time);
    }

    ordered
}

fn duration_at(times: &[Smp], index: usize) -> Smp {
    if index + 1 >= times.len() {
        0.0
    } else {
        times[index + 1] - times[index]
    }
}

fn points_from_raw(raw: Vec<RawTimePoint>) -> Vec<TimePoint> {
    let legacy = raw.iter().all(|point| point.duration.is_none());

    if !legacy {
        return raw
            .into_iter()
            .map(|point| TimePoint {
                duration: point.duration.unwrap_or(0.0),
                y: point.y,
                power: point.power,
                smooth: point.smooth,
            })
            .collect();
    }

    let times = absolute_times(raw.iter().map(|point| point.time.unwrap_or(0.0)));

    raw.into_iter()
        .enumerate()
        .map(|(index, point)| TimePoint {
            duration: duration_at(&times, index),
            y: point.y,
            power: point.power,
            smooth: point.smooth,
        })
        .collect()
}

#[cfg(test)]
mod tests;
