use std::sync::Arc;

use egui::{Color32, Vec2, epaint::PathStroke, pos2, vec2};
use parking_lot::Mutex;
use rustc_hash::FxHashMap;

use crate::{
    engine_factory::{EngineHandle, UiConfigHandle},
    links::PluginLinks,
    ui_bridge::{GridVec, UiBridge, ui_config::UiConfig},
};
use addi_engine::{
    EngineConfig, EngineParams, Input, InputId, ModuleId, ModuleType, OUTPUT_MODULE_ID,
    SynthEngine, routing_state::ModuleIo,
};

use super::{Grid, GridEvent, GridRect, GridVecExt, GridWidget};

fn rect(x: i32, y: i32, w: i32, h: i32) -> GridRect {
    GridRect { id: 0, x, y, w, h }
}

fn empty_bridge() -> UiBridge {
    let engine = SynthEngine::<PluginLinks>::try_new(
        &EngineConfig {
            engine: EngineParams::default(),
            modules: Vec::new(),
            links: Vec::new(),
        },
        48_000.0,
    )
    .expect("engine");
    let engine: EngineHandle = Arc::new(Mutex::new(engine));
    let ui_config: UiConfigHandle = Arc::new(Mutex::new(UiConfig::default()));

    UiBridge::create(engine, ui_config).expect("ui bridge")
}

fn take_ios(bridge: &mut UiBridge) -> FxHashMap<ModuleId, ModuleIo> {
    bridge.take_modules_io().expect("modules io")
}

fn mount(grid: &mut Grid, ios: &mut FxHashMap<ModuleId, ModuleIo>, ids: &[ModuleId]) {
    grid.widgets = ids
        .iter()
        .map(|id| GridWidget::new(ios.remove(id).expect("module io")))
        .collect();
}

fn picked_ios(
    ios: &mut FxHashMap<ModuleId, ModuleIo>,
    ids: &[ModuleId],
) -> FxHashMap<ModuleId, ModuleIo> {
    ids.iter()
        .map(|id| (*id, ios.remove(id).expect("module io")))
        .collect()
}

/// Five connected inputs make an oscillator tile 3 cells tall.
fn connect_five_osc_inputs(bridge: &mut UiBridge, osc: ModuleId) {
    let lfo = bridge.add_module(ModuleType::Lfo, GridVec::new(20, 0));

    for input in [
        Input::Detune,
        Input::DetunePower,
        Input::PhaseSteal,
        Input::PhasesBlend,
        Input::GainsBlend,
    ] {
        bridge.create_link(lfo, InputId::new(input, osc));
    }
}

fn module_ids(bridge: &UiBridge) -> Vec<ModuleId> {
    let mut ids: Vec<_> = bridge
        .get_modules()
        .iter()
        .map(|module| module.id)
        .collect();
    ids.sort_unstable();
    ids
}

fn added_ids(before: &[ModuleId], bridge: &UiBridge) -> Vec<ModuleId> {
    module_ids(bridge)
        .into_iter()
        .filter(|id| !before.contains(id))
        .collect()
}

#[test]
fn rect_reports_edges_and_overlap() {
    let tile = rect(1, 2, 4, 2);

    assert_eq!(tile.right(), 5);
    assert_eq!(tile.bottom(), 4);
    assert!(tile.overlaps(&rect(4, 3, 2, 2)));
    assert!(rect(0, 0, 4, 4).overlaps(&rect(1, 1, 1, 1)));
    assert!(!tile.overlaps(&rect(5, 2, 2, 2)));
    assert!(!tile.overlaps(&rect(1, 4, 4, 2)));
}

#[test]
fn free_position_is_directly_below_when_clear() {
    let pos = Grid::free_position_below(
        GridVec::new(1, 2),
        GridVec::new(4, 2),
        GridVec::new(4, 2),
        &[],
    );

    assert_eq!(pos, GridVec::new(1, 4));
}

#[test]
fn free_position_skips_overlapping_tiles() {
    let occupied = [rect(0, 0, 4, 2), rect(4, 2, 2, 2), rect(3, 2, 2, 2)];
    let pos = Grid::free_position_below(
        GridVec::ZERO,
        GridVec::new(4, 2),
        GridVec::new(4, 2),
        &occupied,
    );

    assert_eq!(pos, GridVec::new(0, 4));
}

#[test]
fn free_position_steps_to_the_lowest_overlapping_bottom() {
    let occupied = [rect(0, 2, 4, 1), rect(2, 2, 2, 3)];
    let pos = Grid::free_position_below(
        GridVec::ZERO,
        GridVec::new(4, 2),
        GridVec::new(4, 2),
        &occupied,
    );

    assert_eq!(pos, GridVec::new(0, 5));
}

#[test]
fn free_position_ignores_tiles_outside_the_column() {
    let occupied = [rect(4, 2, 4, 4), rect(0, 8, 4, 2)];
    let pos = Grid::free_position_below(
        GridVec::ZERO,
        GridVec::new(4, 2),
        GridVec::new(4, 2),
        &occupied,
    );

    assert_eq!(pos, GridVec::new(0, 2));
}

#[test]
fn grid_vec_snaps_to_cells() {
    assert_eq!(GridVec::from_vec_floor(vec2(0.0, 0.0)), GridVec::new(0, 0));
    assert_eq!(
        GridVec::from_vec_floor(vec2(79.9, -0.1)),
        GridVec::new(1, -1)
    );
    assert_eq!(
        GridVec::from_vec_floor(vec2(-40.0, -41.0)),
        GridVec::new(-1, -2)
    );

    assert_eq!(
        GridVec::from_vec_rounded(vec2(20.0, 60.0)),
        GridVec::new(1, 2)
    );
    assert_eq!(
        GridVec::from_vec_rounded(vec2(-20.0, 19.0)),
        GridVec::new(-1, 0)
    );
}

#[test]
fn trim_partial_cell_discards_a_partial_span() {
    assert_eq!(Grid::trim_partial_cell(0.0), 0.0);
    assert_eq!(Grid::trim_partial_cell(39.9), 0.0);
    assert_eq!(Grid::trim_partial_cell(80.0), 80.0);
    assert_eq!(Grid::trim_partial_cell(-0.1), -40.0);
}

#[test]
fn wire_color_holds_the_output_color_until_the_last_quarter() {
    let output = Color32::from_rgb(10, 20, 30);
    let input = Color32::from_rgb(200, 100, 0);
    let src = pos2(0.0, 0.0);
    let dst = pos2(100.0, 0.0);

    assert_eq!(Grid::wire_color_at(src, src, dst, output, input), output);
    assert_eq!(
        Grid::wire_color_at(pos2(74.0, 0.0), src, dst, output, input),
        output
    );
    assert_eq!(
        Grid::wire_color_at(pos2(87.5, 0.0), src, dst, output, input),
        output.lerp_to_gamma(input, 0.5)
    );
    assert_eq!(Grid::wire_color_at(dst, src, dst, output, input), input);
    assert_eq!(
        Grid::wire_color_at(pos2(5.0, 5.0), src, src, output, input),
        output
    );
}

#[test]
fn wire_shape_is_straight_when_horizontal_and_curved_otherwise() {
    let stroke = PathStroke::new(2.0, Color32::WHITE);
    let horizontal = Grid::wire_shape(pos2(0.0, 10.0), pos2(40.0, 10.05), stroke.clone());
    let sloped = Grid::wire_shape(pos2(0.0, 0.0), pos2(10.0, 20.0), stroke);

    assert!(matches!(horizontal, egui::Shape::Path(_)));
    assert!(matches!(sloped, egui::Shape::CubicBezier(_)));
}

#[test]
fn cell_occupied_uses_the_widget_span() {
    let mut bridge = empty_bridge();
    let amp = bridge.add_module(ModuleType::Amplifier, GridVec::new(1, 2));
    let osc = bridge.add_module(ModuleType::Oscillator, GridVec::new(6, 0));
    connect_five_osc_inputs(&mut bridge, osc);
    let mut ios = take_ios(&mut bridge);
    assert_eq!(ios[&osc].inputs.len(), 5);

    let mut grid = Grid::new();
    mount(&mut grid, &mut ios, &[amp, osc]);

    assert!(grid.cell_occupied(&bridge, GridVec::new(1, 2)));
    assert!(grid.cell_occupied(&bridge, GridVec::new(2, 3)));
    assert!(!grid.cell_occupied(&bridge, GridVec::new(3, 2)));
    assert!(!grid.cell_occupied(&bridge, GridVec::new(1, 4)));
    assert!(grid.cell_occupied(&bridge, GridVec::new(9, 2)));
    assert!(!grid.cell_occupied(&bridge, GridVec::new(9, 3)));
    assert!(!grid.cell_occupied(&bridge, GridVec::new(10, 0)));
}

#[test]
fn content_size_is_the_bottom_right_of_the_widgets() {
    let mut bridge = empty_bridge();
    let amp = bridge.add_module(ModuleType::Amplifier, GridVec::new(1, 2));
    let mixer = bridge.add_module(ModuleType::Mixer, GridVec::ZERO);
    let mut ios = take_ios(&mut bridge);
    let mut grid = Grid::new();

    assert_eq!(grid.calc_content_size(&bridge), Vec2::ZERO);

    mount(&mut grid, &mut ios, &[amp, mixer]);

    // Amplifier occupies cells (1, 2) through (3, 4). Mixer ends at (3, 2).
    assert_eq!(grid.calc_content_size(&bridge), vec2(120.0, 160.0));
}

#[test]
fn resolve_overlaps_pushes_later_tiles_toward_bottom_right() {
    let mut bridge = empty_bridge();
    let first = bridge.add_module(ModuleType::Amplifier, GridVec::ZERO);
    let second = bridge.add_module(ModuleType::Amplifier, GridVec::new(1, 0));
    let third = bridge.add_module(ModuleType::Amplifier, GridVec::new(2, 0));
    let mut ios = take_ios(&mut bridge);
    let mut grid = Grid::new();
    mount(&mut grid, &mut ios, &[third, first, second]);

    grid.resolve_overlaps(None, &mut bridge);

    assert_eq!(bridge.get_module_position(first), GridVec::ZERO);
    assert_eq!(bridge.get_module_position(second), GridVec::new(2, 0));
    assert_eq!(bridge.get_module_position(third), GridVec::new(4, 0));
}

#[test]
fn resolve_overlaps_pushes_down_when_that_direction_is_shorter() {
    let mut bridge = empty_bridge();
    let osc = bridge.add_module(ModuleType::Oscillator, GridVec::ZERO);
    let amp = bridge.add_module(ModuleType::Amplifier, GridVec::new(1, 1));
    let mut ios = take_ios(&mut bridge);
    let mut grid = Grid::new();
    mount(&mut grid, &mut ios, &[osc, amp]);

    grid.resolve_overlaps(None, &mut bridge);

    assert_eq!(bridge.get_module_position(osc), GridVec::ZERO);
    assert_eq!(bridge.get_module_position(amp), GridVec::new(1, 2));
}

#[test]
fn resolve_overlaps_keeps_the_anchor_fixed() {
    let mut bridge = empty_bridge();
    let amp = bridge.add_module(ModuleType::Amplifier, GridVec::ZERO);
    let mixer = bridge.add_module(ModuleType::Mixer, GridVec::new(1, 0));
    let mut ios = take_ios(&mut bridge);
    let mut grid = Grid::new();
    mount(&mut grid, &mut ios, &[amp, mixer]);

    grid.resolve_overlaps(Some(mixer), &mut bridge);

    assert_eq!(bridge.get_module_position(mixer), GridVec::new(1, 0));
    assert_eq!(bridge.get_module_position(amp), GridVec::new(0, 2));
}

#[test]
fn resolve_overlaps_leaves_separated_tiles_alone() {
    let mut bridge = empty_bridge();
    let left = bridge.add_module(ModuleType::Amplifier, GridVec::ZERO);
    let right = bridge.add_module(ModuleType::Amplifier, GridVec::new(2, 0));
    let mut ios = take_ios(&mut bridge);
    let mut grid = Grid::new();
    mount(&mut grid, &mut ios, &[left, right]);

    grid.resolve_overlaps(None, &mut bridge);

    assert_eq!(bridge.get_module_position(left), GridVec::ZERO);
    assert_eq!(bridge.get_module_position(right), GridVec::new(2, 0));
}

#[test]
fn update_widgets_rebuilds_the_set_and_separates_overlaps() {
    let mut bridge = empty_bridge();
    let amp = bridge.add_module(ModuleType::Amplifier, GridVec::ZERO);
    let mixer = bridge.add_module(ModuleType::Mixer, GridVec::new(1, 0));
    let osc = bridge.add_module(ModuleType::Oscillator, GridVec::new(10, 0));
    let mut ios = take_ios(&mut bridge);
    let mut grid = Grid::new();
    mount(&mut grid, &mut ios, &[osc]);

    grid.update_widgets(picked_ios(&mut ios, &[amp, mixer]), &mut bridge);

    let ids: Vec<_> = grid.widgets.iter().map(GridWidget::module_id).collect();
    assert!(!ids.contains(&osc));
    assert!(ids.contains(&amp));
    assert!(ids.contains(&mixer));
    assert_eq!(bridge.get_module_position(amp), GridVec::ZERO);
    assert_eq!(bridge.get_module_position(mixer), GridVec::new(2, 0));
}

#[test]
fn update_widgets_anchors_the_added_module() {
    let mut bridge = empty_bridge();
    let amp = bridge.add_module(ModuleType::Amplifier, GridVec::ZERO);
    let mixer = bridge.add_module(ModuleType::Mixer, GridVec::new(1, 0));
    let mut ios = take_ios(&mut bridge);
    let mut grid = Grid::new();
    grid.added_module_id = Some(mixer);

    grid.update_widgets(picked_ios(&mut ios, &[amp, mixer]), &mut bridge);

    assert!(grid.added_module_id.is_none());
    assert_eq!(bridge.get_module_position(mixer), GridVec::new(1, 0));
    assert_eq!(bridge.get_module_position(amp), GridVec::new(0, 2));
}

#[test]
fn duplicate_event_places_a_copy_below_and_clears_events() {
    let mut bridge = empty_bridge();
    let osc = bridge.add_module(ModuleType::Oscillator, GridVec::new(1, 2));
    bridge.set_module_label(osc, "Lead".into());
    let mut ios = take_ios(&mut bridge);
    let mut grid = Grid::new();
    mount(&mut grid, &mut ios, &[osc]);
    let before = module_ids(&bridge);

    grid.events.push(GridEvent::Duplicate(osc));
    grid.events.push(GridEvent::Selected(osc));
    assert_eq!(grid.events().len(), 2);

    grid.process_events(&mut bridge);

    let added = added_ids(&before, &bridge);
    assert_eq!(added.len(), 1);
    assert_eq!(bridge.get_module_position(added[0]), GridVec::new(1, 4));
    assert_eq!(bridge.get_module_label(added[0]), "Lead");
    assert_eq!(bridge.get_module_position(osc), GridVec::new(1, 2));
    assert!(grid.events().is_empty());
}

#[test]
fn duplicate_event_skips_tiles_under_the_source() {
    let mut bridge = empty_bridge();
    let osc = bridge.add_module(ModuleType::Oscillator, GridVec::ZERO);
    let amp = bridge.add_module(ModuleType::Amplifier, GridVec::new(0, 2));
    let mixer = bridge.add_module(ModuleType::Mixer, GridVec::new(0, 4));
    let mut ios = take_ios(&mut bridge);
    let mut grid = Grid::new();
    mount(&mut grid, &mut ios, &[osc, amp, mixer]);
    let before = module_ids(&bridge);

    grid.events.push(GridEvent::Duplicate(osc));
    grid.process_events(&mut bridge);

    let added = added_ids(&before, &bridge);
    assert_eq!(added.len(), 1);
    assert_eq!(bridge.get_module_position(added[0]), GridVec::new(0, 6));
    assert_eq!(bridge.get_module_position(amp), GridVec::new(0, 2));
    assert_eq!(bridge.get_module_position(mixer), GridVec::new(0, 4));
}

#[test]
fn duplicate_event_uses_the_body_size_under_a_tall_source() {
    let mut bridge = empty_bridge();
    let osc = bridge.add_module(ModuleType::Oscillator, GridVec::ZERO);
    let amp = bridge.add_module(ModuleType::Amplifier, GridVec::new(0, 3));
    connect_five_osc_inputs(&mut bridge, osc);
    let mut ios = take_ios(&mut bridge);
    assert_eq!(ios[&osc].inputs.len(), 5);
    let mut grid = Grid::new();
    mount(&mut grid, &mut ios, &[osc, amp]);
    let before = module_ids(&bridge);

    grid.events.push(GridEvent::Duplicate(osc));
    grid.process_events(&mut bridge);

    let added = added_ids(&before, &bridge);
    assert_eq!(added.len(), 1);
    assert_eq!(bridge.get_module_position(added[0]), GridVec::new(0, 5));
    assert_eq!(bridge.get_module_position(osc), GridVec::ZERO);
    assert_eq!(bridge.get_module_position(amp), GridVec::new(0, 3));
}

#[test]
fn duplicate_event_ignores_a_missing_widget() {
    let mut bridge = empty_bridge();
    let osc = bridge.add_module(ModuleType::Oscillator, GridVec::new(1, 2));
    let mut grid = Grid::new();
    let before = module_ids(&bridge);

    grid.events.push(GridEvent::Duplicate(osc));
    grid.process_events(&mut bridge);

    assert_eq!(module_ids(&bridge), before);
    assert_eq!(bridge.get_module_position(osc), GridVec::new(1, 2));
    assert!(grid.events().is_empty());
}

#[test]
fn duplicate_event_does_not_copy_output() {
    let mut bridge = empty_bridge();
    let out = bridge.add_module(ModuleType::Output, GridVec::new(2, 2));
    assert_eq!(out, OUTPUT_MODULE_ID);
    let mut ios = take_ios(&mut bridge);
    let mut grid = Grid::new();
    mount(&mut grid, &mut ios, &[out]);
    let before = module_ids(&bridge);

    grid.events.push(GridEvent::Duplicate(out));
    grid.process_events(&mut bridge);

    assert_eq!(module_ids(&bridge), before);
    assert_eq!(bridge.get_module_position(out), GridVec::new(2, 2));
}

#[test]
fn moved_event_resolves_around_the_moved_module() {
    let mut bridge = empty_bridge();
    let amp = bridge.add_module(ModuleType::Amplifier, GridVec::ZERO);
    let mixer = bridge.add_module(ModuleType::Mixer, GridVec::new(1, 0));
    let mut ios = take_ios(&mut bridge);
    let mut grid = Grid::new();
    mount(&mut grid, &mut ios, &[amp, mixer]);

    grid.events.push(GridEvent::Moved(amp));
    grid.process_events(&mut bridge);

    assert_eq!(bridge.get_module_position(amp), GridVec::ZERO);
    assert_eq!(bridge.get_module_position(mixer), GridVec::new(2, 0));
    assert!(grid.events().is_empty());
}

#[test]
fn selected_event_leaves_positions_unchanged() {
    let mut bridge = empty_bridge();
    let osc = bridge.add_module(ModuleType::Oscillator, GridVec::new(3, 4));
    let mut grid = Grid::new();

    grid.events.push(GridEvent::Selected(osc));
    grid.process_events(&mut bridge);

    assert_eq!(bridge.get_module_position(osc), GridVec::new(3, 4));
    assert!(grid.events().is_empty());
}

#[test]
fn new_grid_has_no_events() {
    assert!(Grid::new().events().is_empty());
}
