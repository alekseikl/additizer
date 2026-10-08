use addi_ui_backend::ui_bridge::modules::svf::SvfUiBridge;
use egui::{Color32, Mesh, Painter, Pos2, Rect, Shape, epaint::PathStroke};

use crate::grid::WidgetCtx;
use addi_dsp::{
    C4_PITCH, MAX_CUTOFF, MIN_CUTOFF, db_to_gain_fast,
    filters::{
        control::q_from_resonance,
        svf::{SvfResponse, SvfType},
    },
    gain_to_db_fast, pitch_to_freq,
};
use addi_engine::{Input, MAX_LEVEL_DB, ModuleId, Sample};
use addi_ui_backend::ui_bridge::{ModuleBridge, UiBridge};

use super::GridWidgetContent;

const PADDING: f32 = 4.0;

const STROKE_COLOR: Color32 = Color32::from_rgb(0xe0, 0x8a, 0x3a);
const LINE_WIDTH: f32 = 1.0;
const COLUMNS: usize = 256;
const MAX_POINTS: usize = COLUMNS + 1;

pub struct SvfWidget {
    points: Vec<Pos2>,
}

impl Default for SvfWidget {
    fn default() -> Self {
        Self {
            points: Vec::with_capacity(MAX_POINTS),
        }
    }
}

impl SvfWidget {
    fn filter_ui(
        &mut self,
        ui: &mut egui::Ui,
        bridge: &mut UiBridge,
        svf_bridge: &mut SvfUiBridge,
        module_id: ModuleId,
    ) {
        let size = ui.available_size();
        let response = ui.allocate_response(size, egui::Sense::hover());
        let rect = response.rect.shrink2(egui::vec2(0.0, PADDING));

        if !rect.is_positive() || !ui.is_rect_visible(rect) {
            return;
        }

        let mut config = svf_bridge.config().clone();
        let has_voices = bridge.has_active_voices();

        if has_voices {
            bridge.apply_modulation(module_id, Input::Cutoff, &mut config.cutoff);
        }
        bridge.apply_modulation(module_id, Input::Resonance, &mut config.resonance);
        bridge.apply_modulation(module_id, Input::Drive, &mut config.drive);

        let pitch = if has_voices {
            svf_bridge.pitch()
        } else {
            C4_PITCH
        };
        // Octaves relative to C4, same axis as the cutoff slider.
        let cutoff_log2 = config.cutoff[0] + config.keytrack * (pitch - C4_PITCH);
        let cutoff_freq = pitch_to_freq(C4_PITCH + cutoff_log2);
        let drive_db = config.drive[0];
        let q = q_from_resonance(config.resonance[0]);
        let response = SvfResponse {
            filter_type: config.filter_type,
            q: q.resonant,
            pre_q: q.pre_stage,
            cutoff: cutoff_freq,
            gain: db_to_gain_fast(drive_db),
        };

        self.build_points(rect, &response, cutoff_log2, drive_db);
        Self::paint_response(ui.painter(), rect, &self.points);
    }

    /// Analog prototype in decibels. Drive shifts the curve.
    /// For Peaking and the shelves, drive is the filter gain and is already inside the prototype.
    /// The UI has no sample rate, so bilinear warping near Nyquist is not shown.
    fn build_points(
        &mut self,
        rect: Rect,
        response: &SvfResponse,
        cutoff_log2: Sample,
        drive_db: Sample,
    ) {
        const MIN_DB: f32 = -40.0;
        const DB_RANGE_MULT: f32 = (MAX_LEVEL_DB - MIN_DB).recip();
        let t_mult = ((COLUMNS - 1) as f32).recip();
        let log2_range = MAX_CUTOFF - MIN_CUTOFF;
        let cutoff_col = (cutoff_log2 - MIN_CUTOFF) / log2_range * (COLUMNS - 1) as f32;
        let split = cutoff_col.ceil().clamp(0.0, COLUMNS as f32) as usize;
        let include_cutoff = (0.0..=(COLUMNS - 1) as f32).contains(&cutoff_col);

        self.points.clear();

        let points = &mut self.points;
        let mut push_col = |col: f32| {
            let t = col * t_mult;
            let freq = pitch_to_freq(C4_PITCH + MIN_CUTOFF + t * log2_range);
            let gain = response.at(freq).norm();
            let drive_offset = match response.filter_type {
                SvfType::Peaking
                | SvfType::LowShelf12
                | SvfType::LowShelf24
                | SvfType::HighShelf12
                | SvfType::HighShelf24 => 0.0,
                _ => drive_db,
            };
            let db = gain_to_db_fast(gain) + drive_offset;
            let y_t = ((db - MIN_DB) * DB_RANGE_MULT).clamp(0.0, 1.0);

            points.push(Pos2::new(
                rect.left() + t * rect.width(),
                rect.bottom() - y_t * rect.height(),
            ));
        };

        for c in 0..split {
            push_col(c as f32);
        }

        // The cutoff itself, so narrow resonance peaks are not missed.
        if include_cutoff {
            push_col(cutoff_col);
        }

        for c in split..COLUMNS {
            push_col(c as f32);
        }
    }

    fn paint_response(painter: &Painter, rect: Rect, points: &[Pos2]) {
        let painter = painter.with_clip_rect(Rect::from_min_max(
            rect.left_top(),
            Pos2::new(rect.right(), rect.bottom() - LINE_WIDTH),
        ));

        Self::paint_fill(&painter, rect, points);
        Self::paint_stroke(&painter, points);
    }

    fn fill_color() -> Color32 {
        Color32::from_rgba_unmultiplied(0xe0, 0x8a, 0x3a, 0x66)
    }

    fn paint_stroke(painter: &Painter, points: &[Pos2]) {
        if points.len() < 2 {
            return;
        }

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

impl GridWidgetContent for SvfWidget {
    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut WidgetCtx, module_id: ModuleId) {
        ctx.bridge
            .with_module_bridge(module_id, |bridge, module_bridge| {
                if let ModuleBridge::Svf(svf_bridge) = module_bridge {
                    self.filter_ui(ui, bridge, svf_bridge, module_id);
                }
            });
    }
}
