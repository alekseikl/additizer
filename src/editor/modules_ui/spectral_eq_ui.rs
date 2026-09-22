use egui::{Button, Checkbox, ComboBox, DragValue, Grid, RichText, Ui};

use crate::{
    editor::{
        ModuleUi, module_label::ModuleLabel, slider::Slider, stereo_input::StereoInput,
        units::Units,
    },
    synth_engine::{
        Input, ModuleId, ModuleType, Sample,
        filters::spectral_filter::{FilterType, MAX_RESONANCE, MIN_RESONANCE},
        spectral_eq::{
            EqFilter, MAX_CUTOFF_HZ, MAX_EQ_FILTERS, MIN_CUTOFF_HZ, SpectralEqUiBridge,
        },
        spectral_filter::{MAX_CUTOFF, MAX_DRIVE, MIN_CUTOFF, MIN_DRIVE},
        ui_bridge::{ModuleBridge, UiBridge},
    },
    utils::from_st,
};

const REMOVE_ICON: &str = "❌";
const REMOVE_TINT: egui::Color32 = egui::Color32::from_rgb(0xe0, 0x6a, 0x6a);

pub struct SpectralEqUi {
    module_id: ModuleId,
}

impl SpectralEqUi {
    pub fn new(module_id: ModuleId) -> Self {
        Self { module_id }
    }

    fn paint_ui(&mut self, bridge: &mut UiBridge, eq_bridge: &mut SpectralEqUiBridge, ui: &mut Ui) {
        let module_id = self.module_id;
        let mut config = eq_bridge.config().clone();

        ui.add(ModuleLabel::new(module_id, ModuleType::SpectralEq, bridge));

        ui.add_space(16.0);

        Grid::new("spectral_eq_grid")
            .num_columns(4)
            .spacing([8.0, 8.0])
            .show(ui, |ui| {
                ui.label("Cutoff");
                if ui
                    .add(
                        StereoInput::new(
                            Input::Cutoff,
                            module_id,
                            &mut config.cutoff,
                            bridge,
                        )
                        .slider(|slider| slider.units(Units::Octaves(false))),
                    )
                    .changed()
                {
                    eq_bridge.set_param(Input::Cutoff, config.cutoff);
                }

                ui.label("Q Limit");
                if ui
                    .add(
                        Slider::stereo(&mut config.q_limit_to, MIN_CUTOFF..=MAX_CUTOFF, None)
                            .default(from_st(12.0))
                            .units(Units::Octaves(true)),
                    )
                    .changed()
                {
                    eq_bridge.set_q_limit_to(config.q_limit_to);
                }
                ui.end_row();

                ui.label("Q Slope");
                if ui
                    .add(Slider::stereo(&mut config.q_limit_slope, 0.0..=1.0, None).default(0.5))
                    .changed()
                {
                    eq_bridge.set_q_limit_slope(config.q_limit_slope);
                }

                ui.label("Linear");
                if ui
                    .add(Checkbox::without_text(&mut config.linear_phase))
                    .changed()
                {
                    eq_bridge.set_linear_phase(config.linear_phase);
                }
                ui.end_row();

                ui.label("Keytrack");
                if ui
                    .add(Slider::mono(&mut config.keytrack, 0.0..=1.0, None).default(0.0))
                    .changed()
                {
                    eq_bridge.set_keytrack(config.keytrack);
                }

                ui.label("Output");
                if ui
                    .add(StereoInput::new(
                        Input::Level,
                        module_id,
                        &mut config.output_level,
                        bridge,
                    ))
                    .changed()
                {
                    eq_bridge.set_param(Input::Level, config.output_level);
                }
                ui.end_row();
            });

        ui.add_space(16.0);

        let mut removed = None;

        Grid::new("spectral_eq_filters")
            .num_columns(5)
            .spacing([8.0, 6.0])
            .show(ui, |ui| {
                ui.label("Type");
                ui.label("Cutoff");
                ui.label("Resonance");
                ui.label("Drive");
                ui.label("");
                ui.end_row();

                for (index, band) in config.filters.iter_mut().enumerate() {
                    let mut changed = false;

                    ComboBox::from_id_salt(format!("eq-filter-type-{module_id}-{index}"))
                        .selected_text(band.filter_type.label())
                        .width(120.0)
                        .show_ui(ui, |ui| {
                            for filter_type in FilterType::ALL {
                                changed |= ui
                                    .selectable_value(
                                        &mut band.filter_type,
                                        filter_type,
                                        filter_type.label(),
                                    )
                                    .changed();
                            }
                        });

                    changed |= ui.add(Self::cutoff_drag(&mut band.cutoff_hz)).changed();
                    changed |= ui.add(Self::resonance_drag(&mut band.resonance)).changed();
                    changed |= ui.add(Self::drive_drag(&mut band.drive)).changed();

                    if ui
                        .button(RichText::new(REMOVE_ICON).color(REMOVE_TINT))
                        .clicked()
                    {
                        removed = Some(index as u8);
                    }

                    ui.end_row();

                    if changed {
                        eq_bridge.set_filter(index as u8, *band);
                    }
                }
            });

        if let Some(index) = removed {
            eq_bridge.remove_filter(index);
        }

        ui.add_space(8.0);

        if ui
            .add_enabled(config.filters.len() < MAX_EQ_FILTERS, Button::new("Add"))
            .clicked()
        {
            eq_bridge.add_filter(EqFilter::default());
        }
    }

    fn cutoff_drag(cutoff_hz: &mut Sample) -> DragValue<'_> {
        let speed = (cutoff_hz.abs() * 0.002).max(0.05);

        DragValue::new(cutoff_hz)
            .range(MIN_CUTOFF_HZ..=MAX_CUTOFF_HZ)
            .speed(speed)
            .custom_formatter(|value, _| Units::Frequency.format(value as Sample))
            .custom_parser(|text| {
                Units::Frequency
                    .parse(text, false)
                    .map(|value| value.left() as f64)
            })
    }

    fn resonance_drag(resonance: &mut Sample) -> DragValue<'_> {
        DragValue::new(resonance)
            .range(MIN_RESONANCE..=MAX_RESONANCE)
            .speed(0.005)
            .custom_formatter(|value, _| Units::Normalized.format(value as Sample))
            .custom_parser(|text| {
                Units::Normalized
                    .parse(text, false)
                    .map(|value| value.left() as f64)
            })
    }

    fn drive_drag(drive: &mut Sample) -> DragValue<'_> {
        DragValue::new(drive)
            .range(MIN_DRIVE..=MAX_DRIVE)
            .speed(0.1)
            .custom_formatter(|value, _| Units::Db.format(value as Sample))
            .custom_parser(|text| {
                Units::Db
                    .parse(text, false)
                    .map(|value| value.left() as f64)
            })
    }
}

impl ModuleUi for SpectralEqUi {
    fn module_id(&self) -> Option<ModuleId> {
        Some(self.module_id)
    }

    fn ui(&mut self, bridge: &mut UiBridge, ui: &mut Ui) {
        bridge.with_module_bridge(self.module_id, |bridge, module_bridge| {
            if let ModuleBridge::SpectralEq(eq_bridge) = module_bridge {
                self.paint_ui(bridge, eq_bridge, ui);
            }
        });
    }
}
