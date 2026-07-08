use egui::{Color32, Mesh, Painter, Pos2, Rect, Shape, epaint::PathStroke};

use crate::{
    editor::grid::WidgetCtx,
    synth_engine::{
        Input, ModuleId, Sample,
        filters::spectral_filter::{
            FilterParams, MAX_RESONANCE, MIN_RESONANCE, SpectralFilter as SpectralFilterEngine,
        },
        spectral_filter::SpectralFilterUiBridge,
        ui_bridge::{ModuleBridge, UiBridge},
    },
};

use super::GridWidgetContent;

const PADDING: f32 = 4.0;

const MIN_LOG2_FREQ: Sample = -2.0;
const MAX_LOG2_FREQ: Sample = 10.0;

const MIN_DB: Sample = -30.0;
const MAX_DB: Sample = 24.0;

const STROKE_COLOR: Color32 = Color32::from_rgb(0xff, 0xb0, 0x00);
const LINE_WIDTH: f32 = 1.0;

pub struct SpectralFilterWidget {}

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

        bridge.apply_modulation(module_id, Input::Cutoff, &mut config.cutoff);
        bridge.apply_modulation(module_id, Input::Resonance, &mut config.resonance);
        bridge.apply_modulation(module_id, Input::Drive, &mut config.drive);

        let filter = SpectralFilterEngine::new(
            config.filter_type,
            FilterParams {
                drive: config.drive[0].min(24.0),
                cutoff: config.cutoff[0].clamp(-4.0, 10.0),
                resonance: config.resonance[0].clamp(MIN_RESONANCE, MAX_RESONANCE),
                q_limit_to: config.q_limit_to[0],
                q_limit_curve: config.q_limit_curve[0],
                linear_phase: config.linear_phase,
            },
        );

        Self::paint_response(ui.painter(), rect, &filter);
    }

    fn curve_points(rect: Rect, filter: &SpectralFilterEngine) -> Vec<Pos2> {
        const DB_RANGE_MULT: f32 = (MAX_DB - MIN_DB).recip();
        let columns = rect.width().ceil().max(2.0) as usize;
        let t_mult = ((columns - 1) as f32).recip();

        (0..columns)
            .map(|column| {
                let t = column as f32 * t_mult;
                let freq = (MIN_LOG2_FREQ + t * (MAX_LOG2_FREQ - MIN_LOG2_FREQ)).exp2();
                let db = 20.0 * filter.response_at(freq).norm().max(1e-6).log10();
                let y_t = ((db - MIN_DB) * DB_RANGE_MULT).clamp(0.0, 1.0);

                Pos2::new(
                    rect.left() + t * rect.width(),
                    rect.bottom() - y_t * rect.height(),
                )
            })
            .collect()
    }

    fn paint_response(painter: &Painter, rect: Rect, filter: &SpectralFilterEngine) {
        let painter = painter.with_clip_rect(Rect::from_min_max(
            rect.left_top(),
            Pos2::new(rect.right(), rect.bottom() - LINE_WIDTH),
        ));
        let points = Self::curve_points(rect, filter);

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
