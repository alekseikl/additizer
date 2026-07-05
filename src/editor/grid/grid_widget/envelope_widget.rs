use egui::{Color32, Mesh, Painter, Pos2, Rect, Shape, epaint::PathStroke};

use crate::{
    editor::grid::WidgetCtx,
    synth_engine::{
        Input, ModuleId, Sample,
        envelope::{EnvelopeConfig, EnvelopePhase, EnvelopeUiBridge},
        ui_bridge::{ModuleBridge, UiBridge},
    },
    utils::power_scale,
};

use super::GridWidgetContent;

const PADDING: f32 = 4.0;
const POINTS_PER_SECTION: usize = 32;
const PHASE_RADIUS: f32 = 3.5;
const LINE_WIDTH: f32 = 1.0;
const MIN_TOTAL_TIME: Sample = 1e-3;

const STROKE_COLOR: Color32 = Color32::from_rgb(0x06, 0xaa, 0x1c);

struct EnvelopeShape {
    delay: Sample,
    attack: Sample,
    hold: Sample,
    decay: Sample,
    sustain: Sample,
    release: Sample,
    attack_curvature: Sample,
    decay_curvature: Sample,
    release_curvature: Sample,
}

impl EnvelopeShape {
    fn from_config(config: &EnvelopeConfig) -> Self {
        Self {
            delay: config.delay[0].max(0.0),
            attack: config.attack[0].max(0.0),
            hold: config.hold[0].max(0.0),
            decay: config.decay[0].max(0.0),
            sustain: config.sustain[0].clamp(0.0, 1.0),
            release: config.release[0].max(0.0),
            attack_curvature: config.attack_curvature,
            decay_curvature: config.decay_curvature,
            release_curvature: config.release_curvature,
        }
    }

    fn total_time(&self) -> Sample {
        (self.delay + self.attack + self.hold + self.decay + self.release).max(MIN_TOTAL_TIME)
    }
}

pub struct EnvelopeWidget {}

impl EnvelopeWidget {
    fn envelope_ui(
        &mut self,
        ui: &mut egui::Ui,
        bridge: &mut UiBridge,
        env_bridge: &mut EnvelopeUiBridge,
        module_id: ModuleId,
    ) {
        let size = ui.available_size();
        let response = ui.allocate_response(size, egui::Sense::hover());
        let rect = response.rect.shrink2(egui::vec2(0.0, PADDING));

        if !rect.is_positive() || !ui.is_rect_visible(rect) {
            return;
        }

        let mut config = env_bridge.config().clone();

        bridge.apply_modulation(module_id, Input::Delay, &mut config.delay);
        bridge.apply_modulation(module_id, Input::Attack, &mut config.attack);
        bridge.apply_modulation(module_id, Input::Hold, &mut config.hold);
        bridge.apply_modulation(module_id, Input::Decay, &mut config.decay);
        bridge.apply_modulation(module_id, Input::Sustain, &mut config.sustain);
        bridge.apply_modulation(module_id, Input::Release, &mut config.release);

        let shape = EnvelopeShape::from_config(&config);
        let phase = env_bridge.get_phase();
        let painter = ui.painter();
        let points = Self::curve_points(rect, &shape);

        Self::paint_fill(painter, rect, &points);
        Self::paint_stroke(painter, &points);

        if let Some(pos) = Self::phase_pos(rect, &shape, phase) {
            painter.circle_filled(pos, PHASE_RADIUS, STROKE_COLOR);
        }
    }

    fn curve_value(t: Sample, curvature: Sample, from: Sample, to: Sample) -> Sample {
        let power = curvature.clamp(-1.0, 1.0) * -10.0;
        (to - from).mul_add(power_scale(t.clamp(0.0, 1.0), power), from)
    }

    fn to_pos(rect: Rect, total: Sample, time: Sample, value: Sample) -> Pos2 {
        Pos2::new(
            rect.left() + (time / total).clamp(0.0, 1.0) * rect.width(),
            rect.bottom() - value.clamp(0.0, 1.0) * rect.height(),
        )
    }

    fn curve_points(rect: Rect, shape: &EnvelopeShape) -> Vec<Pos2> {
        let total = shape.total_time();
        let mut points = Vec::with_capacity(POINTS_PER_SECTION * 4 + 2);

        points.push(Self::to_pos(rect, total, 0.0, 0.0));

        if shape.delay > 0.0 {
            points.push(Self::to_pos(rect, total, shape.delay, 0.0));
        }

        let attack_start = shape.delay;

        for i in 1..=POINTS_PER_SECTION {
            let t = i as Sample / POINTS_PER_SECTION as Sample;
            let value = Self::curve_value(t, shape.attack_curvature, 0.0, 1.0);
            points.push(Self::to_pos(
                rect,
                total,
                attack_start + shape.attack * t,
                value,
            ));
        }

        let hold_start = attack_start + shape.attack;

        if shape.hold > 0.0 {
            points.push(Self::to_pos(rect, total, hold_start + shape.hold, 1.0));
        }

        let decay_start = hold_start + shape.hold;

        for i in 1..=POINTS_PER_SECTION {
            let t = i as Sample / POINTS_PER_SECTION as Sample;
            let value = Self::curve_value(t, shape.decay_curvature, 1.0, shape.sustain);
            points.push(Self::to_pos(
                rect,
                total,
                decay_start + shape.decay * t,
                value,
            ));
        }

        let release_start = decay_start + shape.decay;

        for i in 1..=POINTS_PER_SECTION {
            let t = i as Sample / POINTS_PER_SECTION as Sample;
            let value = Self::curve_value(t, shape.release_curvature, shape.sustain, 0.0);
            points.push(Self::to_pos(
                rect,
                total,
                release_start + shape.release * t,
                value,
            ));
        }

        points
    }

    fn phase_pos(rect: Rect, shape: &EnvelopeShape, phase: EnvelopePhase) -> Option<Pos2> {
        let total = shape.total_time();

        Some(match phase {
            EnvelopePhase::Delay(t) => Self::to_pos(rect, total, shape.delay * t, 0.0),
            EnvelopePhase::Attack(t) => Self::to_pos(
                rect,
                total,
                shape.delay + shape.attack * t,
                Self::curve_value(t, shape.attack_curvature, 0.0, 1.0),
            ),
            EnvelopePhase::Hold(t) => Self::to_pos(
                rect,
                total,
                shape.delay + shape.attack + shape.hold * t,
                1.0,
            ),
            EnvelopePhase::Decay(t) => Self::to_pos(
                rect,
                total,
                shape.delay + shape.attack + shape.hold + shape.decay * t,
                Self::curve_value(t, shape.decay_curvature, 1.0, shape.sustain),
            ),
            EnvelopePhase::Sustain => Self::to_pos(
                rect,
                total,
                shape.delay + shape.attack + shape.hold + shape.decay,
                shape.sustain,
            ),
            EnvelopePhase::Release(t) => Self::to_pos(
                rect,
                total,
                shape.delay + shape.attack + shape.hold + shape.decay + shape.release * t,
                Self::curve_value(t, shape.release_curvature, shape.sustain, 0.0),
            ),
            EnvelopePhase::Done => return None,
        })
    }

    fn fill_color() -> Color32 {
        Color32::from_rgba_unmultiplied(STROKE_COLOR.r(), STROKE_COLOR.g(), STROKE_COLOR.b(), 0x66)
    }

    fn paint_stroke(painter: &Painter, points: &[Pos2]) {
        if points.len() < 2 {
            return;
        }

        let stroke = PathStroke::new(LINE_WIDTH, STROKE_COLOR).middle();
        painter.line(points.to_vec(), stroke);
    }

    fn paint_fill(painter: &Painter, rect: Rect, points: &[Pos2]) {
        if points.len() < 2 {
            return;
        }

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

impl GridWidgetContent for EnvelopeWidget {
    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut WidgetCtx, module_id: ModuleId) {
        ctx.bridge
            .with_module_bridge(module_id, |bridge, module_bridge| {
                if let ModuleBridge::Envelope(env_bridge) = module_bridge {
                    self.envelope_ui(ui, bridge, env_bridge, module_id);
                }
            });
    }
}
