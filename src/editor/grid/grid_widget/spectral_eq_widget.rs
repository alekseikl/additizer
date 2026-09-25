use egui::{Color32, Mesh, Painter, Pos2, Rect, Shape, epaint::PathStroke};
use smallvec::SmallVec;

use crate::{
    editor::grid::WidgetCtx,
    synth_engine::{
        ComplexSample, Input, ModuleId, Sample,
        filters::spectral_filter::{FilterParams, SpectralFilter as SpectralFilterEngine},
        spectral_eq::{MAX_EQ_FILTERS, SpectralEqUiBridge},
        ui_bridge::{ModuleBridge, UiBridge},
    },
    utils::{
        C4_PITCH, MAX_CUTOFF, MAX_LEVEL_DB, MIN_CUTOFF, MIN_LEVEL_DB, db_to_gain_fast,
        freq_to_c4_pitch, gain_to_db_fast,
    },
};

use super::GridWidgetContent;

const PADDING: f32 = 4.0;

const STROKE_COLOR: Color32 = Color32::from_rgb(0xff, 0xb0, 0x00);
const LINE_WIDTH: f32 = 1.0;
const COLUMNS: usize = 512;
const MAX_POINTS: usize = COLUMNS + MAX_EQ_FILTERS;

pub struct SpectralEqWidget {
    cols: Vec<f32>,
    freqs: Vec<Sample>,
    responses: Vec<ComplexSample>,
    band_responses: Vec<ComplexSample>,
}

impl Default for SpectralEqWidget {
    fn default() -> Self {
        Self {
            cols: Vec::with_capacity(MAX_POINTS),
            freqs: Vec::with_capacity(MAX_POINTS),
            responses: Vec::with_capacity(MAX_POINTS),
            band_responses: Vec::with_capacity(MAX_POINTS),
        }
    }
}

impl SpectralEqWidget {
    fn eq_ui(
        &mut self,
        ui: &mut egui::Ui,
        bridge: &mut UiBridge,
        eq_bridge: &mut SpectralEqUiBridge,
        module_id: ModuleId,
    ) {
        let size = ui.available_size();
        let response = ui.allocate_response(size, egui::Sense::hover());
        let rect = response.rect.shrink2(egui::vec2(0.0, PADDING));

        if !rect.is_positive() || !ui.is_rect_visible(rect) {
            return;
        }

        let mut config = eq_bridge.config().clone();
        let has_voices = bridge.has_active_voices();

        if has_voices {
            bridge.apply_modulation(module_id, Input::Cutoff, &mut config.cutoff);
        }
        bridge.apply_modulation(module_id, Input::Level, &mut config.output_level);

        let shift = if has_voices {
            (eq_bridge.pitch() - C4_PITCH) * config.keytrack
        } else {
            0.0
        };
        let q_cutoff = config.q_cutoff[0] + shift;
        let q_rolloff = config.q_rolloff[0];
        let linear_phase = config.linear_phase;
        let gain = db_to_gain_fast(config.output_level[0]);
        let cutoff_offset = config.cutoff[0];

        let cutoffs: SmallVec<[Sample; MAX_EQ_FILTERS]> = config
            .filters
            .iter()
            .take(MAX_EQ_FILTERS)
            .map(|band| freq_to_c4_pitch(band.cutoff_hz) + cutoff_offset + shift)
            .collect();

        self.sample_curve(&cutoffs);
        self.responses
            .resize(self.freqs.len(), ComplexSample::new(gain, 0.0));
        self.responses.fill(ComplexSample::new(gain, 0.0));
        self.band_responses
            .resize(self.freqs.len(), ComplexSample::ZERO);

        for (band, &cutoff) in config.filters.iter().zip(&cutoffs) {
            let filter = SpectralFilterEngine::new(
                band.filter_type,
                FilterParams {
                    drive: band.drive,
                    cutoff,
                    q: band.q,
                    q_cutoff,
                    q_rolloff,
                    linear_phase,
                },
            );

            filter.response_at_freqs(&self.freqs, &mut self.band_responses);
            for (acc, band_response) in self.responses.iter_mut().zip(self.band_responses.iter()) {
                *acc *= *band_response;
            }
        }

        self.paint_response(ui.painter(), rect);
    }

    fn sample_curve(&mut self, cutoffs: &[Sample]) {
        let t_mult = ((COLUMNS - 1) as f32).recip();
        let log2_range = MAX_CUTOFF - MIN_CUTOFF;
        let col_range = 0.0..=(COLUMNS - 1) as f32;
        let col_scale = (COLUMNS - 1) as f32 / log2_range;
        let mut marks: SmallVec<[f32; MAX_EQ_FILTERS]> = cutoffs
            .iter()
            .map(|&oct| (oct - MIN_CUTOFF) * col_scale)
            .filter(|col| col_range.contains(col))
            .collect();

        marks.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        self.cols.clear();
        self.freqs.clear();

        let cols = &mut self.cols;
        let freqs = &mut self.freqs;
        let mut push_col = |col: f32| {
            cols.push(col);
            freqs.push((MIN_CUTOFF + col * t_mult * log2_range).exp2());
        };

        let mut mark_i = 0;

        for c in 0..COLUMNS {
            let c = c as f32;

            while mark_i < marks.len() && marks[mark_i] <= c + 1e-3 {
                if (marks[mark_i] - c).abs() > 1e-3 {
                    push_col(marks[mark_i]);
                }
                mark_i += 1;
            }

            push_col(c);
        }
    }

    fn curve_points(&self, rect: Rect) -> Vec<Pos2> {
        const DB_RANGE_MULT: f32 = (MAX_LEVEL_DB - MIN_LEVEL_DB).recip();
        let t_mult = ((COLUMNS - 1) as f32).recip();

        self.cols
            .iter()
            .zip(self.responses.iter())
            .map(|(&col, response)| {
                let t = col * t_mult;
                let db = gain_to_db_fast(response.norm());
                let y_t = ((db - MIN_LEVEL_DB) * DB_RANGE_MULT).clamp(0.0, 1.0);

                Pos2::new(
                    rect.left() + t * rect.width(),
                    rect.bottom() - y_t * rect.height(),
                )
            })
            .collect()
    }

    fn paint_response(&self, painter: &Painter, rect: Rect) {
        let painter = painter.with_clip_rect(Rect::from_min_max(
            rect.left_top(),
            Pos2::new(rect.right(), rect.bottom() - LINE_WIDTH),
        ));
        let points = self.curve_points(rect);

        Self::paint_fill(&painter, rect, &points);
        Self::paint_stroke(&painter, &points);
    }

    fn fill_color() -> Color32 {
        Color32::from_rgba_unmultiplied(0xff, 0xb0, 0x00, 0x66)
    }

    fn paint_stroke(painter: &Painter, points: &[Pos2]) {
        let stroke = PathStroke::new(LINE_WIDTH, STROKE_COLOR).inside();

        painter.line(points.to_vec(), stroke);
    }

    fn paint_fill(painter: &Painter, rect: Rect, points: &[Pos2]) {
        let mut mesh = Mesh::default();
        let fill = Self::fill_color();
        let bottom = rect.bottom();

        for window in points.windows(2) {
            let (a, b) = (window[0], window[1]);
            let i_a = mesh.vertices.len() as u32;

            mesh.colored_vertex(a, fill);
            mesh.colored_vertex(b, fill);
            mesh.colored_vertex(Pos2::new(b.x, bottom), fill);
            mesh.colored_vertex(Pos2::new(a.x, bottom), fill);

            mesh.add_triangle(i_a, i_a + 1, i_a + 2);
            mesh.add_triangle(i_a, i_a + 2, i_a + 3);
        }

        painter.add(Shape::mesh(mesh));
    }
}

impl GridWidgetContent for SpectralEqWidget {
    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut WidgetCtx, module_id: ModuleId) {
        ctx.bridge
            .with_module_bridge(module_id, |bridge, module_bridge| {
                if let ModuleBridge::SpectralEq(eq_bridge) = module_bridge {
                    self.eq_ui(ui, bridge, eq_bridge, module_id);
                }
            });
    }
}
