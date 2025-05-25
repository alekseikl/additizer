use iced_baseview::{
    Border, Color, Element, Event, Length, Point, Rectangle, Shadow, Size, Theme,
    core::{
        self, Layout, Text, Widget, event, layout, mouse,
        renderer::{self, Quad},
        text::LineHeight,
        widget::Tree,
    },
    graphics,
};

const SLIDER_WIDTH: f32 = 20.0;
const SLIDER_MIN_HEIGHT: f32 = 60.0;

// #[derive(Clone, Copy, PartialEq)]
pub struct SliderControl {
    vertical: bool,
    toggle: bool,
}

impl SliderControl {
    pub fn new(vertical: bool) -> Self {
        Self {
            vertical,
            toggle: false,
        }
    }
}

impl<Message, Renderer> Widget<Message, Theme, Renderer> for SliderControl
where
    Renderer: core::text::Renderer + core::image::Renderer + graphics::geometry::Renderer,
{
    fn size(&self) -> Size<Length> {
        Size {
            width: Length::Fixed(SLIDER_WIDTH),
            height: Length::Fill,
        }
    }

    fn layout(
        &self,
        _tree: &mut Tree,
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        layout::Node::new(limits.resolve(
            Length::Fixed(SLIDER_WIDTH),
            Length::Fill,
            Size {
                width: SLIDER_WIDTH,
                height: SLIDER_MIN_HEIGHT,
            },
        ))
    }

    fn on_event(
        &mut self,
        _state: &mut Tree,
        event: core::Event,
        _layout: Layout<'_>,
        _cursor: core::mouse::Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn core::Clipboard,
        _shell: &mut core::Shell<'_, Message>,
        _viewport: &Rectangle,
    ) -> core::event::Status {
        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                self.toggle = !self.toggle;
                return event::Status::Captured;
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right)) => {
                self.toggle = !self.toggle;
                return event::Status::Captured;
            }
            _ => (),
        }

        event::Status::Ignored
    }

    fn draw(
        &self,
        _state: &Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        renderer.fill_quad(
            Quad {
                bounds: layout.bounds(),
                border: Border {
                    color: Color::from_rgb(0.6, 0.8, 1.0),
                    width: 1.0,
                    radius: 4.0.into(),
                },
                shadow: Shadow::default(),
            },
            Color::from_rgb(0.0, 0.2, 0.4),
        );

        renderer.fill_text(
            Text {
                content: if self.toggle {
                    "Enabled".to_string()
                } else {
                    "Disabled".to_string()
                },
                font: renderer.default_font(),
                size: core::Pixels(12.0),
                bounds: Size {
                    width: 200.0,
                    height: 200.0,
                },
                horizontal_alignment: core::alignment::Horizontal::Left,
                vertical_alignment: core::alignment::Vertical::Center,
                line_height: LineHeight::Relative(1.0),
                shaping: core::text::Shaping::Basic,
                wrapping: core::text::Wrapping::None,
            },
            layout.position(),
            Color::from_rgb(0.6, 0.8, 1.0),
            *viewport,
        );
    }
}

impl<Message, Renderer> From<SliderControl> for Element<'_, Message, Theme, Renderer>
where
    Renderer: core::text::Renderer + core::image::Renderer + graphics::geometry::Renderer,
{
    fn from(widget: SliderControl) -> Self {
        Self::new(widget)
    }
}
