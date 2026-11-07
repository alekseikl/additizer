use addi_dsp::segments::time_segments::TimeSegments;
use egui::{Popup, Response, Ui};

use super::consts as C;
use super::utils::{Ctx, Handle};

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

/// Coordinate of a breakpoint the menu asked to edit.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum Axis {
    #[default]
    X,
    Y,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum PointAction {
    Remove,
    Time,
    Level,
}

impl PointAction {
    pub(super) fn axis(self) -> Option<Axis> {
        match self {
            Self::Time => Some(Axis::X),
            Self::Level => Some(Axis::Y),
            Self::Remove => None,
        }
    }
}

/// What the menu asked to do with this point.
pub(super) fn point_menu(
    ctx: &mut Ctx<'_>,
    response: &Response,
    index: usize,
) -> Option<PointAction> {
    let mut action = None;

    response.context_menu(|ui| {
        if index >= ctx.segs.num_points() {
            return;
        }

        if ui.button("Set time").clicked() {
            action = Some(PointAction::Time);
            ui.close();
        }

        if ui.button("Set level").clicked() {
            action = Some(PointAction::Level);
            ui.close();
        }

        smooth_item(ui, ctx.segs, index);
        marker_items(ui, ctx.segs, index);

        if ctx.is_inner(index) && ui.button("Remove point").clicked() {
            action = Some(PointAction::Remove);
            ui.close();
        }
    });

    action
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
        if ui.button("Reset bends").clicked() {
            ctx.segs.reset_powers();
            ui.close();
        }
    });
}

fn smooth_item(ui: &mut Ui, segments: &mut TimeSegments, index: usize) {
    let smooth = segments.point(index).curve.smooth;
    let label = if smooth { "Flat" } else { "Smooth" };

    if ui.button(label).on_hover_text(C::SMOOTH_TIP).clicked() {
        segments.set_smooth(index, !smooth);
        ui.close();
    }
}

fn marker_items(ui: &mut Ui, segments: &mut TimeSegments, index: usize) {
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
        let can_release = index > 0
            && index + 1 < segments.num_points()
            && segments
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
