use egui::{Color32, Mesh, Painter, Pos2, Rect, Shape, epaint::PathStroke};

use crate::{
    editor::grid::WidgetCtx,
    synth_engine::{
        ComplexSample, Input, ModuleId, Sample,
        filters::spectral_filter::{
            FilterParams, MAX_DRIVE, MAX_RESONANCE, MIN_DRIVE, MIN_RESONANCE,
            SpectralFilter as SpectralFilterEngine,
        },
        spectral_filter::{MAX_CUTOFF, MIN_CUTOFF, SpectralFilterUiBridge},
        ui_bridge::{ModuleBridge, UiBridge},
    },
    utils::{C4_PITCH, MAX_LEVEL_DB, MIN_LEVEL_DB, gain_to_db_fast},
};

use super::GridWidgetContent;

const PADDING: f32 = 4.0;

const STROKE_COLOR: Color32 = Color32::from_rgb(0xff, 0xb0, 0x00);
const LINE_WIDTH: f32 = 1.0;
const COLUMNS: usize = 512;
const MAX_POINTS: usize = COLUMNS + 1;

pub struct SpectralFilterWidget {
    cols: Vec<f32>,
    freqs: Vec<Sample>,
    responses: Vec<ComplexSample>,
}

impl Default for SpectralFilterWidget {
    fn default() -> Self {
        Self {
            cols: Vec::with_capacity(MAX_POINTS),
            freqs: Vec::with_capacity(MAX_POINTS),
            responses: Vec::with_capacity(MAX_POINTS),
        }
    }
}

impl SpectralFilterWidget {
    fn filter_ui(
        &mut self,
        ui: &mut egui::Ui,
        bridge: &mut UiBridge,
        filter_bridge: &mut SpectralFilterUiBridge,
        module_id: ModuleId,
    ) {
        let size = ui.available_size();
        let response = ui.allocate_response(size, egui::Sense::hover());
        let rect = response.rect.shrink2(egui::vec2(0.0, PADDING));

        if !rect.is_positive() || !ui.is_rect_visible(rect) {
            return;
        }

        let mut config = filter_bridge.config().clone();
        let has_voices = bridge.has_active_voices();

        if has_voices {
            bridge.apply_modulation(module_id, Input::Cutoff, &mut config.cutoff);
        }
        bridge.apply_modulation(module_id, Input::Resonance, &mut config.resonance);
        bridge.apply_modulation(module_id, Input::Drive, &mut config.drive);

        let offset = if has_voices {
            (filter_bridge.pitch() - C4_PITCH) * config.keytrack
        } else {
            0.0
        };
        let cutoff = (config.cutoff[0] + offset).clamp(MIN_CUTOFF, MAX_CUTOFF);
        let q_limit_to = (config.q_limit_to[0] + offset).clamp(MIN_CUTOFF, MAX_CUTOFF);
        let filter = SpectralFilterEngine::new(
            config.filter_type,
            FilterParams {
                drive: config.drive[0].clamp(MIN_DRIVE, MAX_DRIVE),
                cutoff,
                resonance: config.resonance[0].clamp(MIN_RESONANCE, MAX_RESONANCE),
                q_limit_to,
                q_limit_slope: config.q_limit_slope[0],
                linear_phase: config.linear_phase,
            },
        );

        self.paint_response(ui.painter(), rect, &filter, cutoff);
    }

    fn curve_points(
        &mut self,
        rect: Rect,
        filter: &SpectralFilterEngine,
        cutoff_log2: Sample,
    ) -> Vec<Pos2> {
        const DB_RANGE_MULT: f32 = (MAX_LEVEL_DB - MIN_LEVEL_DB).recip();
        let t_mult = ((COLUMNS - 1) as f32).recip();
        let log2_range = MAX_CUTOFF - MIN_CUTOFF;

        let cutoff_col = (cutoff_log2 - MIN_CUTOFF) / log2_range * (COLUMNS - 1) as f32;
        let split = cutoff_col.ceil().clamp(0.0, COLUMNS as f32) as usize;
        let include_cutoff = (0.0..=(COLUMNS - 1) as f32).contains(&cutoff_col);

        self.cols.clear();
        self.freqs.clear();

        let cols = &mut self.cols;
        let freqs = &mut self.freqs;
        let mut push_col = |col: f32| {
            cols.push(col);
            freqs.push((MIN_CUTOFF + col * t_mult * log2_range).exp2());
        };

        // Columns strictly before the cutoff.
        for c in 0..split {
            push_col(c as f32);
        }

        // The cutoff point itself, when it lands in the visible range.
        if include_cutoff {
            push_col(cutoff_col);
        }

        // Remaining columns after the cutoff.
        for c in split..COLUMNS {
            push_col(c as f32);
        }

        self.responses
            .resize(self.freqs.len(), ComplexSample::new(0.0, 0.0));
        filter.response_at_freqs(&self.freqs, &mut self.responses);

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

    fn paint_response(
        &mut self,
        painter: &Painter,
        rect: Rect,
        filter: &SpectralFilterEngine,
        cutoff_log2: Sample,
    ) {
        let painter = painter.with_clip_rect(Rect::from_min_max(
            rect.left_top(),
            Pos2::new(rect.right(), rect.bottom() - LINE_WIDTH),
        ));
        let points = self.curve_points(rect, filter, cutoff_log2);

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

impl GridWidgetContent for SpectralFilterWidget {
    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut WidgetCtx, module_id: ModuleId) {
        ctx.bridge
            .with_module_bridge(module_id, |bridge, module_bridge| {
                if let ModuleBridge::SpectralFilter(filter_bridge) = module_bridge {
                    self.filter_ui(ui, bridge, filter_bridge, module_id);
                }
            });
    }
}
