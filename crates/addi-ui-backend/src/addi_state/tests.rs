use std::collections::BTreeMap;

use addi_engine::external_param::ExternalParamConfig;
use nice_plug_core::params::ParamFlags;
use rustc_hash::FxHashMap;

use crate::ui_bridge::ui_config::{UiConfig, UiModuleConfig};

use super::*;

fn store_preset(state: &AddiState, preset: &Preset) {
    let mut fields = BTreeMap::new();
    fields.insert(
        PRESET_FIELD.to_string(),
        serde_json::to_string(preset).expect("preset serializes"),
    );
    state.deserialize_fields(&fields);
}

fn preset_with_params(modules: Vec<(i32, usize, &str)>) -> Preset {
    let mut ui_modules = FxHashMap::default();
    let engine_modules = modules
        .into_iter()
        .map(|(id, index, label)| {
            ui_modules.insert(
                id,
                UiModuleConfig {
                    id,
                    label: label.to_string(),
                    position: Default::default(),
                },
            );
            ModuleConfig::ExternalParam(Box::new(ExternalParamConfig {
                id,
                selected_param_index: index,
                sample_on_trigger: false,
            }))
        })
        .collect();

    Preset {
        info: PresetInfo::default(),
        engine: EngineConfig {
            modules: engine_modules,
            ..EngineConfig::default()
        },
        oversampling: true,
        ui: UiConfig {
            modules: ui_modules,
        },
    }
}

#[test]
fn inactive_state_reads_host_params_from_the_preset() {
    let state = AddiState::new();
    store_preset(
        &state,
        &preset_with_params(vec![(3, 0, "Later"), (1, 0, "Cutoff"), (2, 2, "")]),
    );

    assert_eq!(state.param_name(0), "Cutoff");
    assert_eq!(state.param_flags(0), ParamFlags::empty());
    assert_eq!(state.param_normalized_value(0), 0.0);
    assert_eq!(state.param_name(2), "Ext Param");
    assert_eq!(
        state.param_flags(1),
        ParamFlags::HIDDEN | ParamFlags::NON_AUTOMATABLE
    );
    assert_eq!(state.param_name(1), "Param 2");

    let signature = state.host_param_signature();
    assert_eq!(signature[0], (false, "Cutoff".to_string()));
    assert_eq!(signature[1], (true, "Param 2".to_string()));
    assert_eq!(signature[2], (false, "Ext Param".to_string()));
}

#[test]
fn host_params_follow_the_preset_across_activation() {
    let state = AddiState::new();
    store_preset(&state, &preset_with_params(vec![(1, 2, "Cutoff")]));

    assert_eq!(state.param_name(2), "Cutoff");
    assert!(state.activate(48_000.0).is_some());
    assert_eq!(state.param_name(2), "Cutoff");
    assert_eq!(state.param_flags(2), ParamFlags::empty());

    state.deactivate();
    assert_eq!(state.param_name(2), "Cutoff");
    assert!(!state.host_param_signature()[2].0);
}
