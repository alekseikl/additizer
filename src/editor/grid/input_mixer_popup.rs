use egui::containers::menu::menu_style;
use egui::{
    Align, Area, Color32, FontFamily, FontId, Frame, Grid, Id, Label, LayerId, Layout, Margin,
    Order, Popup, PopupCloseBehavior, PopupKind, Pos2, Response, RichText, Sense, TextFormat,
    TextStyle, Ui,
    text::{LayoutJob, TextWrapping},
    vec2,
};

use crate::{
    editor::slider::{self, Slider},
    synth_engine::{
        Input, InputId, ModuleId, ModuleType, StereoSample,
        ui_bridge::{UiBridge, routing_state::ConnectedInputSource},
    },
    utils::st_to_octave,
};

const MAX_LABEL_WIDTH: f32 = 200.0;
const SLIDER_LENGTH: f32 = 160.0;
const IO_DOT_SIZE: f32 = 8.0;
const REMOVE_ICON: &str = "❌";
const REMOVE_TINT: Color32 = Color32::from_rgb(0xe0, 0x6a, 0x6a);
const MULTIPLY_TINT: Color32 = Color32::from_rgb(0xff, 0xb0, 0x00);

pub struct InputMixerPopup {
    pub module_id: ModuleId,
    pub module_type: ModuleType,
    pub input: Input,
    pub pos: Pos2,
}

impl InputMixerPopup {
    /// Returns `true` if the edit request should be cleared.
    pub fn show(&self, ui: &mut Ui, bridge: &mut UiBridge) -> bool {
        let input_id = InputId::new(self.input, self.module_id);
        let connected = bridge.get_connected_input_sources(input_id);

        if connected.is_empty() {
            return true;
        }

        let ctx_egui = ui.ctx().clone();
        let menu_id = Id::new(("input-mixer-menu", self.module_id, self.input));
        let backdrop_id = menu_id.with("backdrop");
        let screen = ctx_egui.content_rect();

        Area::new(backdrop_id)
            .order(Order::Foreground)
            .fixed_pos(screen.min)
            .sense(Sense::click_and_drag())
            .show(&ctx_egui, |ui| {
                ui.allocate_exact_size(screen.size(), Sense::click_and_drag());
            });

        let Some(popup) = Popup::new(menu_id, ctx_egui.clone(), self.pos, ui.layer_id())
            .kind(PopupKind::Popup)
            .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
            .layout(Layout::top_down(Align::Min))
            .style(menu_style)
            .gap(0.0)
            .frame(Frame::popup(ui.style()).inner_margin(Margin::same(8)))
            .show(|ui| {
                self.title_ui(ui, bridge);
                ui.add_space(8.0);

                let connected = bridge.get_connected_input_sources(input_id);

                Grid::new(("input-mixer-links", self.module_id, self.input))
                    .num_columns(3)
                    .spacing([8.0, 4.0])
                    .min_col_width(0.0)
                    .striped(false)
                    .show(ui, |ui| {
                        for src in &connected {
                            self.link_rows(ui, bridge, input_id, src);
                        }
                    });
            })
        else {
            return true;
        };

        ctx_egui.set_sublayer(
            LayerId::new(Order::Foreground, backdrop_id),
            LayerId::new(Order::Foreground, menu_id),
        );

        popup.response.should_close()
    }

    fn title_ui(&self, ui: &mut Ui, bridge: &UiBridge) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = vec2(6.0, 0.0);

            let row_height = ui.spacing().interact_size.y;
            let (dot_rect, _) =
                ui.allocate_exact_size(vec2(IO_DOT_SIZE, row_height), Sense::empty());
            ui.painter()
                .circle_filled(dot_rect.center(), IO_DOT_SIZE * 0.5, self.input.color());

            let size = TextStyle::Body.resolve(ui.style()).size;
            let color = ui.visuals().text_color();
            let mut job = LayoutJob::default();
            job.append(
                &self.module_type.input_label(self.input),
                0.0,
                TextFormat {
                    font_id: FontId::new(size, FontFamily::Name("Bold".into())),
                    color,
                    ..Default::default()
                },
            );
            job.append(
                &format!(" ({})", bridge.get_module_label(self.module_id)),
                0.0,
                TextFormat {
                    font_id: FontId::new(size, FontFamily::Proportional),
                    color,
                    ..Default::default()
                },
            );
            job.wrap = TextWrapping::truncate_at_width(MAX_LABEL_WIDTH);
            ui.add(Label::new(job).truncate());
        });
    }

    fn link_rows(
        &self,
        ui: &mut Ui,
        bridge: &mut UiBridge,
        input_id: InputId,
        src: &ConnectedInputSource,
    ) {
        ui.with_layout(Layout::top_down(Align::Max), |ui| {
            ui.set_max_width(MAX_LABEL_WIDTH);
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            ui.add(Label::new(&src.label).truncate());

            if let Some(modulation) = src.modulation.as_ref() {
                ui.add_space(0.5);
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing = vec2(2.0, 0.0);

                    ui.add(Label::new(&modulation.label).truncate());

                    Frame::NONE
                        .inner_margin(Margin {
                            top: -1,
                            bottom: 1,
                            ..Default::default()
                        })
                        .show(ui, |ui| {
                            if ui
                                .button(RichText::new("×").color(MULTIPLY_TINT))
                                .on_hover_text("Remove Modulation")
                                .clicked()
                            {
                                bridge.remove_link_modulation(src.src, &input_id);
                            }
                        });
                });
            }
        });

        let mut amount = src.amount;
        let slider_response = ui.add(amount_slider(self.input, &mut amount));

        if slider_response.changed() {
            bridge.set_link_amount(src.src, input_id, amount);
        }

        if remove_button(ui).on_hover_text("Remove Link").clicked() {
            bridge.remove_link(src.src, input_id);
        }

        ui.end_row();
    }
}

fn remove_button(ui: &mut Ui) -> Response {
    ui.button(RichText::new(REMOVE_ICON).color(REMOVE_TINT))
}

fn amount_slider<'a>(input: Input, amount: &'a mut StereoSample) -> Slider<'a> {
    let slider = match input {
        Input::Gain | Input::GainMix(_) => {
            Slider::stereo(amount, 0.0..=1.0, Some(-1.0)).default(0.0)
        }
        Input::Level | Input::LevelMix(_) => Slider::stereo(amount, 0.0..=100.0, Some(-100.0))
            .default(0.0)
            .skew(2.0)
            .units(slider::Units::Db),
        Input::Drive | Input::ClippingLevel => Slider::stereo(amount, 0.0..=24.0, Some(-24.0))
            .default(0.0)
            .units(slider::Units::Db),
        Input::Distortion => Slider::stereo(amount, 0.0..=48.0, Some(-48.0))
            .default(0.0)
            .units(slider::Units::Db),
        Input::Blend | Input::GainsBlend | Input::PhasesBlend => {
            Slider::stereo(amount, 0.0..=1.0, Some(-1.0)).default(0.0)
        }
        Input::Cutoff => Slider::stereo(amount, 0.0..=10.0, Some(-10.0))
            .default(0.0)
            .units(slider::Units::Octaves),
        Input::Resonance => Slider::stereo(amount, -1.0..=1.0, None).default(0.0),
        Input::Detune => Slider::stereo(amount, 0.0..=st_to_octave(1.0), Some(-st_to_octave(1.0)))
            .default(st_to_octave(0.2))
            .units(slider::Units::Octaves),
        Input::DetunePower => Slider::stereo(amount, 0.0..=5.0, Some(-5.0)).default(0.0),
        Input::PitchShift => Slider::stereo(amount, 0.0..=8.0, Some(-8.0))
            .skew(1.8)
            .default(1.0)
            .units(slider::Units::Octaves),
        Input::Glide => Slider::stereo(amount, 0.0..=5.0, Some(-5.0))
            .default(0.0)
            .skew(2.0)
            .units(slider::Units::Time),
        Input::GlideSlope => Slider::stereo(amount, 0.0..=1.0, Some(-1.0))
            .default(0.0)
            .units(slider::Units::Percents),
        Input::PhaseShift => Slider::stereo(amount, 0.0..=1.0, Some(-1.0)).default(0.0),
        Input::FrequencyShift => Slider::stereo(amount, 0.0..=880.0, Some(-880.0))
            .default(0.0)
            .skew(2.0)
            .units(slider::Units::Frequency),
        Input::LowFrequency => Slider::stereo(amount, 0.0..=100.0, Some(-100.0))
            .default(1.0)
            .skew(1.8)
            .units(slider::Units::Frequency),
        Input::Skew => Slider::stereo(amount, 0.0..=1.0, Some(-1.0)).default(0.0),
        Input::Sustain => Slider::stereo(amount, 0.0..=1.0, None)
            .default(0.5)
            .units(slider::Units::Percents),
        Input::Delay | Input::Attack | Input::Hold | Input::Decay | Input::Release => {
            Slider::stereo(amount, 0.0..=8.0, Some(-8.0))
                .default(0.0)
                .skew(2.0)
                .units(slider::Units::Time)
        }
        Input::Spectrum | Input::SpectrumTo | Input::SpectrumMix(_) => {
            Slider::stereo(amount, 0.0..=1.0, Some(-1.0))
                .default(1.0)
                .units(slider::Units::Percents)
        }
        Input::Audio | Input::AudioMix(_) => Slider::stereo(amount, 0.0..=1.0, Some(-1.0)),
    };

    slider.length(SLIDER_LENGTH)
}
