use addi_dsp::segments::Segments;
use egui::{Popup, Response, Ui};

use super::consts as C;
use super::utils::{Ctx, Handle};

/// Handle whose context menu is open.
/// The menu closes unless that handle is interacted again this frame.
pub(super) fn open_menu(ctx: &Ctx<'_>) -> Option<Handle> {
    (0..ctx.segs.num_points()).find_map(|index| {
        if menu_open(ctx, (C::POINT_ID, index)) {
            Some(Handle::Point(index))
        } else if menu_open(ctx, (C::POWER_ID, index)) {
            Some(Handle::Power(index))
        } else {
            None
        }
    })
}

fn menu_open(ctx: &Ctx<'_>, salt: impl std::hash::Hash + std::fmt::Debug) -> bool {
    let id = ctx.ui.make_persistent_id(salt).with("popup");

    Popup::is_id_open(ctx.ui.ctx(), id)
}

/// `true` when the menu asked to remove this point. Other choices edit the curve directly.
pub(super) fn point_menu(
    markers: bool,
    ctx: &mut Ctx<'_>,
    response: &Response,
    index: usize,
) -> bool {
    let mut remove = false;

    response.context_menu(|ui| {
        if index >= ctx.segs.num_points() {
            return;
        }

        smooth_item(ui, ctx.segs, index);

        if markers {
            marker_items(ui, ctx.segs, index);
        }

        if ctx.is_inner(index) && ui.button("Remove point").clicked() {
            remove = true;
            ui.close();
        }
    });

    remove
}

pub(super) fn power_menu(ctx: &mut Ctx<'_>, response: &Response, index: usize) {
    response.context_menu(|ui| {
        if index < ctx.segs.num_points() {
            smooth_item(ui, ctx.segs, index);
        }

        if ui.button("Reset bend").clicked() {
            ctx.segs.set_power(index, 0.0);
            ui.close();
        }
    });
}

pub(super) fn canvas_menu(ctx: &mut Ctx<'_>) {
    ctx.res.context_menu(|ui| {
        if ui.button("Flip horizontal").clicked() {
            ctx.segs.flip_horizontal();
            ui.close();
        }

        if ui.button("Flip vertical").clicked() {
            ctx.segs.flip_vertical();
            ui.close();
        }

        if ui.button("Reset bends").clicked() {
            ctx.segs.reset_powers();
            ui.close();
        }
    });
}

/// Toggles the sine easing of segment `index`.
fn smooth_item(ui: &mut Ui, segments: &mut Segments, index: usize) {
    let smooth = segments.point(index).smooth;
    let label = if smooth { "Flat" } else { "Smooth" };

    if ui.button(label).on_hover_text(C::SMOOTH_TIP).clicked() {
        segments.set_smooth(index, !smooth);
        ui.close();
    }
}

/// Loop and release choices for point `index`.
fn marker_items(ui: &mut Ui, segments: &mut Segments, index: usize) {
    if segments.loop_point_idx() == Some(index) {
        if ui.button("Clear loop").clicked() {
            segments.set_loop_point(None);
            ui.close();
        }
    } else {
        let can_loop = index + 1 < segments.num_points()
            && segments
                .release_point_idx()
                .is_none_or(|release| index < release);

        if can_loop && ui.button("Loop point").on_hover_text(C::LOOP_TIP).clicked() {
            segments.set_loop_point(Some(index));
            ui.close();
        }
    }

    if segments.release_point_idx() == Some(index) {
        if ui.button("Clear release").clicked() {
            segments.set_release_point(None);
            ui.close();
        }
    } else {
        let can_release = segments
            .loop_point_idx()
            .is_none_or(|loop_point| index > loop_point);

        if can_release
            && ui
                .button("Release point")
                .on_hover_text(C::RELEASE_TIP)
                .clicked()
        {
            segments.set_release_point(Some(index));
            ui.close();
        }
    }
}
