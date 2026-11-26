use std::collections::BTreeMap;
use std::sync::atomic::Ordering;

use atomic_float::AtomicF32;
use nice_plug_core::params::{ParamFlags, PluginState};
use parking_lot::Mutex;

use addi_engine::external_param::NUM_EXT_PARAMS;
use addi_engine::{EngineConfig, ModuleConfig, ModuleType, Smp, SynthEngine};

use crate::{
    default_scheme::build_default_preset,
    links::PluginLinks,
    preset::{Preset, PresetInfo},
    ui_bridge::{HostParamBinding, UiBridge, module_default_label},
};

/// The preset field key written by the previous parameter object. Kept so old presets still load.
const PRESET_FIELD: &str = "plugin-preset";

pub struct AddiState {
    /// Host sample rate. Zero until [`activate`][Self::activate]; a non-zero value means the
    /// plugin is activated.
    host_sample_rate: AtomicF32,
    /// Preset last published or last restored from the host. The open editor's bridge is newer;
    /// [`serialize_fields`][PluginState::serialize_fields] reads that first.
    ///
    /// In place before [`activate`][Self::activate]: the default patch at construction, or the
    /// host preset if state was restored first.
    preset: Mutex<Box<Preset>>,
    /// Newly published engine. The audio thread takes it when the mutex is free.
    engine: Mutex<Option<Box<SynthEngine<PluginLinks>>>>,
    /// Current UI bridge. The editor locks it while drawing. A replacement bridge reports
    /// [`UiBridge::is_new`] until the editor calls [`UiBridge::reset_new`].
    ui_bridge: Mutex<Option<Box<UiBridge>>>,
}

impl AddiState {
    pub fn new() -> Self {
        Self {
            host_sample_rate: AtomicF32::new(0.0),
            preset: Mutex::new(Box::new(build_default_preset())),
            engine: Mutex::new(None),
            ui_bridge: Mutex::new(None),
        }
    }

    /// Slot the editor locks while drawing.
    pub fn ui_bridge(&self) -> &Mutex<Option<Box<UiBridge>>> {
        &self.ui_bridge
    }

    /// Takes the published engine when the slot is free, leaving `None`.
    ///
    /// Returns `None` when the slot is empty or currently locked.
    pub fn try_take_engine(&self) -> Option<Box<SynthEngine<PluginLinks>>> {
        self.engine.try_lock()?.take()
    }

    /// Build the engine for `sample_rate` from the preset already stored here.
    ///
    /// The returned engine is for the audio thread. A replacement published later stays in
    /// [`try_take_engine`][Self::try_take_engine].
    pub fn activate(&self, sample_rate: Smp) -> Option<Box<SynthEngine<PluginLinks>>> {
        self.host_sample_rate.store(sample_rate, Ordering::Release);
        self.engine.lock().take();

        let (engine, bridge) = {
            let preset = self.preset.lock();
            let mut engine = self.build_engine(&preset.engine, preset.oversampling)?;
            let bridge = UiBridge::create(&mut engine, preset.ui.clone(), preset.oversampling)?;
            (engine, bridge)
        };
        self.ui_bridge.lock().replace(Box::new(bridge));
        Some(Box::new(engine))
    }

    /// Copy the open editor's preset into `preset`, then drop the engine and the UI bridge.
    ///
    /// The host reactivates later without sending the preset again. `host_sample_rate` goes back
    /// to zero.
    pub fn deactivate(&self) {
        let live = self.ui_bridge.lock().as_mut().map(|bridge| bridge.preset());
        if let Some(preset) = live {
            *self.preset.lock() = preset;
        }
        self.host_sample_rate.store(0.0, Ordering::Release);
        self.engine.lock().take();
        self.ui_bridge.lock().take();
    }

    pub fn load_preset(&self, preset: &Preset) -> bool {
        let Some(mut engine) = self.build_engine(&preset.engine, preset.oversampling) else {
            return false;
        };
        let Some(bridge) = UiBridge::create(&mut engine, preset.ui.clone(), preset.oversampling)
        else {
            return false;
        };

        **self.preset.lock() = Preset {
            info: PresetInfo::default(),
            engine: preset.engine.clone(),
            oversampling: preset.oversampling,
            ui: preset.ui.clone(),
        };
        *self.engine.lock() = Some(Box::new(engine));
        self.ui_bridge.lock().replace(Box::new(bridge));
        true
    }

    fn build_engine(
        &self,
        config: &EngineConfig,
        oversampling: bool,
    ) -> Option<SynthEngine<PluginLinks>> {
        SynthEngine::<PluginLinks>::try_new(
            config,
            self.host_sample_rate.load(Ordering::Acquire),
            oversampling,
        )
    }

    /// Hidden flag and name of every host parameter. The editor uses this to notice when the host
    /// should re-read parameter info.
    pub fn host_param_signature(&self) -> Vec<(bool, String)> {
        (0..NUM_EXT_PARAMS)
            .map(|index| match self.binding(index as u32) {
                Some(binding) => (false, binding.name),
                None => (true, format!("Param {}", index + 1)),
            })
            .collect()
    }

    /// Binding for `index`. The live bridge is current while the plugin is activated. Before
    /// activation and after deactivation the stored preset supplies the name and which index is
    /// bound. The preset does not store the automation value, so that stays `0.0`.
    fn binding(&self, index: u32) -> Option<HostParamBinding> {
        let index = index as usize;
        if index >= NUM_EXT_PARAMS {
            return None;
        }

        let mut bridge = self.ui_bridge.lock();
        if let Some(bridge) = bridge.as_mut() {
            return bridge.host_param_binding(index);
        }
        drop(bridge);

        let preset = self.preset.lock();
        binding_from_preset(&preset, index)
    }
}

/// Lowest-id external-parameter module in `preset` that is bound to `index`.
fn binding_from_preset(preset: &Preset, index: usize) -> Option<HostParamBinding> {
    let module = preset
        .engine
        .modules
        .iter()
        .filter_map(|module| match module {
            ModuleConfig::ExternalParam(config)
                if config.selected_param_index.min(NUM_EXT_PARAMS - 1) == index =>
            {
                Some(config.as_ref())
            }
            _ => None,
        })
        .min_by_key(|config| config.id)?;

    let name = preset
        .ui
        .modules
        .get(&module.id)
        .filter(|module| !module.label.is_empty())
        .map(|module| module.label.clone())
        .unwrap_or_else(|| module_default_label(ModuleType::ExternalParam).to_string());

    Some(HostParamBinding { name, value: 0.0 })
}

impl PluginState for AddiState {
    fn param_count(&self) -> u32 {
        NUM_EXT_PARAMS as u32
    }

    fn param_name(&self, index: u32) -> String {
        self.binding(index)
            .map(|binding| binding.name)
            .unwrap_or_else(|| format!("Param {}", index + 1))
    }

    fn param_flags(&self, index: u32) -> ParamFlags {
        if self.binding(index).is_some() {
            ParamFlags::empty()
        } else {
            ParamFlags::HIDDEN | ParamFlags::NON_AUTOMATABLE
        }
    }

    fn param_normalized_value(&self, index: u32) -> f32 {
        self.binding(index).map(|binding| binding.value).unwrap_or(0.0)
    }

    fn serialize_fields(&self) -> BTreeMap<String, String> {
        let mut serialized = BTreeMap::new();
        let mut bridge = self.ui_bridge.lock();
        let preset_json = if let Some(bridge) = bridge.as_mut() {
            serde_json::to_string(&bridge.preset())
        } else {
            drop(bridge);
            serde_json::to_string(self.preset.lock().as_ref())
        };
        match preset_json {
            Ok(data) => {
                serialized.insert(String::from(PRESET_FIELD), data);
            }
            Err(err) => {
                tracing::error!("Could not serialize '{PRESET_FIELD}': {err}");
            }
        }
        serialized
    }

    fn deserialize_fields(&self, serialized: &BTreeMap<String, String>) {
        let Some(data) = serialized.get(PRESET_FIELD) else {
            return;
        };
        let preset = match serde_json::from_str::<Preset>(data) {
            Ok(preset) => preset,
            Err(err) => {
                tracing::error!("Could not deserialize '{PRESET_FIELD}': {err}");
                return;
            }
        };

        if self.host_sample_rate.load(Ordering::Acquire) != 0.0 {
            self.load_preset(&preset);
        } else {
            **self.preset.lock() = preset;
        }
    }
}

#[cfg(test)]
mod tests {
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
        assert_eq!(state.host_param_signature()[2].0, false);
    }
}
