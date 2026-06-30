use egui::{Context, Pos2, Response, Tooltip, Ui, WidgetText, emath::RectAlign};

const DELAY: f32 = 0.25;

fn is_visible(ctx: &Context, response: &Response) -> bool {
    let hover_start_id = response.id.with("input_tooltip_hover");

    if !response.enabled() || !response.hovered() || response.dragged() {
        ctx.data_mut(|d| d.remove_temp::<f64>(hover_start_id));
        return false;
    }

    let now = ctx.input(|i| i.time);
    let elapsed = ctx.data_mut(|d| {
        let hover_start = d.get_temp_mut_or(hover_start_id, now);
        (now - *hover_start) as f32
    });

    if elapsed >= DELAY {
        true
    } else {
        ctx.request_repaint_after_secs(DELAY - elapsed);
        false
    }
}

pub fn show_above(ui: &Ui, response: &Response, anchor: Pos2, label: impl Into<WidgetText>) {
    let global_anchor = ui
        .ctx()
        .layer_transform_to_global(response.layer_id)
        .map(|transform| transform * anchor)
        .unwrap_or(anchor);

    let mut tooltip = Tooltip::for_widget(response);
    tooltip.popup = tooltip
        .popup
        .anchor(global_anchor)
        .align(RectAlign::TOP)
        .open(is_visible(ui.ctx(), response));
    tooltip.show(|ui| {
        ui.set_max_width(ui.spacing().tooltip_width);
        ui.label(label);
    });
}
