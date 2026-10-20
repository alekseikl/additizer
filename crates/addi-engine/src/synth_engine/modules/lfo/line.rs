use serde::{Deserialize, Serialize};

use crate::synth_engine::{Smp, power_scale};

/// Most breakpoints a curve can hold. Matches Vital's `LineGenerator::kMaxPoints`.
pub const MAX_POINTS: usize = 100;
/// Bend range for a segment. Matches Vital's `SynthLfo::kMaxPower`.
pub const MAX_POWER: Smp = 20.0;

const MIN_POINTS: usize = 2;

/// One breakpoint. `x` is phase in `[0, 1]`, `y` is the output level in `[0, 1]`
/// (`1` is the top of the editor).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LfoPoint {
    pub x: Smp,
    pub y: Smp,
}

/// Breakpoint curve for one LFO cycle, in the same form as Vital's `LineGenerator`.
///
/// Vital stores Y with `0` at the top of the widget. Points here use the output level,
/// so that axis is flipped and the baked table stores `y` directly.
///
/// Each point owns the bend (`power`) of the segment that starts at that point.
/// `smooth` runs every segment through a sine ease before the bend. The first point
/// stays at phase `0` and the last at phase `1`. The cycle loops.
#[derive(Clone, Copy, Debug)]
pub struct LfoLine {
    points: [LfoPoint; MAX_POINTS],
    powers: [Smp; MAX_POINTS],
    num_points: u8,
    smooth: bool,
}

impl PartialEq for LfoLine {
    fn eq(&self, other: &Self) -> bool {
        let n = self.num_points();

        self.num_points == other.num_points
            && self.smooth == other.smooth
            && self.points[..n] == other.points[..n]
            && self.powers[..n] == other.powers[..n]
    }
}

impl Eq for LfoLine {}

impl Default for LfoLine {
    fn default() -> Self {
        Self::triangle()
    }
}

impl LfoLine {
    pub fn linear() -> Self {
        Self::from_points(&[(0.0, 0.0), (1.0, 1.0)], false)
    }

    pub fn triangle() -> Self {
        Self::triangle_at(0.5)
    }

    pub fn square() -> Self {
        Self::from_points(
            &[(0.0, 0.0), (0.0, 1.0), (0.5, 1.0), (0.5, 0.0), (1.0, 0.0)],
            false,
        )
    }

    pub fn sine() -> Self {
        Self::sine_at(0.5)
    }

    /// Rising ramp that drops back to `0` at the loop point.
    pub fn saw_up() -> Self {
        Self::from_points(&[(0.0, 0.0), (1.0, 1.0), (1.0, 0.0)], false)
    }

    /// Falling ramp that jumps back to `1` at the loop point.
    pub fn saw_down() -> Self {
        Self::from_points(&[(0.0, 1.0), (1.0, 0.0), (1.0, 1.0)], false)
    }

    /// Triangle whose peak sits at `skew`, matching the old shape plus skew control.
    pub(crate) fn triangle_at(skew: Smp) -> Self {
        let skew = skew.clamp(0.0, 1.0);

        if skew <= 0.0 {
            return Self::from_points(&[(0.0, 1.0), (1.0, 0.0)], false);
        }

        if skew >= 1.0 {
            return Self::from_points(&[(0.0, 0.0), (1.0, 1.0)], false);
        }

        Self::from_points(&[(0.0, 0.0), (skew, 1.0), (1.0, 0.0)], false)
    }

    /// High until `skew`, then low. Matches the old square plus skew control.
    pub(crate) fn square_at(skew: Smp) -> Self {
        let skew = skew.clamp(0.0, 1.0);

        if skew <= 0.0 {
            return Self::from_points(&[(0.0, 0.0), (1.0, 0.0)], false);
        }

        if skew >= 1.0 {
            return Self::from_points(&[(0.0, 1.0), (1.0, 1.0)], false);
        }

        Self::from_points(&[(0.0, 1.0), (skew, 1.0), (skew, 0.0), (1.0, 0.0)], false)
    }

    /// Smooth triangle peaked at `skew`. This is the old sine plus skew control.
    pub(crate) fn sine_at(skew: Smp) -> Self {
        let mut line = Self::triangle_at(skew);
        line.smooth = true;
        line
    }

    pub fn num_points(&self) -> usize {
        self.num_points as usize
    }

    pub fn point(&self, index: usize) -> LfoPoint {
        self.points[index]
    }

    pub fn points(&self) -> &[LfoPoint] {
        &self.points[..self.num_points()]
    }

    pub fn power(&self, index: usize) -> Smp {
        self.powers[index]
    }

    pub fn smooth(&self) -> bool {
        self.smooth
    }

    pub fn set_smooth(&mut self, smooth: bool) {
        self.smooth = smooth;
    }

    pub fn set_point(&mut self, index: usize, mut point: LfoPoint) {
        let n = self.num_points();

        if index >= n {
            return;
        }

        point.x = finite_unit(point.x, self.points[index].x);
        point.y = finite_unit(point.y, self.points[index].y);

        if index == 0 {
            point.x = 0.0;
        } else if index + 1 == n {
            point.x = 1.0;
        }

        self.points[index] = point;
    }

    pub fn set_power(&mut self, index: usize, power: Smp) {
        if index >= self.num_points() {
            return;
        }

        self.powers[index] = if power.is_finite() {
            power.clamp(-MAX_POWER, MAX_POWER)
        } else {
            0.0
        };
    }

    pub fn reset_powers(&mut self) {
        let n = self.num_points();
        self.powers[..n].fill(0.0);
    }

    /// Insert `point` so it stays in phase order. Fails when the curve is full.
    pub fn add_point(&mut self, index: usize, mut point: LfoPoint) -> bool {
        let n = self.num_points();

        if n >= MAX_POINTS || index > n || !point.x.is_finite() || !point.y.is_finite() {
            return false;
        }

        point.x = point.x.clamp(0.0, 1.0);
        point.y = point.y.clamp(0.0, 1.0);

        for i in (index..n).rev() {
            self.points[i + 1] = self.points[i];
            self.powers[i + 1] = self.powers[i];
        }

        if index == 0 {
            point.x = 0.0;
        }

        if index == n {
            point.x = 1.0;
        }

        self.points[index] = point;
        self.powers[index] = 0.0;
        self.num_points += 1;
        true
    }

    /// Drop an interior point. The phase `0` and phase `1` endpoints stay.
    pub fn remove_point(&mut self, index: usize) -> bool {
        let n = self.num_points();

        if n <= MIN_POINTS || index == 0 || index + 1 == n {
            return false;
        }

        for i in index..n - 1 {
            self.points[i] = self.points[i + 1];
            self.powers[i] = self.powers[i + 1];
        }

        self.num_points -= 1;
        let discarded = self.num_points();
        self.points[discarded].y = 0.0;
        self.powers[discarded] = 0.0;
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
            let tmp = self.powers[i];
            let j = n - i - 2;

            self.powers[i] = -self.powers[j];
            self.powers[j] = -tmp;
        }
    }

    pub fn flip_vertical(&mut self) {
        let n = self.num_points();

        for point in &mut self.points[..n] {
            point.y = 1.0 - point.y;
        }
    }

    /// Level at `phase` along the segment that contains it.
    pub fn value_at(&self, phase: Smp) -> Smp {
        let n = self.num_points();

        if n == 0 {
            return 0.0;
        }

        let phase = if phase.is_finite() {
            phase.clamp(0.0, 1.0)
        } else {
            0.0
        };

        for i in 0..n - 1 {
            let start = self.points[i];
            let end = self.points[i + 1];

            if start.x <= phase && end.x >= phase {
                return self.value_between(phase, i, i + 1);
            }
        }

        self.points[n - 1].y
    }

    pub fn value_between(&self, x: Smp, index_from: usize, index_to: usize) -> Smp {
        let first = self.points[index_from];
        let second = self.points[index_to];
        let width = second.x - first.x;

        if width <= 0.0 {
            return second.y;
        }

        let mut t = ((x - first.x) / width).clamp(0.0, 1.0);

        if self.smooth {
            t = smooth_transition(t);
        }

        t = power_scale(t, self.powers[index_from]).clamp(0.0, 1.0);
        t.mul_add(second.y - first.y, first.y)
    }

    fn from_points(points: &[(Smp, Smp)], smooth: bool) -> Self {
        debug_assert!(points.len() >= MIN_POINTS && points.len() <= MAX_POINTS);

        let mut line = Self {
            points: [LfoPoint { x: 0.0, y: 0.0 }; MAX_POINTS],
            powers: [0.0; MAX_POINTS],
            num_points: points.len() as u8,
            smooth,
        };

        for (i, &(x, y)) in points.iter().enumerate() {
            line.points[i] = LfoPoint { x, y };
        }

        line
    }

    fn sanitize(&mut self) {
        let n = self.num_points().clamp(MIN_POINTS, MAX_POINTS);

        if self.points[..n]
            .iter()
            .any(|point| !point.x.is_finite() || !point.y.is_finite())
            || self.powers[..n].iter().any(|power| !power.is_finite())
        {
            *self = Self::triangle();
            return;
        }

        let mut order = [0usize; MAX_POINTS];

        for (i, slot) in order.iter_mut().enumerate() {
            *slot = i;
        }

        order[..n].sort_by(|&a, &b| self.points[a].x.total_cmp(&self.points[b].x));

        let points = self.points;
        let powers = self.powers;

        for (i, &src) in order[..n].iter().enumerate() {
            self.points[i] = points[src];
            self.powers[i] = powers[src].clamp(-MAX_POWER, MAX_POWER);
            self.points[i].x = self.points[i].x.clamp(0.0, 1.0);
            self.points[i].y = self.points[i].y.clamp(0.0, 1.0);
        }

        self.points[0].x = 0.0;
        self.points[n - 1].x = 1.0;
        self.num_points = n as u8;
    }
}

fn finite_unit(value: Smp, fallback: Smp) -> Smp {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        fallback
    }
}

fn smooth_transition(t: Smp) -> Smp {
    0.5 * ((t - 0.5) * std::f32::consts::PI).sin() + 0.5
}

#[derive(Serialize, Deserialize)]
struct RawLine {
    points: Vec<[Smp; 2]>,
    powers: Vec<Smp>,
    #[serde(default)]
    smooth: bool,
}

impl Serialize for LfoLine {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let n = self.num_points();
        let raw = RawLine {
            points: self.points[..n]
                .iter()
                .map(|point| [point.x, point.y])
                .collect(),
            powers: self.powers[..n].to_vec(),
            smooth: self.smooth,
        };

        raw.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for LfoLine {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = RawLine::deserialize(deserializer)?;

        if raw.points.len() < MIN_POINTS {
            return Ok(Self::triangle());
        }

        let n = raw.points.len().min(MAX_POINTS);
        let mut line = Self {
            points: [LfoPoint { x: 0.0, y: 0.0 }; MAX_POINTS],
            powers: [0.0; MAX_POINTS],
            num_points: n as u8,
            smooth: raw.smooth,
        };

        for (i, point) in raw.points.iter().take(n).enumerate() {
            line.points[i] = LfoPoint {
                x: point[0],
                y: point[1],
            };
            line.powers[i] = raw.powers.get(i).copied().unwrap_or(0.0);
        }

        line.sanitize();
        Ok(line)
    }
}
