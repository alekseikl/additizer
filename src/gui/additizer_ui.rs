use iced_baseview::{
    Alignment::Center,
    Element,
    Length::Fill,
    Task, Theme,
    futures::Subscription,
    widget::{column, container, keyed_column, slider, text},
};
use nih_plug::nih_log;

use super::slider_control::SliderControl;

#[derive(Debug, Clone, Copy)]
pub enum Message {
    SliderChanged(u8),
    ParameterUpdate,
}

pub struct AdditizerUI {
    value: u8,
}

impl AdditizerUI {
    pub fn new() -> (Self, Task<Message>) {
        (Self { value: 0 }, Task::none())
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::SliderChanged(value) => {
                self.value = value;
            }
            Message::ParameterUpdate => {
                nih_log!("Param Update");
            }
        }

        Task::none()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::none()
    }

    pub fn view(&self) -> Element<Message> {
        // let h_slider = container(
        //     slider(1..=100, self.value, Message::SliderChanged)
        //         .default(50)
        //         .shift_step(5),
        // )
        // .width(250);

        // let text = text(self.value);

        keyed_column![
            ("Slider", SliderControl::new(true)),
            // (h_slider, "Hslider"),
            // (text, "Textt"),
        ]
        .width(Fill)
        .spacing(20)
        .padding(20)
        .into()
    }

    pub fn theme(&self) -> Theme {
        Theme::Dark
    }
}
