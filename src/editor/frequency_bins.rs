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

        for (col, column) in self.columns.iter().enumerate() {
            if column.level <= 0.0 {
                continue;
            }

            let x = rect.left() + col as f32;
            let y = bottom - column.level * height;
            let attenuated = Self::attenuated_color(column.hue);

            if column.level > ZERO_DB_LEVEL {
                Self::push_bar(&mut mesh, x, y, zero_y, AMPLIFIED_COLOR.into());
                Self::push_bar(&mut mesh, x, zero_y, bottom, attenuated);
            } else {
                Self::push_bar(&mut mesh, x, y, bottom, attenuated);
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
                zero_y,
                bottom,
                PLACEHOLDER_COLOR,
            );
        }

        if !mesh.vertices.is_empty() {
            painter.with_clip_rect(rect).add(Shape::mesh(mesh));
        }
    }

    fn push_bar(mesh: &mut Mesh, x: f32, top: f32, bottom: f32, color: Color32) {
        if bottom - top <= 0.0 {
            return;
        }

        let i = mesh.vertices.len() as u32;
        let right = x + BAR_WIDTH;

        mesh.colored_vertex(Pos2::new(x, top), color);
        mesh.colored_vertex(Pos2::new(right, top), color);
        mesh.colored_vertex(Pos2::new(right, bottom), color);
        mesh.colored_vertex(Pos2::new(x, bottom), color);
        mesh.add_triangle(i, i + 1, i + 2);
        mesh.add_triangle(i, i + 2, i + 3);
    }
}
