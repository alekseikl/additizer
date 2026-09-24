use egui::{
    Button, Checkbox, Color32, ComboBox, DragAndDrop, DragValue, Grid, Id, Label, Pos2, Rangef,
    Rect, RichText, Stroke, StrokeKind, Ui,
};

use crate::{
    editor::{
        ModuleUi, module_label::ModuleLabel, slider::Slider, stereo_input::StereoInput,
        units::Units,
    },
    synth_engine::{
        Input, ModuleId, ModuleType, Sample,
        filters::spectral_filter::FilterType,
        spectral_eq::{
            EqFilter, MAX_CUTOFF_HZ, MAX_EQ_FILTERS, MAX_Q, MIN_CUTOFF_HZ, MIN_Q,
            SpectralEqUiBridge,
        },
        spectral_filter::{MAX_CUTOFF, MAX_DRIVE, MIN_CUTOFF, MIN_DRIVE},
        ui_bridge::{ModuleBridge, UiBridge},
    },
    utils::from_st,
};

const DRAG_HANDLE: &str = "☰";
const REMOVE_ICON: &str = "❌";
const REMOVE_TINT: Color32 = Color32::from_rgb(0xe0, 0x6a, 0x6a);
const BAND_DRAG_TINT: Color32 = Color32::from_rgb(0xff, 0xb0, 0x00);

#[derive(Clone, Copy)]
struct BandDrag {
    module_id: ModuleId,
    index: usize,
}

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
                        StereoInput::new(Input::Cutoff, module_id, &mut config.cutoff, bridge)
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
        let mut rows = Vec::with_capacity(config.filters.len());

        Grid::new("spectral_eq_filters")
            .num_columns(6)
            .min_col_width(0.0)
            .spacing([8.0, 6.0])
            .show(ui, |ui| {
                ui.label("");
                ui.label("Type");
                ui.label("Cutoff");
                ui.label("Q");
                ui.label("Drive");
                ui.label("");
                ui.end_row();

                for (index, band) in config.filters.iter_mut().enumerate() {
                    let mut changed = false;
                    let drag_id = Id::new(("eq-band-drag", module_id, index));

                    let handle = ui
                        .dnd_drag_source(drag_id, BandDrag { module_id, index }, |ui| {
                            // The drag source is a scope, which otherwise fills the grid cell.
                            ui.set_max_width(ui.spacing().interact_size.y);
                            ui.add(Label::new(DRAG_HANDLE).selectable(false));
                        })
                        .response;

                    let filter_type =
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
                            })
                            .response;

                    let cutoff = ui.add(Self::cutoff_drag(&mut band.cutoff_hz));
                    let q = ui.add(Self::q_drag(&mut band.q));
                    let drive = ui.add(Self::drive_drag(&mut band.drive));
                    changed |= cutoff.changed() || q.changed() || drive.changed();

                    let remove = ui.button(RichText::new(REMOVE_ICON).color(REMOVE_TINT));

                    if remove.clicked() {
                        removed = Some(index as u8);
                    }

                    let row_rect = handle
                        .rect
                        .union(filter_type.rect)
                        .union(cutoff.rect)
                        .union(q.rect)
                        .union(drive.rect)
                        .union(remove.rect);

                    if ui.ctx().is_being_dragged(drag_id) {
                        ui.painter().rect_stroke(
                            row_rect,
                            2.0,
                            Stroke::new(1.0, BAND_DRAG_TINT),
                            StrokeKind::Outside,
                        );
                    }

                    rows.push((index, row_rect));
                    ui.end_row();

                    if changed {
                        eq_bridge.set_filter(index as u8, *band);
                    }
                }
            });

        let moved = Self::dropped_band(ui, module_id, &rows);

        if let Some(index) = removed {
            eq_bridge.remove_filter(index);
        } else if let Some((from, to)) = moved {
            eq_bridge.move_filter(from, to);
        }

        ui.add_space(8.0);

        if ui
            .add_enabled(config.filters.len() < MAX_EQ_FILTERS, Button::new("Add"))
            .clicked()
        {
            eq_bridge.add_filter(EqFilter::default());
        }
    }

    fn dropped_band(ui: &Ui, module_id: ModuleId, rows: &[(usize, Rect)]) -> Option<(u8, u8)> {
        let drag = DragAndDrop::payload::<BandDrag>(ui.ctx())?;

        if drag.module_id != module_id {
            return None;
        }

        let pointer = ui.input(|input| input.pointer.interact_pos())?;
        let (insert_at, y, x_range) = Self::insertion(rows, pointer)?;

        ui.painter()
            .hline(x_range, y, Stroke::new(2.0, BAND_DRAG_TINT));

        if !ui.input(|input| input.pointer.any_released()) {
            return None;
        }

        let to = if drag.index < insert_at {
            insert_at - 1
        } else {
            insert_at
        };

        (drag.index != to).then_some((drag.index as u8, to as u8))
    }

    fn insertion(rows: &[(usize, Rect)], pointer: Pos2) -> Option<(usize, f32, Rangef)> {
        let mut bounds = rows.first()?.1;

        for (_, rect) in rows.iter().skip(1) {
            bounds = bounds.union(*rect);
        }

        let margin = 12.0;
        let over_table = bounds.x_range().contains(pointer.x)
            && pointer.y >= bounds.top() - margin
            && pointer.y <= bounds.bottom() + margin;

        if !over_table {
            return None;
        }

        for &(index, rect) in rows {
            if pointer.y < rect.center().y {
                return Some((index, rect.top(), bounds.x_range()));
            }
        }

        let &(index, rect) = rows.last()?;
        Some((index + 1, rect.bottom(), bounds.x_range()))
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

    fn q_drag(q: &mut Sample) -> DragValue<'_> {
        DragValue::new(q)
            .range(MIN_Q..=MAX_Q)
            .speed(0.01)
            .fixed_decimals(3)
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
