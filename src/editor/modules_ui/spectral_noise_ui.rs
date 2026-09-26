use egui::{Checkbox, ComboBox, DragValue, Grid, Ui};

use crate::{
    editor::{
        ModuleUi,
        module_label::ModuleLabel,
        slider::Slider,
        stereo_input::StereoInput,
        units::{OctavesDisplay, Units},
    },
    synth_engine::{
        Input, MAX_BANDWIDTH, ModuleId, ModuleType,
        spectral_noise::{
            MAX_ROLLOFF, MIN_ROLLOFF, NoiseColor, SpectralNoiseUiBridge,
        },
        ui_bridge::{ModuleBridge, UiBridge},
    },
    utils::{MAX_CUTOFF, MIN_CUTOFF, from_st},
};

pub struct SpectralNoiseUi {
    module_id: ModuleId,
}

impl SpectralNoiseUi {
    pub fn new(module_id: ModuleId) -> Self {
        Self { module_id }
    }

    fn paint_ui(
        &mut self,
        bridge: &mut UiBridge,
        noise_bridge: &mut SpectralNoiseUiBridge,
        ui: &mut Ui,
    ) {
        let module_id = self.module_id;
        let mut config = noise_bridge.config().clone();

        ui.add(ModuleLabel::new(
            module_id,
            ModuleType::SpectralNoise,
            bridge,
        ));

        ui.add_space(16.0);

        Grid::new(("spectral-noise", module_id))
            .num_columns(4)
            .spacing([8.0, 8.0])
            .show(ui, |ui| {
                ui.label("Color").on_hover_text(
                    "White is flat. Pink falls 3 dB/octave. Brown falls 6 dB/octave.",
                );
                ComboBox::from_id_salt(("spectral-noise-color", module_id))
                    .selected_text(config.color.label())
                    .show_ui(ui, |ui| {
                        for color in NoiseColor::ALL {
                            if ui
                                .selectable_value(&mut config.color, color, color.label())
                                .clicked()
                            {
                                noise_bridge.set_color(color);
                            }
                        }
                    });

                ui.label("Bandwidth")
                    .on_hover_text("Fixed harmonic count, or note-based bandlimiting at 24 kHz.");
                ui.horizontal(|ui| {
                    let mut bandwidth = config.bandwidth;
                    let mut note_based = bandwidth == 0;

                    if !note_based
                        && ui
                            .add(DragValue::new(&mut bandwidth).range(1..=MAX_BANDWIDTH as i32))
                            .changed()
                    {
                        noise_bridge.set_bandwidth(bandwidth);
                    }

                    if ui
                        .add(Checkbox::new(&mut note_based, "Note based"))
                        .changed()
                    {
                        noise_bridge.set_bandwidth(if note_based {
                            0
                        } else {
                            MAX_BANDWIDTH as i32
                        });
                    }
                });
                ui.end_row();

                ui.label("Amount").on_hover_text(
                    "Scales how far each harmonic's phase turns between frames. At 100%, harmonics at and above the cutoff get a new random phase.",
                );
                if ui
                    .add(StereoInput::new(
                        Input::Amount,
                        module_id,
                        &mut config.amount,
                        bridge,
                    ))
                    .changed()
                {
                    noise_bridge.set_param(Input::Amount, config.amount);
                }

                ui.label("Level");
                if ui
                    .add(StereoInput::new(
                        Input::Level,
                        module_id,
                        &mut config.level,
                        bridge,
                    ))
                    .changed()
                {
                    noise_bridge.set_param(Input::Level, config.level);
                }
                ui.end_row();

                ui.label("Cutoff").on_hover_text(
                    "Harmonics below this frequency get a smaller phase turn. At it and above, the turn is Amount.",
                );
                if ui
                    .add(
                        Slider::stereo(&mut config.cutoff, MIN_CUTOFF..=MAX_CUTOFF, None)
                            .default(from_st(0.0))
                            .units(Units::Octaves(OctavesDisplay::Frequency)),
                    )
                    .changed()
                {
                    noise_bridge.set_cutoff(config.cutoff);
                }

                ui.label("Rolloff").on_hover_text(
                    "Phase-turn rolloff below Cutoff, in dB per octave. The attenuation is converted to gain and scales Amount.",
                );
                if ui
                    .add(
                        Slider::stereo(
                            &mut config.rolloff,
                            MIN_ROLLOFF..=MAX_ROLLOFF,
                            None,
                        )
                        .default((MIN_ROLLOFF + MAX_ROLLOFF) * 0.5)
                        .units(Units::Rolloff),
                    )
                    .changed()
                {
                    noise_bridge.set_rolloff(config.rolloff);
                }
                ui.end_row();

                ui.label("Steal phase").on_hover_text(
                    "On a new note, take harmonic phases from the replaced voice. Otherwise draw new random phases.",
                );
                if ui
                    .add(Checkbox::without_text(&mut config.steal_phase))
                    .changed()
                {
                    noise_bridge.set_steal_phase(config.steal_phase);
                }

                ui.label("Stereo")
                    .on_hover_text("When off, both channels share the same harmonics.");
                if ui.add(Checkbox::without_text(&mut config.stereo)).changed() {
                    noise_bridge.set_stereo(config.stereo);
                }
                ui.end_row();
            });
    }
}

impl ModuleUi for SpectralNoiseUi {
    fn module_id(&self) -> Option<ModuleId> {
        Some(self.module_id)
    }

    fn ui(&mut self, bridge: &mut UiBridge, ui: &mut Ui) {
        bridge.with_module_bridge(self.module_id, |bridge, module_bridge| {
            if let ModuleBridge::SpectralNoise(noise_bridge) = module_bridge {
                self.paint_ui(bridge, noise_bridge, ui);
            }
        });
    }
}
