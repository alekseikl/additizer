use egui::{Grid, Ui};
use nih_plug::util::{db_to_gain, gain_to_db};

use crate::{
    editor::{ModuleUi, db_slider::DbSlider},
    synth_engine::{ModuleId, OUTPUT_MODULE_ID, StereoSample, ui_bridge::UiBridge},
};

pub struct OutputUi;

impl OutputUi {
    pub fn new() -> Self {
        Self
    }

    fn paint_ui(&mut self, bridge: &mut UiBridge, ui: &mut Ui) {
        ui.heading("Output");

        ui.add_space(20.0);

        let mut gain_db: StereoSample = bridge
            .engine_params()
            .output_gain
            .iter()
            .map(|gain| gain_to_db(*gain))
            .collect();

        Grid::new("output_grid")
            .num_columns(2)
            .spacing([40.0, 24.0])
            .striped(true)
            .show(ui, |ui| {
                ui.label("Volume");
                if ui
                    .add(DbSlider::new(&mut gain_db).max_dbs(6.0).width(200.0))
                    .changed()
                {
                    bridge.set_output_gain(gain_db.iter().copied().map(db_to_gain).collect());
                }
                ui.end_row();
            });

        ui.add_space(8.0);
    }
}

impl ModuleUi for OutputUi {
    fn module_id(&self) -> Option<ModuleId> {
        Some(OUTPUT_MODULE_ID)
    }

    fn ui(&mut self, bridge: &mut UiBridge, ui: &mut Ui) {
        self.paint_ui(bridge, ui);
    }
}
