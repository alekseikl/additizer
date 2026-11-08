use egui::Response;

use super::super::common::{self, Handle};
use super::consts as C;
use super::utils::Ctx;

pub(super) use super::super::common::{Axis, PointAction};

pub fn open_menu(ctx: &Ctx<'_>) -> Option<Handle> {
    common::open_handle(ctx.ui, ctx.segs.num_points(), C::POINT_ID, C::POWER_ID)
}

/// What the menu asked to do with this point.
pub fn point_menu(
    ctx: &mut Ctx<'_>,
    response: &Response,
    index: usize,
) -> Option<PointAction> {
    common::point_menu(response, ctx.segs, index, "Set time", true)
}

pub fn power_menu(ctx: &mut Ctx<'_>, response: &Response, index: usize) {
    common::power_menu(response, ctx.segs, index);
}

pub fn canvas_menu(ctx: &mut Ctx<'_>) {
    ctx.res.context_menu(|ui| {
        common::reset_bends_item(ui, ctx.segs);
    });
}
