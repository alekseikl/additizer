use egui::ecolor::Hsva;

pub const fn hsva(h: f32, s: f32, v: f32, a: f32) -> Hsva {
    Hsva { h, s, v, a }
}

#[macro_export]
macro_rules! show_modal {
    ($self:ident, $state:ident, $func:ident, $synth:ident, $ui:ident) => {
        if let Some(mut state) = $self.$state.take()
            && $self.$func($synth, $ui, &mut state)
        {
            $self.$state.replace(state);
        }
    };
}
