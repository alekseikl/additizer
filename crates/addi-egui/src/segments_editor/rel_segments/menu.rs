use egui::Response;

use super::super::common::{self, Handle};
use super::consts as C;
use super::utils::Ctx;

pub(super) use super::super::common::{Axis, PointAction};

/// Handle whose context menu is open.
/// The menu closes unless that handle is interacted again this frame.
pub(super) fn open_menu(ctx: &Ctx<'_>) -> Option<Handle> {
    common::open_handle(ctx.ui, ctx.segs.num_points(), C::POINT_ID, C::POWER_ID)
}

/// What the menu asked to do with this point.
pub(super) fn point_menu(
    markers: bool,
    ctx: &mut Ctx<'_>,
    response: &Response,
    index: usize,
) -> Option<PointAction> {
    common::point_menu(response, ctx.segs, index, "Set phase", markers)
}

pub(super) fn power_menu(ctx: &mut Ctx<'_>, response: &Response, index: usize) {
    common::power_menu(response, ctx.segs, index);
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

        common::reset_bends_item(ui, ctx.segs);
    });
}
