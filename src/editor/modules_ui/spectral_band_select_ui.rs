use std::ops::RangeInclusive;

use egui::{ComboBox, DragValue, Ui};

use crate::{
    editor::{ModuleUi, module_label::ModuleLabel, units::Units},
    synth_engine::{
        ModuleId, ModuleType, Sample,
        spectral_band_select::{
            BandSelectMode, MAX_BAND_HZ, MAX_HARMONIC, MAX_HARMONIC_END, MIN_BAND_HZ, MIN_HARMONIC,
            SpectralBandSelectUiBridge,
        },
        ui_bridge::{ModuleBridge, UiBridge},
    },
};

pub struct SpectralBandSelectUi {
    module_id: ModuleId,
}

impl SpectralBandSelectUi {
    pub fn new(module_id: ModuleId) -> Self {
        Self { module_id }
    }

    fn paint_ui(
        &mut self,
        bridge: &mut UiBridge,
        select_bridge: &mut SpectralBandSelectUiBridge,
        ui: &mut Ui,
    ) {
        let module_id = self.module_id;
        let mut config = select_bridge.config().clone();

        if config.harmonic_from > config.harmonic_to {
            std::mem::swap(&mut config.harmonic_from, &mut config.harmonic_to);
            select_bridge.set_harmonic_from(config.harmonic_from);
            select_bridge.set_harmonic_to(config.harmonic_to);
        }

        if config.freq_from > config.freq_to {
            std::mem::swap(&mut config.freq_from, &mut config.freq_to);
            select_bridge.set_freq_from(config.freq_from);
            select_bridge.set_freq_to(config.freq_to);
        }

        ui.add(ModuleLabel::new(
            module_id,
            ModuleType::SpectralBandSelect,
            bridge,
        ));

        ui.add_space(16.0);

        ui.horizontal(|ui| {
            ComboBox::from_id_salt(("spectral-band-select-mode", module_id))
                .selected_text(config.mode.label())
                .show_ui(ui, |ui| {
                    for mode in BandSelectMode::ALL {
                        if ui
                            .selectable_value(&mut config.mode, mode, mode.label())
                            .changed()
                        {
                            select_bridge.set_mode(config.mode);
                        }
                    }
                });

            let from_changed = match config.mode {
                BandSelectMode::Harmonic => ui
                    .add(
                        DragValue::new(&mut config.harmonic_from)
                            .range(MIN_HARMONIC..=config.harmonic_to.min(MAX_HARMONIC)),
                    )
                    .changed(),
                BandSelectMode::Frequency => ui
                    .add(Self::freq_drag(
                        &mut config.freq_from,
                        MIN_BAND_HZ..=config.freq_to,
                    ))
                    .changed(),
            };
            ui.label("—");
            let to_changed = match config.mode {
                BandSelectMode::Harmonic => ui
                    .add(
                        DragValue::new(&mut config.harmonic_to)
                            .range(config.harmonic_from..=MAX_HARMONIC_END),
                    )
                    .changed(),
                BandSelectMode::Frequency => ui
                    .add(Self::freq_drag(
                        &mut config.freq_to,
                        config.freq_from..=MAX_BAND_HZ,
                    ))
                    .changed(),
            };

            if from_changed {
                match config.mode {
                    BandSelectMode::Harmonic => {
                        select_bridge.set_harmonic_from(config.harmonic_from);
                    }
                    BandSelectMode::Frequency => {
                        select_bridge.set_freq_from(config.freq_from);
                    }
                }
            }

            if to_changed {
                match config.mode {
                    BandSelectMode::Harmonic => {
                        select_bridge.set_harmonic_to(config.harmonic_to);
                    }
                    BandSelectMode::Frequency => {
                        select_bridge.set_freq_to(config.freq_to);
                    }
                }
            }
        });
    }

    fn freq_drag(freq_hz: &mut Sample, range: RangeInclusive<Sample>) -> DragValue<'_> {
        let speed = (freq_hz.abs() * 0.002).max(0.05);

        DragValue::new(freq_hz)
            .range(range)
            .speed(speed)
            .custom_formatter(|value, _| Units::Frequency.format(value as Sample))
            .custom_parser(|text| {
                Units::Frequency
                    .parse(text, false)
                    .map(|value| value.left() as f64)
            })
    }
}

impl ModuleUi for SpectralBandSelectUi {
    fn module_id(&self) -> Option<ModuleId> {
        Some(self.module_id)
    }

    fn ui(&mut self, bridge: &mut UiBridge, ui: &mut Ui) {
        bridge.with_module_bridge(self.module_id, |bridge, module_bridge| {
            if let ModuleBridge::SpectralBandSelect(select_bridge) = module_bridge {
                self.paint_ui(bridge, select_bridge, ui);
            }
        });
    }
}
