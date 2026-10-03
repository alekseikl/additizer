use std::{
    f32::consts::{PI, TAU},
    ops::Range,
};

use egui::{Color32, Mesh, Painter, Pos2, Rect, Shape, ecolor::Hsva};

use crate::editor::utils::hsva;
use addi_dsp::gain_to_db;
use addi_engine::{
    ComplexSample, DC_OFFSET, MAX_LEVEL_DB, MIN_LEVEL_DB, harmonic_editor::sawtooth_phase,
};

#[cfg(test)]
mod tests;

const BAR_WIDTH: f32 = 1.0;
/// Adjacent columns closer than this, in points, get a sloped top toward the next column.
const SLOPE_HEIGHT: f32 = 2.0;

const DB_SPAN: f32 = MAX_LEVEL_DB - MIN_LEVEL_DB;
const ZERO_DB_LEVEL: f32 = -MIN_LEVEL_DB / DB_SPAN;

/// Above 0 dB, independent of phase.
const AMPLIFIED_COLOR: Hsva = hsva(0.02, 0.88, 0.55, 1.0);

/// Below 0 dB when phase sits on sawtooth (angle zero).
const PHASE_0_COLOR: Hsva = hsva(0.567, 1.0, 0.35, 1.0);
/// 90° from sawtooth.
const PHASE_90_COLOR: Hsva = hsva(1.0 / 3.0, 1.0, 0.5, 1.0);
/// 180° from sawtooth.
const PHASE_180_COLOR: Hsva = hsva(0.0, 1.0, 0.5, 1.0);
/// 270° from sawtooth.
const PHASE_270_COLOR: Hsva = hsva(1.0 / 6.0, 1.0, 0.55, 1.0);

const PLACEHOLDER_BARS: usize = 128;
const PLACEHOLDER_COLOR: Color32 = Color32::from_rgb(36, 38, 50);

#[derive(Clone, Copy, Default)]
struct Column {
    level: f32,
    /// Turns from sawtooth phase, in `0..1`. Sawtooth is angle zero.
    phase: f32,
}

impl Column {
    fn new(harmonic: usize, bin: ComplexSample) -> Self {
        Self {
            level: level_from_bin(harmonic, bin),
            phase: phase_turns(harmonic, bin),
        }
    }
}

#[derive(Default)]
pub struct FrequencyBins {
    columns: Vec<Column>,
}

impl FrequencyBins {
    pub fn paint(&mut self, painter: &Painter, rect: Rect, spectrum: &[ComplexSample]) {
        let column_count = rect.width().floor() as usize;

        if !rect.is_positive() || column_count <= 1 {
            return;
        }

        let mut bars = BarMesh::new(rect, feather_width(painter));

        if self.accumulate_columns(spectrum, column_count) {
            self.paint_columns(&mut bars, rect);
        } else {
            Self::paint_placeholder(&mut bars, rect, column_count);
        }

        bars.finish(painter, rect);
    }

    /// Keeps the loudest bin of every column. Returns `false` when all columns are silent.
    fn accumulate_columns(&mut self, spectrum: &[ComplexSample], column_count: usize) -> bool {
        self.columns.clear();
        self.columns.resize(column_count, Column::default());

        if spectrum.len() <= DC_OFFSET {
            return false;
        }

        let ranges = column_ranges(spectrum.len() - 1, column_count);
        let mut has_level = false;

        for (column, range) in self.columns.iter_mut().zip(ranges) {
            if let Some(harmonic) = loudest_harmonic(spectrum, range) {
                *column = Column::new(harmonic, spectrum[harmonic]);
                has_level |= column.level > 0.0;
            }
        }

        has_level
    }

    fn paint_columns(&self, bars: &mut BarMesh, rect: Rect) {
        for (col, column) in self.columns.iter().enumerate() {
            if column.level <= 0.0 {
                continue;
            }

            // Slope the top toward a close neighbour instead of stepping.
            let next_level = self
                .columns
                .get(col + 1)
                .map(|next| next.level)
                .filter(|next| {
                    *next > 0.0 && within_slope_height(column.level, *next, rect.height())
                })
                .unwrap_or(column.level);

            bars.column(
                rect.left() + col as f32,
                column.level,
                next_level,
                attenuated_color(column.phase),
            );
        }
    }

    fn paint_placeholder(bars: &mut BarMesh, rect: Rect, column_count: usize) {
        let zero_y = bars.zero_y;

        for (col, range) in column_ranges(PLACEHOLDER_BARS, column_count).enumerate() {
            if !range.is_empty() {
                bars.flat_bar(rect.left() + col as f32, zero_y, PLACEHOLDER_COLOR);
            }
        }
    }
}

/// Harmonic ranges covered by each of `column_count` columns, spreading
/// `DC_OFFSET..=last_harmonic` over the columns on a log2 scale.
fn column_ranges(last_harmonic: usize, column_count: usize) -> impl Iterator<Item = Range<usize>> {
    debug_assert!(column_count > 1);

    let end = last_harmonic + 1;
    // `last_harmonic` lands on the last column. Infinite when there is a single harmonic,
    // which then fills the first column (`as usize` saturates).
    let octaves_per_column = if last_harmonic > DC_OFFSET {
        (last_harmonic as f32).log2() / (column_count - 1) as f32
    } else {
        f32::INFINITY
    };
    let mut lo = DC_OFFSET;

    (1..=column_count).map(move |col| {
        // First harmonic that rounds to `col`.
        let hi = ((col as f32 - 0.5) * octaves_per_column).exp2().ceil() as usize;
        let range = lo..hi.clamp(lo, end);
        lo = range.end;
        range
    })
}

/// Harmonic with the highest amplitude in `range`, if any of them is non-zero.
fn loudest_harmonic(spectrum: &[ComplexSample], range: Range<usize>) -> Option<usize> {
    let (harmonic, power) = range.fold((0, 0.0_f32), |best, harmonic| {
        // Orders like `level_from_bin` without the square root and logarithm.
        let power = spectrum[harmonic].norm_sqr() * (harmonic * harmonic) as f32;

        if power > best.1 {
            (harmonic, power)
        } else {
            best
        }
    });

    (power > 0.0).then_some(harmonic)
}

/// Normalized level in `0..=1` over the harmonic editor dB range; `0` at or below `MIN_LEVEL_DB`.
fn level_from_bin(harmonic: usize, bin: ComplexSample) -> f32 {
    let amplitude = bin.norm() * harmonic as f32 * PI;
    ((gain_to_db(amplitude) - MIN_LEVEL_DB) / DB_SPAN).clamp(0.0, 1.0)
}

/// Turns from sawtooth phase, in `0..1`. Sawtooth is angle zero.
fn phase_turns(harmonic: usize, bin: ComplexSample) -> f32 {
    let phase = (bin.arg() / TAU).rem_euclid(1.0);
    (phase - sawtooth_phase(harmonic)).rem_euclid(1.0)
}

/// Color at and below 0 dB for a phase offset in turns.
///
/// 0° is [`PHASE_0_COLOR`], then [`PHASE_90_COLOR`], [`PHASE_180_COLOR`], and
/// [`PHASE_270_COLOR`], blended in gamma between those stops. Above 0 dB stays [`AMPLIFIED_COLOR`].
fn attenuated_color(turns: f32) -> Color32 {
    let turns = turns.rem_euclid(1.0);
    let (from, to, t) = if turns < 0.25 {
        (PHASE_0_COLOR, PHASE_90_COLOR, turns * 4.0)
    } else if turns < 0.5 {
        (PHASE_90_COLOR, PHASE_180_COLOR, (turns - 0.25) * 4.0)
    } else if turns < 0.75 {
        (PHASE_180_COLOR, PHASE_270_COLOR, (turns - 0.5) * 4.0)
    } else {
        (PHASE_270_COLOR, PHASE_0_COLOR, (turns - 0.75) * 4.0)
    };

    Color32::from(from).lerp_to_gamma(Color32::from(to), t)
}

/// Screen-space height gap between two normalized levels is under [`SLOPE_HEIGHT`].
fn within_slope_height(level: f32, next: f32, plot_height: f32) -> bool {
    (level - next).abs() * plot_height < SLOPE_HEIGHT
}

/// Fraction along the top edge where a slope crosses `zero_y`.
fn slope_cross_t(left_y: f32, right_y: f32, zero_y: f32) -> f32 {
    ((zero_y - left_y) / (right_y - left_y)).clamp(0.0, 1.0)
}

/// Feather width in points: one physical pixel, same as egui's tessellator.
fn feather_width(painter: &Painter) -> f32 {
    let pixels_per_point = painter.pixels_per_point();

    if pixels_per_point <= 0.0 {
        return 0.0;
    }

    painter.ctx().tessellation_options(|options| {
        if options.feathering {
            options.feathering_size_in_pixels / pixels_per_point
        } else {
            0.0
        }
    })
}

/// Outer (transparent) and inner (opaque) y of a feather centered on `edge_y`.
fn feather_ys(edge_y: f32, floor: f32, feather: f32) -> (f32, f32) {
    let half = feather * 0.5;
    (edge_y - half, (edge_y + half).min(floor))
}

/// Accumulates bars sharing one baseline, 0 dB line and feather width into a single mesh.
struct BarMesh {
    mesh: Mesh,
    bottom: f32,
    height: f32,
    zero_y: f32,
    feather: f32,
}

impl BarMesh {
    fn new(rect: Rect, feather: f32) -> Self {
        Self {
            mesh: Mesh::default(),
            bottom: rect.bottom(),
            height: rect.height(),
            zero_y: rect.bottom() - ZERO_DB_LEVEL * rect.height(),
            feather,
        }
    }

    fn finish(self, painter: &Painter, clip: Rect) {
        if !self.mesh.vertices.is_empty() {
            painter.with_clip_rect(clip).add(Shape::mesh(self.mesh));
        }
    }

    /// One column whose top edge runs from `level` to `next_level`; [`AMPLIFIED_COLOR`] above 0 dB.
    fn column(&mut self, x: f32, level: f32, next_level: f32, attenuated: Color32) {
        let left_y = self.bottom - level * self.height;
        let right_y = self.bottom - next_level * self.height;
        let right = x + BAR_WIDTH;

        if (left_y < self.zero_y) != (right_y < self.zero_y) {
            let cross_x = x + slope_cross_t(left_y, right_y, self.zero_y) * BAR_WIDTH;
            self.segment(x, left_y, cross_x, self.zero_y, attenuated);
            self.segment(cross_x, self.zero_y, right, right_y, attenuated);
        } else {
            self.segment(x, left_y, right, right_y, attenuated);
        }
    }

    /// Part of a column whose top edge stays on one side of 0 dB.
    fn segment(&mut self, left: f32, left_y: f32, right: f32, right_y: f32, attenuated: Color32) {
        if left_y < self.zero_y || right_y < self.zero_y {
            let amplified = AMPLIFIED_COLOR.into();
            self.feathered_span(left, left_y, right, right_y, self.zero_y, amplified);
            self.trapezoid(
                left,
                self.zero_y,
                right,
                self.zero_y,
                self.bottom,
                attenuated,
            );
        } else {
            self.feathered_span(left, left_y, right, right_y, self.bottom, attenuated);
        }
    }

    fn flat_bar(&mut self, x: f32, top: f32, color: Color32) {
        self.trapezoid(x, top, x + BAR_WIDTH, top, self.bottom, color);
    }

    /// Filled span with egui-style feathering: a gradient into transparency centered on the top edge.
    fn feathered_span(
        &mut self,
        left: f32,
        left_top: f32,
        right: f32,
        right_top: f32,
        floor: f32,
        color: Color32,
    ) {
        if right <= left {
            return;
        }

        let (outer_left, inner_left) = feather_ys(left_top, floor, self.feather);
        let (outer_right, inner_right) = feather_ys(right_top, floor, self.feather);

        self.trapezoid(left, inner_left, right, inner_right, floor, color);

        if self.feather > 0.0 {
            self.quad(
                [
                    Pos2::new(left, outer_left),
                    Pos2::new(right, outer_right),
                    Pos2::new(right, inner_right),
                    Pos2::new(left, inner_left),
                ],
                [Color32::TRANSPARENT, Color32::TRANSPARENT, color, color],
            );
        }
    }

    fn trapezoid(
        &mut self,
        left: f32,
        left_top: f32,
        right: f32,
        right_top: f32,
        bottom: f32,
        color: Color32,
    ) {
        if right <= left || (left_top >= bottom && right_top >= bottom) {
            return;
        }

        self.quad(
            [
                Pos2::new(left, left_top),
                Pos2::new(right, right_top),
                Pos2::new(right, bottom),
                Pos2::new(left, bottom),
            ],
            [color; 4],
        );
    }

    fn quad(&mut self, corners: [Pos2; 4], colors: [Color32; 4]) {
        let i = self.mesh.vertices.len() as u32;

        for (pos, color) in corners.into_iter().zip(colors) {
            self.mesh.colored_vertex(pos, color);
        }

        self.mesh.add_triangle(i, i + 1, i + 2);
        self.mesh.add_triangle(i, i + 2, i + 3);
    }
}
