use egui::{Label, Response, TextStyle, Ui, Widget};

pub struct FitLabel<'a> {
    label: &'a str,
    short_label: &'a str,
}

impl<'a> FitLabel<'a> {
    pub fn new(label: &'a str, short_label: &'a str) -> Self {
        Self { label, short_label }
    }
}

impl Widget for FitLabel<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let font_id = TextStyle::Body.resolve(ui.style());
        let text_color = ui.visuals().text_color();
        let galley =
            ui.fonts_mut(|fonts| fonts.layout_no_wrap(self.label.to_owned(), font_id, text_color));

        if galley.size().x <= ui.available_width() {
            ui.add(Label::new(self.label).selectable(false))
        } else {
            ui.add(Label::new(self.short_label).selectable(false).truncate())
                .on_hover_text(self.label)
        }
    }
}
