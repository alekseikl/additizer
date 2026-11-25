use std::sync::Arc;

use egui::{Ui, Vec2};
use nice_plug::context::gui::GuiContext;
use nice_plug::editor::dpi::LogicalSize;
use nice_plug::prelude::*;
use nice_plug_egui::{
    EguiEditorState, EguiNiceSettings, NiceEguiApp, RepaintNotifier, create_egui_editor,
    resizable_window::ResizableWindow,
};
use addi_egui::{EguiEditor, install_fonts};
use addi_ui_backend::addi_state::AddiState;

const MIN_WINDOW_SIZE: LogicalSize<f32> = LogicalSize::new(640.0, 480.0);
const INITIAL_WINDOW_SIZE: LogicalSize<f32> = LogicalSize::new(900.0, 600.0);
const INITIAL_ZOOM_FACTOR: f32 = 1.0;
const RESIZE_HINT: ResizeHint = ResizeHint::resizable().with_min_logical_size(MIN_WINDOW_SIZE);

/// nice-plug window around the egui editor.
pub struct PluginEditor {
    addi_state: Arc<AddiState>,
    state: EguiEditor,
}

impl PluginEditor {
    pub fn new(addi_state: Arc<AddiState>) -> Self {
        Self {
            addi_state,
            state: EguiEditor::new(),
        }
    }
}

impl NiceEguiApp for PluginEditor {
    fn build(
        &mut self,
        egui_ctx: egui::Context,
        _nice_gui_ctx: GuiContext,
        _frame: &mut nice_plug_egui::Frame,
    ) -> Result<(), nice_plug_egui::baseview::HandlerError> {
        #[cfg(debug_assertions)]
        egui_ctx.global_style_mut(|style| style.debug.warn_if_rect_changes_id = false);

        install_fonts(&egui_ctx);
        Ok(())
    }

    fn ui(&mut self, ui: &mut Ui, frame: &mut nice_plug_egui::Frame) {
        let mut slot = self.addi_state.ui_bridge().lock();
        if let Some(bridge) = slot.as_mut()
            && bridge.is_new()
        {
            self.state.reset();
            bridge.reset_new();
        }
        if let Some(bridge) = slot.as_mut() {
            bridge.update();
        }

        ResizableWindow::new("res-wind")
            .min_size(Vec2::new(MIN_WINDOW_SIZE.width, MIN_WINDOW_SIZE.height))
            .show(ui, |ui| {
                let Some(ui_bridge) = slot.as_mut() else {
                    return;
                };

                self.state.show(ui, ui_bridge);
            });
        drop(slot);

        if let Some(preset) = self.state.take_postponed_preset() {
            self.addi_state.load_preset(&preset);
        }

        // `request_repaint` wakes the display link without setting
        // `redraw_requested`, so baseview pauses it again when this frame
        // returns. Ask the window directly to keep drawing every vsync.
        ui.ctx().request_repaint();
        frame.baseview_window().request_redraw();
    }
}

pub fn create_editor(
    egui_state: Arc<EguiEditorState>,
    addi_state: Arc<AddiState>,
) -> Option<nice_plug_egui::EguiEditor<PluginEditor>> {
    create_egui_editor(
        egui_state,
        RepaintNotifier::new(),
        EguiNiceSettings::new().with_resize_hint(RESIZE_HINT),
        PluginEditor::new(addi_state),
    )
}

pub fn new_editor_state() -> Arc<EguiEditorState> {
    EguiEditorState::from_size(INITIAL_WINDOW_SIZE, INITIAL_ZOOM_FACTOR)
}
