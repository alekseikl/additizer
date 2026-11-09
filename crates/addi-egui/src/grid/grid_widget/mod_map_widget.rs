use addi_ui_backend::ui_bridge::modules::mod_map::ModMapUiBridge;
use egui::{Rect, Vec2, emath::GuiRounding};

use crate::fit_label::FitLabel;
use crate::grid::WidgetCtx;
use crate::routing_ui_ext::ModuleTypeUi;
use crate::segments_editor::{CurveMap, RelWidgetPlot};
use addi_engine::{Input, ModuleId, ModuleType};
use addi_ui_backend::ui_bridge::{GridVec, ModuleBridge, UiBridge};

use super::GridWidgetContent;

const VERT_PADDING: f32 = 2.0;

pub struct ModMapWidget {
    plot: RelWidgetPlot,
}

impl Default for ModMapWidget {
    fn default() -> Self {
        Self {
            plot: RelWidgetPlot::map(),
        }
    }
}

impl ModMapWidget {
    fn map_ui(
        &mut self,
        ui: &mut egui::Ui,
        bridge: &mut UiBridge,
        map_bridge: &mut ModMapUiBridge,
        module_id: ModuleId,
        has_active_voices: bool,
    ) {
        ui.add_space(2.0);
        ui.add(FitLabel::new(ModuleType::ModMap.default_label(), "Map"));

        let size = ui.available_size();
        let response = ui.allocate_response(size, egui::Sense::hover());
        let inner = Rect::from_min_max(
            response.rect.min + Vec2::new(0.0, VERT_PADDING),
            response.rect.max - Vec2::new(0.0, VERT_PADDING),
        );
        let plot = square_in(inner, ui.pixels_per_point());

        if !plot.is_positive() || !ui.is_rect_visible(plot) {
            return;
        }

        let probe = has_active_voices.then(|| map_bridge.probe());
        let config = map_bridge.config();
        let mut amount = config.amount;

        bridge.apply_modulation(module_id, Input::Amount, &mut amount);

        let amount = amount[0].clamp(0.0, 1.0);
        let bipolar = config.bipolar;
        let segments = &config.segments;
        let playhead = probe.map(|probe| {
            let level = |x| segments.value_at(x);

            [
                (probe.left(), level(probe.left())),
                (probe.right(), level(probe.right())),
            ]
        });

        self.plot.paint(
            ui,
            plot,
            segments,
            Some(CurveMap { amount, bipolar }),
            playhead,
        );
    }
}

fn square_in(rect: Rect, ppt: f32) -> Rect {
    let side = rect.width().min(rect.height());
    Rect::from_center_size(rect.center(), Vec2::splat(side)).round_to_pixels(ppt)
}

impl GridWidgetContent for ModMapWidget {
    fn grid_size(&self) -> GridVec {
        GridVec::new(2, 2)
    }

    fn show_label(&self) -> bool {
        false
    }

    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut WidgetCtx, module_id: ModuleId) {
        let has_active_voices = ctx.bridge.has_active_voices();

        ctx.bridge
            .with_module_bridge(module_id, |bridge, module_bridge| {
                if let ModuleBridge::ModMap(map_bridge) = module_bridge {
                    self.map_ui(ui, bridge, map_bridge, module_id, has_active_voices);
                }
            });
    }
}
