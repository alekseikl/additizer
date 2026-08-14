use egui::{Grid, Ui};

use crate::{
    editor::{ModuleUi, slider::Slider, units::Units},
    synth_engine::{ModuleId, OUTPUT_MODULE_ID, StereoSample, ui_bridge::UiBridge},
    utils::{db_to_gain, gain_to_db},
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
                    .add(
                        Slider::stereo(&mut gain_db, -100.0..=6.0, None)
                            .default(0.0)
                            .over(0.0)
                            .skew(1.6)
                            .units(Units::Db)
                            .length(200.0),
                    )
                    .changed()
                {
                    bridge.set_output_gain(gain_db.iter().copied().map(db_to_gain).collect());
                }
                ui.end_row();
            });
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
