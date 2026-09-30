use std::f32::consts::{PI, TAU};

use egui::{Color32, Mesh, Painter, Pos2, Rect, Shape, ecolor::Hsva};

use crate::{
    editor::utils::hsva,
    synth_engine::{ComplexSample, DC_OFFSET, harmonic_editor::sawtooth_phase},
    utils::{MAX_LEVEL_DB, MIN_LEVEL_DB, gain_to_db},
};

#[cfg(test)]
mod tests;

const BAR_WIDTH: f32 = 1.0;
/// Adjacent columns closer than this, in points, get a sloped top toward the next column.
const SLOPE_HEIGHT: f32 = 2.0;

const DB_SPAN: f32 = MAX_LEVEL_DB - MIN_LEVEL_DB;
const ZERO_DB_LEVEL: f32 = -MIN_LEVEL_DB / DB_SPAN;
const SILENCE_DB: f32 = -100.0;

/// Hue when a harmonic sits on the sawtooth phase. Bars below 0 dB turn this hue by the phase offset.
const ATTENUATED_COLOR: Hsva = hsva(0.567, 1.0, 0.35, 1.0);
const AMPLIFIED_COLOR: Hsva = hsva(0.02, 0.88, 0.55, 1.0);

const PLACEHOLDER_BARS: usize = 128;
const PLACEHOLDER_COLOR: Color32 = Color32::from_rgb(36, 38, 50);

#[derive(Clone, Copy, Default)]
struct Column {
    level: f32,
    hue: f32,
}

#[derive(Default)]
pub struct FrequencyBins {
    columns: Vec<Column>,
}

impl FrequencyBins {
    pub fn paint(&mut self, painter: &Painter, rect: Rect, spectrum: &[ComplexSample]) {
        if !rect.is_positive() {
            return;
        }

        if !self.paint_bins(painter, rect, spectrum) {
            Self::paint_placeholder(painter, rect);
        }
    }

    fn paint_bins(&mut self, painter: &Painter, rect: Rect, spectrum: &[ComplexSample]) -> bool {
        let column_count = rect.width().floor() as usize;

        if column_count <= 1 {
            return false;
        }

        self.columns.clear();
        self.columns.resize(column_count, Column::default());

        if !self.accumulate_columns(spectrum) {
            return false;
        }

        self.paint_columns(painter, rect);
        true
    }

    fn paint_columns(&self, painter: &Painter, rect: Rect) {
        let mut mesh = Mesh::default();
        let bottom = rect.bottom();
        let height = rect.height();
        let zero_y = bottom - ZERO_DB_LEVEL * height;
        let feather = Self::feather_width(painter);

        for col in 0..self.columns.len() {
            let column = self.columns[col];

            if column.level <= 0.0 {
                continue;
            }

            let x = rect.left() + col as f32;
            let attenuated = Self::attenuated_color(column.hue);
            let next_level = self
                .columns
                .get(col + 1)
                .map(|next| next.level)
                .filter(|level| *level > 0.0);

            if let Some(next_level) = next_level
                .filter(|next_level| Self::within_slope_height(column.level, *next_level, height))
            {
                Self::paint_sloped_bar(
                    &mut mesh,
                    x,
                    BAR_WIDTH,
                    column.level,
                    next_level,
                    bottom,
                    height,
                    zero_y,
                    attenuated,
                    feather,
                );
            } else {
                Self::paint_bar(
                    &mut mesh,
                    x,
                    BAR_WIDTH,
                    column.level,
                    bottom,
                    height,
                    zero_y,
                    attenuated,
                    feather,
                );
            }
        }

        if !mesh.vertices.is_empty() {
            painter.with_clip_rect(rect).add(Shape::mesh(mesh));
        }
    }

    fn column_for_harmonic(harmonic: usize, log2_last: f32, column_count: usize) -> usize {
        debug_assert!(harmonic >= DC_OFFSET);
        debug_assert!(log2_last > 0.0);
        debug_assert!(column_count > 1);

        let t = (harmonic as f32).log2() / log2_last;
        let col = (t * (column_count - 1) as f32).round() as usize;
        col.min(column_count - 1)
    }

    fn level_from_bin(harmonic: usize, bin: ComplexSample) -> f32 {
        let amplitude = bin.norm() * harmonic as f32 * PI;
        let db = gain_to_db(amplitude);

        if db <= SILENCE_DB {
            return 0.0;
        }

        ((db - MIN_LEVEL_DB) / DB_SPAN).clamp(0.0, 1.0)
    }

    /// `ATTENUATED_COLOR` hue, turned by this harmonic's phase offset from sawtooth (turns).
    fn attenuated_hue(harmonic: usize, bin: ComplexSample) -> f32 {
        let phase = (bin.arg() / TAU).rem_euclid(1.0);
        (ATTENUATED_COLOR.h + phase - sawtooth_phase(harmonic)).rem_euclid(1.0)
    }

    fn attenuated_color(hue: f32) -> Color32 {
        Color32::from(Hsva {
            h: hue,
            ..ATTENUATED_COLOR
        })
    }

    fn accumulate_columns(&mut self, spectrum: &[ComplexSample]) -> bool {
        let column_count = self.columns.len();

        debug_assert_ne!(column_count, 1);

        if spectrum.len() <= DC_OFFSET || column_count <= 1 {
            return false;
        }

        let last_harmonic = spectrum.len() - 1;

        if last_harmonic == 1 {
            let bin = spectrum[last_harmonic];
            let level = Self::level_from_bin(last_harmonic, bin);
            self.columns[0] = Column {
                level,
                hue: Self::attenuated_hue(last_harmonic, bin),
            };
            return level > 0.0;
        }

        let log2_last = (last_harmonic as f32).log2();
        let mut has_level = false;

        for (harmonic, bin) in spectrum.iter().enumerate().skip(DC_OFFSET) {
            let col = Self::column_for_harmonic(harmonic, log2_last, column_count);
            let level = Self::level_from_bin(harmonic, *bin);

            if level > self.columns[col].level {
                self.columns[col] = Column {
                    level,
                    hue: Self::attenuated_hue(harmonic, *bin),
                };
                has_level = true;
            }
        }

        has_level
    }

    fn paint_placeholder(painter: &Painter, rect: Rect) {
        let column_count = rect.width().floor() as usize;

        if column_count <= 1 {
            return;
        }

        let mut mesh = Mesh::default();
        let bottom = rect.bottom();
        let zero_y = bottom - ZERO_DB_LEVEL * rect.height();
        let mut last_col = None;
        let log2_last = (PLACEHOLDER_BARS as f32).log2();

        for harmonic in DC_OFFSET..=PLACEHOLDER_BARS {
            let col = Self::column_for_harmonic(harmonic, log2_last, column_count);

            if last_col == Some(col) {
                continue;
            }

            last_col = Some(col);
            Self::push_bar(
                &mut mesh,
                rect.left() + col as f32,
                BAR_WIDTH,
                zero_y,
                bottom,
                PLACEHOLDER_COLOR,
            );
        }

        if !mesh.vertices.is_empty() {
            painter.with_clip_rect(rect).add(Shape::mesh(mesh));
        }
    }

    /// Screen-space height gap between two normalized levels is under [`SLOPE_HEIGHT`].
    fn within_slope_height(level: f32, next: f32, plot_height: f32) -> bool {
        let delta = (level - next).abs() * plot_height;
        delta > 0.0 && delta < SLOPE_HEIGHT
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

    fn paint_bar(
        mesh: &mut Mesh,
        x: f32,
        width: f32,
        level: f32,
        bottom: f32,
        height: f32,
        zero_y: f32,
        attenuated: Color32,
        feather: f32,
    ) {
        let y = bottom - level * height;
        let right = x + width;

        if level > ZERO_DB_LEVEL {
            Self::paint_feathered_span(
                mesh,
                x,
                y,
                right,
                y,
                zero_y,
                AMPLIFIED_COLOR.into(),
                feather,
            );
            Self::push_trapezoid(mesh, x, zero_y, right, zero_y, bottom, attenuated);
        } else {
            Self::paint_feathered_span(mesh, x, y, right, y, bottom, attenuated, feather);
        }
    }

    /// One column whose top edge runs from `level` to `next_level`.
    fn paint_sloped_bar(
        mesh: &mut Mesh,
        x: f32,
        width: f32,
        level: f32,
        next_level: f32,
        bottom: f32,
        height: f32,
        zero_y: f32,
        attenuated: Color32,
        feather: f32,
    ) {
        let left_y = bottom - level * height;
        let right_y = bottom - next_level * height;
        let right = x + width;
        let amplified = AMPLIFIED_COLOR.into();
        let left_above = left_y < zero_y;
        let right_above = right_y < zero_y;

        if !left_above && !right_above {
            Self::paint_feathered_span(
                mesh, x, left_y, right, right_y, bottom, attenuated, feather,
            );
            return;
        }

        if left_above && right_above {
            Self::paint_feathered_span(mesh, x, left_y, right, right_y, zero_y, amplified, feather);
            Self::push_trapezoid(mesh, x, zero_y, right, zero_y, bottom, attenuated);
            return;
        }

        let cross_x = x + Self::slope_cross_t(left_y, right_y, zero_y) * width;

        if left_above {
            Self::paint_feathered_span(
                mesh, x, left_y, cross_x, zero_y, zero_y, amplified, feather,
            );
            Self::push_trapezoid(mesh, x, zero_y, cross_x, zero_y, bottom, attenuated);
            Self::paint_feathered_span(
                mesh, cross_x, zero_y, right, right_y, bottom, attenuated, feather,
            );
        } else {
            Self::paint_feathered_span(
                mesh, cross_x, zero_y, right, right_y, zero_y, amplified, feather,
            );
            Self::paint_feathered_span(
                mesh, x, left_y, cross_x, zero_y, bottom, attenuated, feather,
            );
            Self::push_trapezoid(mesh, cross_x, zero_y, right, zero_y, bottom, attenuated);
        }
    }

    /// Filled span with egui-style feathering: a gradient into transparency centered on the top edge.
    fn paint_feathered_span(
        mesh: &mut Mesh,
        left: f32,
        left_top: f32,
        right: f32,
        right_top: f32,
        floor: f32,
        color: Color32,
        feather: f32,
    ) {
        if right - left <= 0.0 {
            return;
        }

        let (outer_left, inner_left) = Self::feather_ys(left_top, floor, feather);
        let (outer_right, inner_right) = Self::feather_ys(right_top, floor, feather);

        Self::push_trapezoid(mesh, left, inner_left, right, inner_right, floor, color);

        if feather <= 0.0 {
            return;
        }

        Self::push_colored_quad(
            mesh,
            Pos2::new(left, outer_left),
            Color32::TRANSPARENT,
            Pos2::new(right, outer_right),
            Color32::TRANSPARENT,
            Pos2::new(right, inner_right),
            color,
            Pos2::new(left, inner_left),
            color,
        );
    }

    fn push_bar(mesh: &mut Mesh, x: f32, width: f32, top: f32, bottom: f32, color: Color32) {
        Self::push_trapezoid(mesh, x, top, x + width, top, bottom, color);
    }

    fn push_trapezoid(
        mesh: &mut Mesh,
        left: f32,
        left_top: f32,
        right: f32,
        right_top: f32,
        bottom: f32,
        color: Color32,
    ) {
        if right - left <= 0.0 || (bottom - left_top <= 0.0 && bottom - right_top <= 0.0) {
            return;
        }

        let i = mesh.vertices.len() as u32;

        mesh.colored_vertex(Pos2::new(left, left_top), color);
        mesh.colored_vertex(Pos2::new(right, right_top), color);
        mesh.colored_vertex(Pos2::new(right, bottom), color);
        mesh.colored_vertex(Pos2::new(left, bottom), color);
        mesh.add_triangle(i, i + 1, i + 2);
        mesh.add_triangle(i, i + 2, i + 3);
    }

    fn push_colored_quad(
        mesh: &mut Mesh,
        a: Pos2,
        a_color: Color32,
        b: Pos2,
        b_color: Color32,
        c: Pos2,
        c_color: Color32,
        d: Pos2,
        d_color: Color32,
    ) {
        let i = mesh.vertices.len() as u32;

        mesh.colored_vertex(a, a_color);
        mesh.colored_vertex(b, b_color);
        mesh.colored_vertex(c, c_color);
        mesh.colored_vertex(d, d_color);
        mesh.add_triangle(i, i + 1, i + 2);
        mesh.add_triangle(i, i + 2, i + 3);
    }
}
