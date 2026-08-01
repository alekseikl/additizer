use egui::containers::menu::menu_style;
use egui::{
    Align, Area, Color32, Frame, Id, Layout, Order, Popup, PopupCloseBehavior, PopupKind, Pos2,
    Response, Sense, Ui,
};

use crate::synth_engine::ModuleType;

const ADDABLE_MODULES: [ModuleType; 12] = [
    ModuleType::HarmonicEditor,
    ModuleType::Oscillator,
    ModuleType::Envelope,
    ModuleType::Lfo,
    ModuleType::SpectralFilter,
    ModuleType::SpectralBlend,
    ModuleType::SpectralMixer,
    ModuleType::ExternalParam,
    ModuleType::Expressions,
    ModuleType::WaveShaper,
    ModuleType::Amplifier,
    ModuleType::Mixer,
];

pub enum AddResult {
    Selected(ModuleType),
    KeepVisible,
    Close,
}

pub struct AddModulePopup {
    pub pos: Pos2,
}

impl AddModulePopup {
    pub fn show(&self, response: &Response, ui: &mut Ui) -> AddResult {
        let ctx = ui.ctx().clone();
        let screen = ctx.content_rect();
        let mut selected = None;

        let backdrop = Area::new(Id::new("add-module-backdrop"))
            .order(Order::Foreground)
            .fixed_pos(screen.min)
            .sense(Sense::click_and_drag())
            .show(&ctx, |ui| {
                let (rect, _) = ui.allocate_exact_size(screen.size(), Sense::click_and_drag());
                ui.painter()
                    .rect_filled(rect, 0.0, Color32::TRANSPARENT);
            });

        let Some(popup) = Popup::from_response(response)
            .kind(PopupKind::Menu)
            .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
            .layout(Layout::top_down_justified(Align::Min))
            .style(menu_style)
            .at_position(self.pos)
            .gap(0.0)
            .frame(Frame::menu(ui.style()))
            .show(|ui| {
                ui.label("Insert");
                ui.separator();

                for module_type in ADDABLE_MODULES {
                    if ui.selectable_label(false, module_type.label()).clicked() {
                        selected = Some(module_type);
                        ui.close();
                    }
                }
            })
        else {
            return AddResult::Close;
        };

        ctx.set_sublayer(backdrop.response.layer_id, popup.response.layer_id);

        if let Some(module_type) = selected {
            return AddResult::Selected(module_type);
        }

        if popup.response.should_close() {
            AddResult::Close
        } else {
            AddResult::KeepVisible
        }
    }
}
