use super::*;

#[test]
fn fill_texture_reloads_when_the_egui_context_is_replaced() {
    let mut fill = FillTexture::new(Color32::from_rgb(0x3d, 0xb9, 0xff));
    let first = Context::default();
    let first_id = fill.id(&first);

    assert_eq!(fill.id(&first), first_id);
    assert_eq!(
        texture_name(&first, first_id).as_deref(),
        Some(FILL_TEXTURE_NAME)
    );

    drop(first);

    let second = Context::default();
    let second_id = fill.id(&second);

    assert_eq!(
        texture_name(&second, second_id).as_deref(),
        Some(FILL_TEXTURE_NAME)
    );
}

fn texture_name(ctx: &Context, id: TextureId) -> Option<String> {
    ctx.tex_manager()
        .read()
        .meta(id)
        .map(|meta| meta.name.clone())
}
