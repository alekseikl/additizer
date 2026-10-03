use std::sync::Arc;

use egui::{Ui, Vec2};
use nice_plug::context::gui::GuiContext;
use nice_plug::editor::dpi::LogicalSize;
use nice_plug::prelude::*;
use nice_plug_egui::{
    EguiEditor, EguiEditorState, EguiNiceSettings, NiceEguiApp, RepaintNotifier,
    create_egui_editor, resizable_window::ResizableWindow,
};

use addi_egui::{EditorState, install_fonts};
use addi_ui_backend::engine_factory::EngineFactory;

const MIN_WINDOW_SIZE: LogicalSize<f32> = LogicalSize::new(640.0, 480.0);
const INITIAL_WINDOW_SIZE: LogicalSize<f32> = LogicalSize::new(900.0, 600.0);
const INITIAL_ZOOM_FACTOR: f32 = 1.0;
const RESIZE_HINT: ResizeHint = ResizeHint::resizable().with_min_logical_size(MIN_WINDOW_SIZE);

/// nice-plug window around the egui editor.
pub struct PluginEditor {
    state: EditorState,
}

impl PluginEditor {
    pub fn new(factory: Arc<EngineFactory>) -> Self {
        Self {
            state: EditorState::new(factory),
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

    fn ui(&mut self, ui: &mut Ui, _frame: &mut nice_plug_egui::Frame) {
        ResizableWindow::new("res-wind")
            .min_size(Vec2::new(MIN_WINDOW_SIZE.width, MIN_WINDOW_SIZE.height))
            .show(ui, |ui| {
                self.state.show(ui);
            });
        ui.ctx().request_repaint();
    }
}

pub fn create_editor(
    egui_state: Arc<EguiEditorState>,
    repaint_notifier: RepaintNotifier,
    factory: Arc<EngineFactory>,
) -> Option<EguiEditor<PluginEditor>> {
    create_egui_editor(
        egui_state,
        repaint_notifier,
        EguiNiceSettings::new().with_resize_hint(RESIZE_HINT),
        PluginEditor::new(factory),
    )
}

pub fn new_editor_state() -> Arc<EguiEditorState> {
    EguiEditorState::from_size(INITIAL_WINDOW_SIZE, INITIAL_ZOOM_FACTOR)
}
