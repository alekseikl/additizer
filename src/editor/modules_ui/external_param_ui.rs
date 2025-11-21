use egui_baseview::egui::{ComboBox, Ui};

use crate::{
    editor::{ModuleUI, module_label::ModuleLabel, utils::confirm_module_removal},
    synth_engine::{ExternalParam, ModuleId, SynthEngine},
};

pub struct ExternalParamUI {
    module_id: ModuleId,
    remove_confirmation: bool,
    label_state: Option<String>,
}

impl ExternalParamUI {
    pub fn new(module_id: ModuleId) -> Self {
        Self {
            module_id,
            remove_confirmation: false,
            label_state: None,
        }
    }

    fn param<'a>(&mut self, synth: &'a mut SynthEngine) -> &'a mut ExternalParam {
        ExternalParam::downcast_mut_unwrap(synth.get_module_mut(self.module_id))
    }
}

impl ModuleUI for ExternalParamUI {
    fn module_id(&self) -> ModuleId {
        self.module_id
    }

    fn ui(&mut self, synth: &mut SynthEngine, ui: &mut Ui) {
        let ui_data = self.param(synth).get_ui();

        ui.add(ModuleLabel::new(
            &ui_data.label,
            &mut self.label_state,
            synth.get_module_mut(self.module_id).unwrap(),
        ));

        ui.add_space(20.0);

        ComboBox::from_id_salt("ext-param-select")
            .selected_text(format!("Param #{}", ui_data.selected_param_index + 1))
            .show_ui(ui, |ui| {
                for i in 0..ui_data.num_of_params {
                    if ui
                        .selectable_label(
                            i == ui_data.selected_param_index,
                            format!("Param #{}", i + 1),
                        )
                        .clicked()
                    {
                        self.param(synth).select_param(i);
                    }
                }
            });

        ui.add_space(40.0);

        if confirm_module_removal(ui, &mut self.remove_confirmation) {
            synth.remove_module(self.module_id);
        }
    }
}
