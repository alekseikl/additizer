use serde::{Deserialize, Serialize};

use crate::{Smp, power_scale};

/// Most breakpoints a curve can hold. Matches Vital's `LineGenerator::kMaxPoints`.
pub const MAX_POINTS: usize = 100;
/// Bend range for a segment. Matches Vital's `SynthLfo::kMaxPower`.
pub const MAX_POWER: Smp = 20.0;

const MIN_POINTS: usize = 2;

/// One breakpoint. `x` is phase in `[0, 1]`, `y` is the output level in `[0, 1]`
/// (`1` is the top of the editor).
///
/// `power` and `smooth` belong to the segment that starts at this point. `power`
/// bends that segment; `smooth` eases it with a sine curve before the bend.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub x: Smp,
    pub y: Smp,
    pub power: Smp,
    pub smooth: bool,
}

impl Default for Point {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            power: 0.0,
            smooth: false,
        }
    }
}

/// Breakpoint segments for one cycle, in the same form as Vital's `LineGenerator`.
///
/// Vital stores Y with `0` at the top of the widget. Points here use the output level,
/// so that axis is flipped and the baked table stores `y` directly.
///
/// The first point stays at phase `0` and the last at phase `1`. The cycle loops.
#[derive(Clone, Copy, Debug)]
pub struct Segments {
    points: [Point; MAX_POINTS],
    num_points: u8,
}

impl Default for Segments {
    fn default() -> Self {
        Self::triangle()
    }
}

impl Segments {
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
    pub fn triangle_at(skew: Smp) -> Self {
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
    pub fn square_at(skew: Smp) -> Self {
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
    pub fn sine_at(skew: Smp) -> Self {
        let mut segments = Self::triangle_at(skew);
        let n = segments.num_points();

        for point in &mut segments.points[..n] {
            point.smooth = true;
        }

        segments
    }

    pub fn num_points(&self) -> usize {
        self.num_points as usize
    }

    pub fn point(&self, index: usize) -> Point {
        self.points[index]
    }

    pub fn points(&self) -> &[Point] {
        &self.points[..self.num_points()]
    }

    pub fn power(&self, index: usize) -> Smp {
        self.points[index].power
    }

    pub fn set_point(&mut self, index: usize, mut point: Point) {
        let n = self.num_points();

        if index >= n {
            return;
        }

        point.x = finite_unit(point.x, self.points[index].x);
        point.y = finite_unit(point.y, self.points[index].y);
        point.power = clamp_power(point.power);

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

        self.points[index].power = clamp_power(power);
    }

    pub fn set_smooth(&mut self, index: usize, smooth: bool) {
        if index >= self.num_points() {
            return;
        }

        self.points[index].smooth = smooth;
    }

    pub fn reset_powers(&mut self) {
        let n = self.num_points();

        for point in &mut self.points[..n] {
            point.power = 0.0;
        }
    }

    /// Insert `point` so it stays in phase order. Fails when the curve is full.
    pub fn add_point(&mut self, index: usize, mut point: Point) -> bool {
        let n = self.num_points();

        if n >= MAX_POINTS || index > n || !point.x.is_finite() || !point.y.is_finite() {
            return false;
        }

        point.x = point.x.clamp(0.0, 1.0);
        point.y = point.y.clamp(0.0, 1.0);

        point.power = clamp_power(point.power);

        for i in (index..n).rev() {
            self.points[i + 1] = self.points[i];
        }

        if index == 0 {
            point.x = 0.0;
        }

        if index == n {
            point.x = 1.0;
        }

        self.points[index] = point;
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
        }

        self.num_points -= 1;
        self.points[self.num_points()] = Point::default();
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

        if first.smooth {
            t = smooth_transition(t);
        }

        t = power_scale(t, first.power).clamp(0.0, 1.0);
        t.mul_add(second.y - first.y, first.y)
    }

    fn from_points(points: &[(Smp, Smp)], smooth: bool) -> Self {
        debug_assert!(points.len() >= MIN_POINTS && points.len() <= MAX_POINTS);

        let mut segments = Self {
            points: [Point::default(); MAX_POINTS],
            num_points: points.len() as u8,
        };

        for (i, &(x, y)) in points.iter().enumerate() {
            segments.points[i] = Point {
                x,
                y,
                smooth,
                ..Point::default()
            };
        }

        segments
    }

    fn sanitize(&mut self) {
        let n = self.num_points().clamp(MIN_POINTS, MAX_POINTS);

        if self.points[..n]
            .iter()
            .any(|point| !point.x.is_finite() || !point.y.is_finite() || !point.power.is_finite())
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

        for (i, &src) in order[..n].iter().enumerate() {
            self.points[i] = points[src];
            self.points[i].x = self.points[i].x.clamp(0.0, 1.0);
            self.points[i].y = self.points[i].y.clamp(0.0, 1.0);
            self.points[i].power = self.points[i].power.clamp(-MAX_POWER, MAX_POWER);
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

fn clamp_power(power: Smp) -> Smp {
    if power.is_finite() {
        power.clamp(-MAX_POWER, MAX_POWER)
    } else {
        0.0
    }
}

fn smooth_transition(t: Smp) -> Smp {
    0.5 * ((t - 0.5) * std::f32::consts::PI).sin() + 0.5
}

#[derive(Serialize)]
struct SavedPoint {
    x: Smp,
    y: Smp,
    power: Smp,
    smooth: bool,
}

#[derive(Serialize)]
struct SavedSegments {
    points: Vec<SavedPoint>,
}

#[derive(Deserialize)]
struct RawSegments {
    points: Vec<RawPoint>,
    /// Presets saved when bend lived beside the points.
    #[serde(default)]
    powers: Vec<Smp>,
    /// Presets saved when one flag eased every segment.
    #[serde(default)]
    smooth: bool,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RawPoint {
    Pair([Smp; 2]),
    Full {
        x: Smp,
        y: Smp,
        #[serde(default)]
        power: Smp,
        #[serde(default)]
        smooth: bool,
    },
}

impl Serialize for Segments {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let n = self.num_points();
        let raw = SavedSegments {
            points: self.points[..n]
                .iter()
                .map(|point| SavedPoint {
                    x: point.x,
                    y: point.y,
                    power: point.power,
                    smooth: point.smooth,
                })
                .collect(),
        };

        raw.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Segments {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = RawSegments::deserialize(deserializer)?;

        if raw.points.len() < MIN_POINTS {
            return Ok(Self::triangle());
        }

        let n = raw.points.len().min(MAX_POINTS);
        let mut segments = Self {
            points: [Point::default(); MAX_POINTS],
            num_points: n as u8,
        };

        for (i, point) in raw.points.into_iter().take(n).enumerate() {
            segments.points[i] = match point {
                RawPoint::Pair([x, y]) => Point {
                    x,
                    y,
                    power: raw.powers.get(i).copied().unwrap_or(0.0),
                    smooth: raw.smooth,
                },
                RawPoint::Full {
                    x,
                    y,
                    power,
                    smooth,
                } => Point {
                    x,
                    y,
                    power,
                    smooth,
                },
            };
        }

        segments.sanitize();
        Ok(segments)
    }
}

#[cfg(test)]
mod tests {
    use super::{MAX_POINTS, Point, Segments};
    use crate::Smp;

    fn skew_arg(arg: Smp, skew: Smp) -> Smp {
        let arg_less = arg < skew;

        Smp::from(arg_less) * (0.5 * arg / skew.max(Smp::EPSILON))
            + Smp::from(!arg_less) * (0.5 + (arg - skew) * 0.5 / (1.0 - skew).max(Smp::EPSILON))
    }

    fn legacy_triangle(t: Smp, skew: Smp) -> Smp {
        let x = skew_arg(t, skew);
        2.0 * x.min(1.0 - x)
    }

    fn legacy_square(t: Smp, skew: Smp) -> Smp {
        Smp::from(skew_arg(t, skew) < 0.5)
    }

    fn legacy_sine(t: Smp, skew: Smp) -> Smp {
        let sine = (std::f32::consts::PI * skew_arg(t, skew)).sin();
        sine * sine
    }

    #[test]
    fn roundtrips_and_falls_back_when_empty() {
        let mut segments = Segments::triangle();
        segments.set_power(0, 4.0);
        segments.set_smooth(0, true);

        let json = serde_json::to_value(segments).unwrap();
        let back: Segments = serde_json::from_value(json).unwrap();

        assert_eq!(back.points(), segments.points());

        let empty = serde_json::json!({ "points": [], "powers": [], "smooth": true });
        let back: Segments = serde_json::from_value(empty).unwrap();

        assert_eq!(back.points(), Segments::triangle().points());

        let legacy = serde_json::json!({
            "points": [[0.0, 0.0], [1.0, 1.0]],
            "powers": [4.0],
            "smooth": true
        });
        let back: Segments = serde_json::from_value(legacy).unwrap();

        assert_eq!(back.point(0).power, 4.0);
        assert!(back.point(0).smooth);
        assert!(back.point(1).smooth);
        assert_eq!(back.point(1).power, 0.0);
    }

    #[test]
    fn smooth_applies_only_to_the_segment_that_starts_at_that_point() {
        let mut segments = Segments::triangle();
        let plain = Segments::triangle();

        segments.set_smooth(0, true);

        assert!((segments.value_at(0.125) - plain.value_at(0.125)).abs() > 1e-3);
        assert!((segments.value_at(0.75) - plain.value_at(0.75)).abs() < 1e-5);
    }

    #[test]
    fn legacy_shapes_match_the_old_curves() {
        let skew = 0.3;
        let triangle = Segments::triangle_at(skew);
        let sine = Segments::sine_at(skew);
        let square = Segments::square_at(0.15);

        for index in 0..2048 {
            let t = index as Smp / 2048.0;

            assert!(
                (triangle.value_at(t) - legacy_triangle(t, skew)).abs() < 1e-5,
                "triangle {t}"
            );
            assert!(
                (sine.value_at(t) - legacy_sine(t, skew)).abs() < 1e-5,
                "sine {t}"
            );
            assert!(
                (square.value_at(t) - legacy_square(t, 0.15)).abs() < 1e-5,
                "square {t}"
            );
        }
    }

    #[test]
    fn positive_power_holds_the_segment_near_its_start() {
        let mut segments = Segments::linear();

        assert!((segments.value_at(0.5) - 0.5).abs() < 1e-6);

        segments.set_power(0, 8.0);

        assert!(segments.value_at(0.5) < 0.5);
    }

    #[test]
    fn edits_keep_endpoints_and_reject_a_full_curve() {
        let mut segments = Segments::triangle();

        assert!(segments.add_point(
            2,
            Point {
                x: 0.75,
                y: 0.25,
                ..Point::default()
            }
        ));
        assert_eq!(segments.num_points(), 4);
        assert_eq!(segments.point(2).x, 0.75);
        assert!(!segments.remove_point(0));
        assert!(segments.remove_point(2));
        assert_eq!(segments.num_points(), 3);

        segments.flip_vertical();

        assert_eq!(segments.point(0).y, 1.0);
        assert_eq!(segments.point(1).y, 0.0);
        assert_eq!(segments.point(2).y, 1.0);

        let mut full = Segments::linear();

        for i in 0..MAX_POINTS - 2 {
            assert!(full.add_point(
                1,
                Point {
                    x: (i + 1) as Smp / MAX_POINTS as Smp,
                    y: 0.5,
                    ..Point::default()
                }
            ));
        }

        assert!(!full.add_point(
            1,
            Point {
                x: 0.5,
                y: 0.5,
                ..Point::default()
            }
        ));
        assert_eq!(full.num_points(), MAX_POINTS);
    }
}
