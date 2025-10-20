use nih_plug::{self, util::MINUS_INFINITY_DB};
use vizia_plug::vizia::{prelude::*, vg};

const FILL_CONTAINER_CLASS: &str = "bar-container";

pub type GainSliderOnChange = dyn Fn(&mut EventContext, f32);

#[derive(Data, Clone, Copy)]
pub struct GainSliderParams {
    max_dbs: f32,
    mid_point: f32,
    skew_factor: f32,
}

#[derive(Lens)]
pub struct GainSlider<L: Lens<Target = f32>> {
    index: i32,
    params: GainSliderParams,
    dragging: bool,
    showing_index: bool,

    #[lens(ignore)]
    param_before_drag: f32,

    #[lens(ignore)]
    on_change: Option<Box<GainSliderOnChange>>,

    #[lens(ignore)]
    gain_lense: L,
}

impl Default for GainSliderParams {
    fn default() -> Self {
        Self {
            max_dbs: 48.0,
            mid_point: 0.75,
            skew_factor: 1.4,
        }
    }
}

pub trait GainSliderModifiers {
    fn on_change<F: Fn(&mut EventContext, f32) + 'static>(self, callback: F) -> Self;
}

impl<L> GainSliderModifiers for Handle<'_, GainSlider<L>>
where
    L: Lens<Target = f32>,
{
    fn on_change<F: Fn(&mut EventContext, f32) + 'static>(self, callback: F) -> Self {
        self.modify(|slider| slider.on_change = Some(Box::new(callback)))
    }
}

impl<L> GainSlider<L>
where
    L: Lens<Target = f32>,
{
    pub fn new(cx: &mut Context, index: i32, gain: L) -> Handle<'_, Self>
    where
        L: Lens<Target = f32>,
    {
        let params = GainSliderParams::default();

        Self {
            index,
            params,
            dragging: false,
            showing_index: false,
            param_before_drag: 0.0,
            on_change: None,
            gain_lense: gain,
        }
        .build(cx, |cx| {
            ZStack::new(cx, |cx| {
                let value = gain.map(move |gain| {
                    Self::gain_to_param_static(
                        *gain,
                        params.max_dbs,
                        params.mid_point,
                        params.skew_factor,
                    )
                });

                let value_top = value.map(move |value| Units::Percentage((1.0 - value) * 100.0));

                GainBar::new(cx, params.mid_point, value);

                Binding::new(cx, Self::dragging, move |cx, show| {
                    if show.get(cx) {
                        VStack::new(cx, |cx| {
                            Label::new(cx, gain.map(move |gain| Self::gain_to_db_string(*gain)))
                                .class("dbs-label");
                        })
                        .class("dbs-label-container")
                        .top(value_top);
                    }
                });

                Binding::new(cx, Self::showing_index, move |cx, show| {
                    if show.get(cx) {
                        VStack::new(cx, |cx| {
                            Label::new(cx, index.to_string()).class("dbs-label");
                        })
                        .class("dbs-label-container")
                        .top(value_top);
                    }
                });
            })
            .class(FILL_CONTAINER_CLASS)
            .pointer_events(false);
        })
    }

    fn gain_to_db_string(gain: f32) -> String {
        let dbs = nih_plug::util::gain_to_db(gain);
        if dbs <= MINUS_INFINITY_DB {
            "-Inf".to_string()
        } else {
            format!("{:+.1}", nih_plug::util::gain_to_db(gain))
        }
    }

    fn gain_to_param_static(gain: f32, max_dbs: f32, mid_point: f32, skew_factor: f32) -> f32 {
        let dbs = nih_plug::util::gain_to_db(gain);

        if dbs > 0.0 {
            let normalized = dbs / max_dbs;

            mid_point + (1.0 - mid_point) * normalized.powf(skew_factor.recip())
        } else {
            let normalized = dbs / nih_plug::util::MINUS_INFINITY_DB;

            mid_point * (1.0 - normalized.powf(skew_factor.recip()))
        }
    }

    fn param_to_gain_static(param: f32, max_dbs: f32, mid_point: f32, skew_factor: f32) -> f32 {
        let dbs = if param > mid_point {
            let normalized = (param - mid_point) / (1.0 - mid_point);

            max_dbs * normalized.powf(skew_factor)
        } else {
            let normalized = 1.0 - param / mid_point;

            nih_plug::util::MINUS_INFINITY_DB * normalized.powf(skew_factor)
        };

        nih_plug::util::db_to_gain(dbs)
    }

    fn gain_to_param(&self, gain: f32) -> f32 {
        Self::gain_to_param_static(
            gain,
            self.params.max_dbs,
            self.params.mid_point,
            self.params.skew_factor,
        )
    }

    fn param_to_gain(&self, param: f32) -> f32 {
        Self::param_to_gain_static(
            param,
            self.params.max_dbs,
            self.params.mid_point,
            self.params.skew_factor,
        )
    }

    fn update_gain(&self, cx: &mut EventContext, gain: f32) {
        if let Some(on_change) = &self.on_change {
            on_change(cx, gain);
        }
    }

    fn update_param(&self, cx: &mut EventContext, param: f32) {
        self.update_gain(cx, self.param_to_gain(param));
    }
}

impl<L> View for GainSlider<L>
where
    L: Lens<Target = f32>,
{
    fn element(&self) -> Option<&'static str> {
        Some("gain-slider")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window_event, meta| match window_event {
            WindowEvent::MouseDown(MouseButton::Left) => {
                if let Some(container) = cx.get_entities_by_class(FILL_CONTAINER_CLASS).first()
                    && !cx.modifiers().alt()
                    && !cx.modifiers().ctrl()
                    && cx
                        .cache
                        .get_bounds(*container)
                        .contains_point(cx.mouse().cursor_x, cx.mouse().cursor_y)
                {
                    self.dragging = true;
                    self.param_before_drag = self.gain_to_param(self.gain_lense.get(cx));
                    cx.capture();
                    meta.consume();
                }
            }
            WindowEvent::MouseUp(MouseButton::Left) => {
                self.dragging = false;
                cx.release();
                meta.consume();
            }
            WindowEvent::MouseMove(_x, y) => {
                if let Some(container) = cx.get_entities_by_class(FILL_CONTAINER_CLASS).first() {
                    if self.dragging {
                        let shift = -cx.mouse().button_delta(MouseButton::Left).1
                            / cx.cache.get_height(*container);

                        let new_param = (self.param_before_drag + shift).clamp(0.0, 1.0);
                        self.update_param(cx, new_param);
                        meta.consume();
                    } else if cx.mouse().left.state == MouseButtonState::Pressed {
                        if cx.modifiers().alt() {
                            let rect = cx.cache.get_bounds(*container);
                            let new_param =
                                1.0 - ((y - rect.top()) / rect.height()).clamp(0.0, 1.0);

                            self.update_param(cx, new_param);
                            meta.consume();
                        } else if cx.modifiers().ctrl() {
                            self.update_gain(cx, 1.0);
                            meta.consume();
                        }
                    }
                }
            }
            WindowEvent::MouseDown(MouseButton::Right) => {
                self.showing_index = true;
                cx.capture();
                meta.consume();
            }
            WindowEvent::MouseUp(MouseButton::Right) => {
                self.showing_index = false;
                cx.release();
                meta.consume();
            }
            WindowEvent::MouseDoubleClick(MouseButton::Left) => {
                self.update_gain(cx, 1.0);
                meta.consume();
            }
            WindowEvent::MouseDoubleClick(MouseButton::Right) => {
                self.update_gain(cx, 0.0);
                meta.consume();
            }

            _ => (),
        });
    }
}

struct GainBar<L: Lens<Target = f32>> {
    mid_point: f32,
    value: L,
}

impl<L: Lens<Target = f32>> GainBar<L> {
    fn new(cx: &mut Context, mid_point: f32, value: L) -> Handle<'_, Self> {
        Self { mid_point, value }
            .build(cx, |_| ())
            .class("gain-bar")
            .bind(value, |mut handle, _| handle.needs_redraw())
    }
}

impl<L: Lens<Target = f32>> View for GainBar<L> {
    fn element(&self) -> Option<&'static str> {
        Some("gain-bar")
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &Canvas) {
        let value = self.value.get(cx);
        let bounds = cx.bounds();
        let value_height = value * bounds.height();
        let mut paint = vg::Paint::default();

        cx.draw_background(canvas);

        if value > self.mid_point {
            let midpoint_height = self.mid_point * bounds.height();

            paint.set_color(cx.border_color());
            canvas.draw_rect(
                vg::Rect::from_ltrb(
                    bounds.left(),
                    bounds.top() + bounds.height() - value_height,
                    bounds.right(),
                    midpoint_height,
                ),
                &paint,
            );

            paint.set_color(cx.font_color());
            canvas.draw_rect(
                vg::Rect::from_ltrb(
                    bounds.left(),
                    bounds.top() + bounds.height() - midpoint_height,
                    bounds.right(),
                    bounds.bottom(),
                ),
                &paint,
            );
        } else {
            paint.set_color(cx.font_color());
            canvas.draw_rect(
                vg::Rect::from_ltrb(
                    bounds.left(),
                    bounds.top() + bounds.height() - value_height,
                    bounds.right(),
                    bounds.bottom(),
                ),
                &paint,
            );
        }
    }
}
