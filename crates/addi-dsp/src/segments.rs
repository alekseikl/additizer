use serde::{Deserialize, Serialize};

use crate::{Smp, power_scale, utils::sanitize};

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

    /// Level at `phase` along the segment that contains it.
    pub fn value_at(&self, phase: Smp) -> Smp {
        let n = self.num_segments();

        if n == 0 {
            return 0.0;
        }

        let phase = if phase.is_finite() {
            phase.clamp(0.0, 1.0)
        } else {
            0.0
        };

        for i in 0..n - 1 {
            let start = self.segments[i];
            let end = self.segments[i + 1];

            if start.x <= phase && end.x >= phase {
                return self.value_between(phase, i, i + 1);
            }
        }

        self.segments[n - 1].y
    }

    /// Write one cycle into `out` at phases `i / out.len()`, the range `[0, 1)`.
    pub fn render(&self, out: &mut [Smp]) {
        let len = out.len();

        if len == 0 {
            return;
        }

        let n = self.num_segments();

        if n < MIN_POINTS {
            return;
        }

        debug_assert_eq!(self.segments[0].x, 0.0);
        debug_assert_eq!(self.segments[n - 1].x, 1.0);

        let len_f = len as Smp;
        let mut from = 0;

        for pair in self.segments.windows(2) {
            let to = samples_until(pair[1].x, len, len_f);

            if from >= to {
                continue;
            }

            render_segment(&mut out[from..to], from, len, pair[0], pair[1]);
            from = to;
        }

        debug_assert_eq!(from, len);
    }

    pub fn value_between(&self, x: Smp, index_from: usize, index_to: usize) -> Smp {
        segment_value(x, self.segments[index_from], self.segments[index_to])
    }
}

/// First index `i` whose phase `i / len` is past `phase`.
///
/// A sample that lands on `phase` belongs to the segment that ends there.
fn samples_until(phase: Smp, len: usize, len_f: Smp) -> usize {
    let scaled = phase * len_f;

    if scaled >= len_f {
        len
    } else {
        (scaled.floor().max(0.0) as usize + 1).min(len)
    }
}

fn render_segment(out: &mut [Smp], start_index: usize, len: usize, start: Segment, end: Segment) {
    let len_f = len as Smp;

    for (offset, sample) in out.iter_mut().enumerate() {
        let phase = (start_index + offset) as Smp / len_f;
        *sample = segment_value(phase, start, end);
    }
}

fn segment_value(phase: Smp, start: Segment, end: Segment) -> Smp {
    let width = end.x - start.x;

    if width <= 0.0 {
        return end.y;
    }

    let mut t = ((phase - start.x) / width).clamp(0.0, 1.0);

    if start.smooth {
        t = 0.5 * ((t - 0.5) * std::f32::consts::PI).sin() + 0.5;
    }

    t = power_scale(t, start.power).clamp(0.0, 1.0);
    t.mul_add(end.y - start.y, start.y)
}

#[cfg(test)]
mod tests;
