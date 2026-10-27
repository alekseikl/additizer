#![allow(clippy::new_without_default)]

//! egui editor: module grid, detail panels, and value widgets.

use std::sync::Arc;

use egui::{
    CentralPanel, FontData, FontDefinitions, FontFamily, Frame, Id, Margin, Panel, ScrollArea, Ui,
    vec2,
};

use addi_engine::{ModuleId, ModuleType};
use addi_ui_backend::{engine_factory::EngineFactory, ui_bridge::UiBridge};

mod bin_slider;
mod circle_grab;
mod control_meter;
mod fit_label;
mod frequency_bins;
mod grid;
mod module_label;
mod modules_ui;
mod routing_ui_ext;
mod segment_presets;
mod segments_editor;
mod slider;
mod stereo_input;
mod units;
mod utils;
mod volume_meter;
mod waveform;

use grid::GridEvent;
use modules_ui::{
    AmplifierUI, EnvelopeUI, ExpressionsUi, ExternalParamUI, HarmonicEditorUI, LfoUi, MixerUi,
    OscillatorUI, OutputUi, ParamsUi, PitchUi, SegmentsEnvelopeUi, SpectralBandSelectUi,
    SpectralBlendUi, SpectralEqUi, SpectralFilterUI, SpectralMixerUi, SpectralNoiseUi, SvfUi,
    WaveShaperUI,
};

pub(crate) trait ModuleUi {
    fn module_id(&self) -> Option<ModuleId>;
    fn ui(&mut self, bridge: &mut UiBridge, ui: &mut Ui);
}

type ModuleUIBox = Box<dyn ModuleUi + Send>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum DetailViewKey {
    Params,
    Module(ModuleId),
}

impl DetailViewKey {
    fn from_view(view: &ModuleUIBox) -> Self {
        match view.module_id() {
            Some(id) => DetailViewKey::Module(id),
            None => DetailViewKey::Params,
        }
    }
}

pub struct EguiEditor {
    grid_module_ui: Option<ModuleUIBox>,
    grid: grid::Grid,
}

impl EguiEditor {
    pub fn new() -> Self {
        Self {
            grid_module_ui: None,
            grid: grid::Grid::new(),
        }
    }

    pub fn reset(&mut self) {
        let showing_params = self
            .grid_module_ui
            .as_ref()
            .is_some_and(|panel| panel.module_id().is_none());

        if !showing_params {
            self.grid_module_ui = None;
        }

        self.grid = grid::Grid::new();
    }

    pub fn show(
        &mut self,
        ui: &mut Ui,
        engine_factory: &Arc<EngineFactory>,
        ui_bridge: &mut UiBridge,
    ) {
        if let Some(modules_io) = ui_bridge.take_modules_io() {
            self.grid.update_widgets(modules_io, ui_bridge);
        }

        if self
            .grid_module_ui
            .as_ref()
            .and_then(|panel| panel.module_id())
            .is_some_and(|module_id| !ui_bridge.has_module_id(module_id))
        {
            self.grid_module_ui = None;
        }

        for event in self.grid.events() {
            if let GridEvent::Selected(module_id) = event {
                self.grid_module_ui = module_ui_for_id(ui_bridge, *module_id);
            }
        }

        self.show_top_bar(ui, engine_factory);

        let grid_selected_id = self
            .grid_module_ui
            .as_ref()
            .and_then(|panel| panel.module_id());

        if let Some(panel) = self.grid_module_ui.as_ref() {
            let detail_key = DetailViewKey::from_view(panel);

            Panel::bottom(Id::new(("grid-module-detail", detail_key)))
                .resizable(true)
                .default_size(300.0)
                .min_size(80.0)
                .show(ui, |ui| {
                    ScrollArea::vertical()
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            if let Some(module_ui) = &mut self.grid_module_ui {
                                Frame::NONE
                                    .inner_margin(Margin {
                                        left: 16,
                                        right: 16,
                                        top: 8,
                                        bottom: 16,
                                    })
                                    .show(ui, |ui| {
                                        module_ui.ui(ui_bridge, ui);
                                    });
                            }
                        });
                });
        }

        CentralPanel::no_frame().show(ui, |ui| {
            self.grid.ui(ui, ui_bridge, grid_selected_id);
        });
    }

    fn show_top_bar(&mut self, ui: &mut Ui, engine_factory: &Arc<EngineFactory>) {
        Frame::new().inner_margin(vec2(8.0, 4.0)).show(ui, |ui| {
            ui.horizontal(|ui| {
                let showing_params = self
                    .grid_module_ui
                    .as_ref()
                    .is_some_and(|panel| panel.module_id().is_none());

                if ui
                    .selectable_label(showing_params, "Engine settings")
                    .clicked()
                {
                    if showing_params {
                        self.grid_module_ui = None;
                    } else {
                        self.grid_module_ui =
                            Some(Box::new(ParamsUi::new(Arc::clone(engine_factory))));
                    }
                }
            });
        });
    }
}

trait ModuleTypeEditor {
    fn ui(&self, id: ModuleId) -> ModuleUIBox;
}

impl ModuleTypeEditor for ModuleType {
    fn ui(&self, id: ModuleId) -> ModuleUIBox {
        match self {
            Self::Output => Box::new(OutputUi::new()),
            Self::HarmonicEditor => Box::new(HarmonicEditorUI::new(id)),
            Self::SpectralNoise => Box::new(SpectralNoiseUi::new(id)),
            Self::SpectralFilter => Box::new(SpectralFilterUI::new(id)),
            Self::SpectralEq => Box::new(SpectralEqUi::new(id)),
            Self::Amplifier => Box::new(AmplifierUI::new(id)),
            Self::Mixer => Box::new(MixerUi::new(id)),
            Self::Oscillator => Box::new(OscillatorUI::new(id)),
            Self::Envelope => Box::new(EnvelopeUI::new(id)),
            Self::ExternalParam => Box::new(ExternalParamUI::new(id)),
            Self::Lfo => Box::new(LfoUi::new(id)),
            Self::SegmentsEnvelope => Box::new(SegmentsEnvelopeUi::new(id)),
            Self::Pitch => Box::new(PitchUi::new(id)),
            Self::SpectralBandSelect => Box::new(SpectralBandSelectUi::new(id)),
            Self::SpectralBlend => Box::new(SpectralBlendUi::new(id)),
            Self::SpectralMixer => Box::new(SpectralMixerUi::new(id)),
            Self::WaveShaper => Box::new(WaveShaperUI::new(id)),
            Self::Svf => Box::new(SvfUi::new(id)),
            Self::Expressions => Box::new(ExpressionsUi::new(id)),
        }
    }
}

fn module_ui_for_id(bridge: &UiBridge, id: ModuleId) -> Option<ModuleUIBox> {
    bridge
        .get_modules()
        .into_iter()
        .find(|module| module.id == id)
        .map(|module| module.module_type.ui(module.id))
}

/// Registers the bundled bold font as a `FontFamily::Name("Bold")` family so widgets
/// can request true bold glyphs (egui's default bundle only ships Ubuntu-Light).
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();

    fonts.font_data.insert(
        "Ubuntu-Bold".into(),
        Arc::new(FontData::from_static(include_bytes!(
            "../../../assets/fonts/Ubuntu-Bold.ttf"
        ))),
    );
    fonts
        .families
        .insert(FontFamily::Name("Bold".into()), vec!["Ubuntu-Bold".into()]);

    ctx.set_fonts(fonts);
}
