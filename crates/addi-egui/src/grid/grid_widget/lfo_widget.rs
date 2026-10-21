use addi_ui_backend::ui_bridge::modules::lfo::LfoUiBridge;
use egui::{Color32, Pos2, Rect};

use crate::{
    grid::WidgetCtx,
    waveform::{self, WaveformOptions},
};
use addi_dsp::segments::Segments;
use addi_engine::{Input, ModuleId, Smp};
use addi_ui_backend::ui_bridge::{ModuleBridge, UiBridge};

use super::GridWidgetContent;

const PADDING: f32 = 4.0;
const PHASE_RADIUS: f32 = 3.5;
const WAVE_COLOR: Color32 = Color32::from_rgb(0x4a, 0xb0, 0xff);

#[derive(Default)]
pub struct LfoWidget {
    waveform: Vec<Smp>,
}

impl LfoWidget {
    /// One sample per point of `width`. `phase_shift` rotates that cycle.
    fn build_waveform(&mut self, segments: &Segments, phase_shift: Smp, width: Smp) {
        let len = (width.round() as usize).max(1);

        self.waveform.resize(len, 0.0);
        segments.render(&mut self.waveform, false);

        let offset = (phase_shift.rem_euclid(1.0) * len as Smp).round() as usize;

        self.waveform.rotate_left(offset % len);
    }

    /// Level on the rendered cycle. Phases between samples are interpolated, and the last sample wraps to the first.
    fn waveform_at(&self, phase: Smp) -> Smp {
        let len = self.waveform.len();

        if len == 0 || !phase.is_finite() {
            return 0.0;
        }

        let scaled = phase.rem_euclid(1.0) * len as Smp;

        if scaled >= len as Smp {
            return self.waveform[0];
        }

        let index = scaled as usize;
        let next = (index + 1) % len;
        let frac = scaled - index as Smp;

        self.waveform[index] + (self.waveform[next] - self.waveform[index]) * frac
    }

    fn phase_pos(rect: Rect, phase: Smp, value: Smp) -> Pos2 {
        Pos2::new(
            rect.left() + phase.clamp(0.0, 1.0) * rect.width(),
            rect.bottom() - value.clamp(0.0, 1.0) * rect.height(),
        )
    }

    fn lfo_ui(
        &mut self,
        ui: &mut egui::Ui,
        bridge: &mut UiBridge,
        lfo_bridge: &mut LfoUiBridge,
        module_id: ModuleId,
        has_active_voices: bool,
    ) {
        let size = ui.available_size();
        let response = ui.allocate_response(size, egui::Sense::hover());
        let rect = response.rect.shrink2(egui::vec2(0.0, PADDING));

        if !rect.is_positive() || !ui.is_rect_visible(rect) {
            return;
        }

        let mut config = lfo_bridge.config().clone();

        bridge.apply_modulation(module_id, Input::PhaseShift, &mut config.phase_shift);

        self.build_waveform(&config.segments, config.phase_shift[0], rect.width());

        let painter = ui.painter();

        waveform::paint_waveform_with_options(
            painter,
            rect,
            &self.waveform,
            WaveformOptions {
                close_period: false,
                normalize: false,
                color: WAVE_COLOR,
                fill: true,
                bipolar: false,
            },
        );

        if has_active_voices {
            let phase = lfo_bridge.get_phase();
            painter.circle_filled(
                Self::phase_pos(rect, phase, self.waveform_at(phase)),
                PHASE_RADIUS,
                WAVE_COLOR,
            );
        }
    }
}

impl GridWidgetContent for LfoWidget {
    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut WidgetCtx, module_id: ModuleId) {
        let has_active_voices = ctx.bridge.has_active_voices();

        ctx.bridge
            .with_module_bridge(module_id, |bridge, module_bridge| {
                if let ModuleBridge::Lfo(lfo_bridge) = module_bridge {
                    self.lfo_ui(ui, bridge, lfo_bridge, module_id, has_active_voices);
                }
            });
    }
}

#[cfg(test)]
mod tests;
