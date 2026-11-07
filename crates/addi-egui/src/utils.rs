use egui::{
    Color32, ColorImage, Context, Order, Response, TextureHandle, TextureId, TextureOptions,
    ecolor::Hsva,
};

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

const FILL_TOP_ALPHA: u8 = 0x47;
const FILL_BOTTOM_ALPHA: u8 = 0x28;

/// Cached 1×2 plot gradient. V 0.25 and 0.75 sit on the texel centers.
pub(crate) struct FillTexture {
    name: &'static str,
    color: Color32,
    texture: Option<TextureHandle>,
}

impl FillTexture {
    pub(crate) fn new(name: &'static str, color: Color32) -> Self {
        Self {
            name,
            color,
            texture: None,
        }
    }

    pub(crate) fn id(&mut self, ctx: &Context) -> TextureId {
        if let Some(texture) = &self.texture {
            return texture.id();
        }

        let texture = ctx.load_texture(
            self.name,
            ColorImage::new(
                [1, 2],
                vec![
                    with_alpha(self.color, FILL_TOP_ALPHA),
                    with_alpha(self.color, FILL_BOTTOM_ALPHA),
                ],
            ),
            TextureOptions::LINEAR,
        );
        let id = texture.id();

        self.texture = Some(texture);
        id
    }
}

fn with_alpha(color: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha)
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
