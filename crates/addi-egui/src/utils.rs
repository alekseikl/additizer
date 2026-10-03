use egui::{Order, Response, ecolor::Hsva};

pub const fn hsva(h: f32, s: f32, v: f32, a: f32) -> Hsva {
    Hsva { h, s, v, a }
}

/// True if a click hit a nested popup/menu layer above `response`.
///
/// Slider context menus and the enter-value overlay live on a separate
/// `Order::Foreground` layer, so `CloseOnClickOutside` would otherwise
/// treat them as outside clicks.
fn clicked_nested_popup(response: &Response) -> bool {
    let ctx = &response.ctx;
    let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) else {
        return false;
    };
    ctx.layer_id_at(pos)
        .is_some_and(|layer| layer != response.layer_id && layer.order >= Order::Foreground)
}

pub fn popup_should_close(response: &Response) -> bool {
    response.should_close() && !clicked_nested_popup(response)
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
