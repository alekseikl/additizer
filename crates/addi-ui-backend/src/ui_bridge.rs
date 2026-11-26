use rustc_hash::{FxHashMap, FxHashSet};

use addi_engine::{
    EngineConfig, InputId, LinkConfig, MIN_MODULE_ID, ModuleId, ModuleType, OUTPUT_MODULE_ID,
    PITCH_MODULE_ID, Smp, StereoSample, SynthEngine,
    config::EngineParams,
    engine_io::EngineLinks,
    routing::{DataType, Input, InputMeta, InputSource, data_types_compatible},
    routing_state::{ConnectedInputSource, ModuleIo, RoutingState},
};

use crate::preset::{Preset, PresetInfo};

pub mod modules;
pub mod ui_config;

pub use modules::{ModuleBridge, ModuleUiBridge, module_default_label};
pub use ui_config::GridVec;

pub use addi_engine::engine_io::{EngineUiEnd, OutputMeter, UiEvent, UiUpdate, VoicesStatus};
use ui_config::{UiConfig, UiModuleConfig};

pub struct ModuleItem {
    pub id: ModuleId,
    pub module_type: ModuleType,
}

/// Host-facing description of an external parameter that is bound to a module.
pub struct HostParamBinding {
    pub name: String,
    pub value: Smp,
}

#[derive(Clone, Copy)]
pub struct ModulatedValue {
    pub value: StereoSample,
    pub normalized: StereoSample,
    pub is_stereo: bool,
}

pub struct LinkableInput {
    pub input_type: Input,
    pub is_direct: bool,
}

#[derive(Default, Clone, Copy)]
struct ModulatedInput {
    value: StereoSample,
    normalized: StereoSample,
}

struct PendingModule {
    label: String,
    position: GridVec,
    anchor: bool,
}

pub struct UiBridge<E: EngineLinks = crate::links::PluginLinks> {
    ui_config: UiConfig,
    ui_end: E::UiEnd,
    routing: RoutingState,
    engine_params: EngineParams,
    oversampling: bool,
    block_size: usize,
    voices: VoicesStatus,
    modulated_inputs: FxHashMap<InputId, ModulatedInput>,
    module_bridges: FxHashMap<ModuleId, Option<ModuleBridge<E>>>,
    next_tag: i32,
    pending_modules: FxHashMap<i32, PendingModule>,
    placed_module: Option<ModuleId>,
    reach_stack: Vec<ModuleId>,
    reach_seen: FxHashSet<ModuleId>,
    /// The editor has not yet rebuilt its view around this bridge.
    is_new: bool,
}

impl<E: EngineLinks> UiBridge<E> {
    pub fn create(
        engine: &mut SynthEngine<E>,
        mut ui_config: UiConfig,
        oversampling: bool,
    ) -> Option<Self> {
        let ui_end = engine.take_ui_end()?;
        let engine_params = engine.get_engine_params();
        let block_size = engine.block_size();
        let routing = engine.get_routing_state();
        let module_links = engine.take_module_ui_links();

        Self::ensure_fixed_pitch_module(&mut ui_config);

        let mut module_bridges: FxHashMap<ModuleId, Option<ModuleBridge<E>>> = FxHashMap::default();

        for link in module_links {
            let bridge = ModuleBridge::new(link);
            let id = bridge.id();

            module_bridges.insert(id, Some(bridge));
        }

        Some(Self {
            ui_config,
            ui_end,
            routing,
            engine_params,
            oversampling,
            block_size,
            voices: VoicesStatus::default(),
            modulated_inputs: FxHashMap::default(),
            module_bridges,
            next_tag: 0,
            pending_modules: FxHashMap::default(),
            placed_module: None,
            reach_stack: Vec::new(),
            reach_seen: FxHashSet::default(),
            is_new: true,
        })
    }

    /// The editor has not yet rebuilt its view around this bridge.
    pub fn is_new(&self) -> bool {
        self.is_new
    }

    /// Mark this bridge as seen by the editor.
    pub fn reset_new(&mut self) {
        self.is_new = false;
    }

    fn ensure_fixed_pitch_module(ui_config: &mut UiConfig) {
        if ui_config.modules.contains_key(&PITCH_MODULE_ID) {
            return;
        }

        ui_config.modules.insert(
            PITCH_MODULE_ID,
            UiModuleConfig {
                id: PITCH_MODULE_ID,
                label: String::new(),
                position: GridVec::new(0, 0),
            },
        );
    }

    pub fn engine_params(&self) -> &EngineParams {
        &self.engine_params
    }

    pub fn oversampling(&self) -> bool {
        self.oversampling
    }

    /// Preset that rebuilds the engine at `oversampling`.
    ///
    /// `None` when oversampling already matches. The editor stores it and applies it after
    /// unlocking this bridge.
    pub fn set_oversampling(&mut self, oversampling: bool) -> Option<Box<Preset>> {
        if self.oversampling == oversampling {
            return None;
        }

        let mut preset = self.preset();
        preset.oversampling = oversampling;
        Some(preset)
    }

    /// Engine config reconstructed from this bridge's module bridges, links, and engine params.
    pub fn engine_config(&mut self) -> EngineConfig {
        let mut module_ids: Vec<ModuleId> = self.module_bridges.keys().copied().collect();
        module_ids.sort_unstable();

        let mut modules = Vec::with_capacity(module_ids.len());
        for id in module_ids {
            if let Some(bridge) = self.module_bridges.get_mut(&id).and_then(Option::as_mut) {
                modules.push(bridge.module_config());
            }
        }

        EngineConfig {
            engine: self.engine_params.clone(),
            modules,
            links: self.link_configs(),
        }
    }

    pub fn preset(&mut self) -> Box<Preset> {
        let engine = self.engine_config();

        Box::new(Preset {
            info: PresetInfo::default(),
            engine,
            oversampling: self.oversampling,
            ui: self.ui_config.clone(),
        })
    }

    fn link_configs(&self) -> Vec<LinkConfig> {
        let mut links = Vec::new();

        for (dst, sources) in &self.routing.routing {
            match sources {
                InputSource::Direct { hidden: true, .. } => {}
                InputSource::Direct {
                    module_id,
                    hidden: false,
                } => links.push(LinkConfig::direct(
                    *module_id,
                    dst.module_id,
                    dst.input_type,
                )),
                InputSource::Mixed(sources) => {
                    for src in sources {
                        links.push(LinkConfig::mixed(
                            src.module_id,
                            dst.module_id,
                            dst.input_type,
                            src.amount,
                        ));
                    }
                }
            }
        }

        links
    }

    pub fn block_size(&self) -> usize {
        self.block_size
    }

    pub fn voices_status(&self) -> &VoicesStatus {
        &self.voices
    }

    /// External parameter module bound to `index`, preferring the lowest module id.
    pub fn host_param_binding(&mut self, index: usize) -> Option<HostParamBinding> {
        let mut ids: Vec<ModuleId> = self.module_bridges.keys().copied().collect();
        ids.sort_unstable();

        let module_id = ids.into_iter().find(|&id| {
            matches!(
                self.module_bridges.get(&id).and_then(Option::as_ref),
                Some(ModuleBridge::ExternalParam(module)) if module.selected_index() == index
            )
        })?;

        let name = self.display_module_label(module_id);
        let value = match self.module_bridges.get_mut(&module_id).and_then(Option::as_mut) {
            Some(ModuleBridge::ExternalParam(module)) => module.automation_value(),
            _ => return None,
        };

        Some(HostParamBinding { name, value })
    }

    pub fn display_module_label(&self, module_id: ModuleId) -> String {
        let Some(module_type) = self.routing.modules.get(&module_id).map(|m| m.module_type) else {
            return String::new();
        };

        self.ui_config
            .modules
            .get(&module_id)
            .map(|module| {
                if module.label.is_empty() {
                    module_default_label(module_type).into()
                } else {
                    module.label.clone()
                }
            })
            .unwrap_or_else(|| module_default_label(module_type).into())
    }

    pub fn get_modules(&self) -> Vec<ModuleItem> {
        self.routing
            .modules
            .values()
            .map(|m| ModuleItem {
                id: m.id,
                module_type: m.module_type,
            })
            .collect()
    }

    pub fn has_module_id(&self, module_id: ModuleId) -> bool {
        self.routing.modules.contains_key(&module_id)
    }

    pub fn with_module_bridge(
        &mut self,
        module_id: ModuleId,
        f: impl FnOnce(&mut Self, &mut ModuleBridge<E>),
    ) {
        let bridge = self
            .module_bridges
            .get_mut(&module_id)
            .and_then(Option::take);

        if let Some(mut bridge) = bridge {
            f(self, &mut bridge);

            if let Some(slot) = self.module_bridges.get_mut(&module_id) {
                *slot = Some(bridge);
            }
        }
    }

    pub fn take_modules_io(&mut self) -> Option<FxHashMap<ModuleId, ModuleIo>> {
        self.routing.modules_io.take()
    }

    pub fn get_module_position(&self, module_id: ModuleId) -> GridVec {
        self.ui_config
            .modules
            .get(&module_id)
            .map(|m| m.position)
            .unwrap_or_default()
    }

    pub fn set_module_position(&mut self, module_id: ModuleId, position: GridVec) {
        if let Some(module) = self.ui_config.modules.get_mut(&module_id) {
            module.position = position;
        }
    }

    pub fn get_module_label(&self, module_id: ModuleId) -> String {
        self.ui_config
            .modules
            .get(&module_id)
            .map(|module| module.label.clone())
            .unwrap_or_default()
    }

    pub fn set_module_label(&mut self, module_id: ModuleId, label: String) {
        let Some(module) = self.ui_config.modules.get_mut(&module_id) else {
            debug_assert!(false, "Module with id {module_id} not found in ui_config");
            return;
        };

        module.label = label;
    }

    pub fn has_active_voices(&self) -> bool {
        self.voices.playing + self.voices.releasing > 0
    }

    pub fn get_linkable_inputs(&mut self, src: ModuleId, dst: ModuleId) -> Vec<LinkableInput> {
        let Some(src_output_type) = self
            .routing
            .modules
            .get(&src)
            .map(|module| module.output_type)
        else {
            return Vec::new();
        };
        let Some(inputs) = self
            .routing
            .modules
            .get(&dst)
            .map(|module| module.inputs.clone())
        else {
            return Vec::new();
        };

        inputs
            .into_iter()
            .filter(|meta| self.is_linkable_input(src, dst, src_output_type, meta))
            .map(|meta| LinkableInput {
                input_type: meta.input_type,
                is_direct: meta.is_direct,
            })
            .collect()
    }

    /// Whether `src` can connect to any input on `dst`.
    pub fn has_linkable_input(&mut self, src: ModuleId, dst: ModuleId) -> bool {
        if src == dst {
            return false;
        }

        let Some(src_output_type) = self
            .routing
            .modules
            .get(&src)
            .map(|module| module.output_type)
        else {
            return false;
        };
        let Some(inputs) = self
            .routing
            .modules
            .get(&dst)
            .map(|module| module.inputs.clone())
        else {
            return false;
        };

        inputs
            .iter()
            .any(|meta| self.is_linkable_input(src, dst, src_output_type, meta))
    }

    pub fn create_link(&mut self, src: ModuleId, dst: InputId) {
        let Some(module) = self.routing.modules.get(&dst.module_id) else {
            return;
        };
        let Some(meta) = module
            .inputs
            .iter()
            .find(|meta| meta.input_type == dst.input_type)
        else {
            return;
        };
        let meta = *meta;

        if meta.is_direct {
            self.set_direct_link(src, dst);
        } else {
            self.add_link(src, dst, StereoSample::ZERO);
        }
    }

    pub fn has_connected_input_sources(&self, input: InputId) -> bool {
        self.routing.routing.contains_key(&input)
    }

    pub fn get_connected_input_sources(&self, input: InputId) -> Vec<ConnectedInputSource> {
        let Some(sources) = self.routing.routing.get(&input) else {
            return Vec::new();
        };

        match sources {
            InputSource::Direct { module_id, .. } => {
                if !self.routing.modules.contains_key(module_id) {
                    return Vec::new();
                }

                vec![ConnectedInputSource {
                    src: *module_id,
                    amount: StereoSample::ONE,
                    label: self.display_module_label(*module_id),
                }]
            }
            InputSource::Mixed(mixed) => mixed
                .iter()
                .filter(|source| self.routing.modules.contains_key(&source.module_id))
                .map(|source| ConnectedInputSource {
                    src: source.module_id,
                    amount: source.amount,
                    label: self.display_module_label(source.module_id),
                })
                .collect(),
        }
    }

    pub fn get_input_modulated_value(&self, input: InputId) -> Option<ModulatedValue> {
        if self.routing.routing.contains_key(&input)
            && self.has_active_voices()
            && let Some(modulated) = self.modulated_inputs.get(&input).copied()
        {
            Some(ModulatedValue {
                value: modulated.value,
                normalized: modulated.normalized,
                is_stereo: true,
            })
        } else {
            None
        }
    }

    pub fn apply_modulation(&self, module_id: ModuleId, input: Input, param: &mut StereoSample) {
        if let Some(modulated) = self.get_input_modulated_value(InputId::new(input, module_id)) {
            *param = if modulated.is_stereo {
                modulated.value
            } else {
                StereoSample::splat(modulated.value.left())
            };
        }
    }

    /// Whether `target` is reachable from `from` through routed sources.
    /// Reuses `reach_stack` and `reach_seen` across calls.
    fn routing_reaches(&mut self, from: ModuleId, target: ModuleId) -> bool {
        self.reach_stack.clear();
        self.reach_seen.clear();
        self.reach_stack.push(from);

        while let Some(id) = self.reach_stack.pop() {
            if id == target {
                return true;
            }

            if !self.reach_seen.insert(id) {
                continue;
            }

            for (input, sources) in &self.routing.routing {
                if sources.source_ids().any(|source| source == id) {
                    self.reach_stack.push(input.module_id);
                }
            }
        }

        false
    }

    // Whether `src` may target this destination input
    fn is_linkable_input(
        &mut self,
        src: ModuleId,
        dst: ModuleId,
        src_output_type: DataType,
        meta: &InputMeta,
    ) -> bool {
        let input_id = InputId::new(meta.input_type, dst);

        src != dst
            && data_types_compatible(src_output_type, meta.data_type)
            && !self.routing_reaches(dst, src)
            && !self
                .routing
                .routing
                .get(&input_id)
                .is_some_and(|sources| sources.contains_module(src))
    }

    pub fn update(&mut self) {
        while let Some(update) = self.ui_end.pop_update() {
            match update {
                UiUpdate::ModulatedInput {
                    module_id,
                    input,
                    channel,
                    value,
                    normalized_value,
                } => {
                    let channel_idx = channel as usize;
                    let modulated = self
                        .modulated_inputs
                        .entry(InputId::new(input, module_id))
                        .or_default();

                    modulated.value[channel_idx] = value;
                    modulated.normalized[channel_idx] = normalized_value;
                }
                UiUpdate::VoicesStatus(status) => self.voices = status,
                UiUpdate::Routing => self.routing = self.ui_end.read_routing().clone(),
                UiUpdate::ModuleAdded { module, tag } => {
                    self.routing = self.ui_end.read_routing().clone();
                    let bridge = ModuleBridge::new(module);
                    let id = bridge.id();

                    self.module_bridges.insert(id, Some(bridge));
                    self.apply_pending_module(tag, id);
                }
                UiUpdate::ModuleRemoved(id) => {
                    self.routing = self.ui_end.read_routing().clone();
                    self.module_bridges.remove(&id);
                    self.ui_config.modules.remove(&id);
                    self.modulated_inputs
                        .retain(|input_id, _| input_id.module_id != id);
                }
            }
        }

        let mut routing_refresh = false;

        for module in self.module_bridges.values_mut().filter_map(|m| m.as_mut()) {
            routing_refresh |= module.update();
        }

        if routing_refresh {
            let _ = self.ui_end.send_event(UiEvent::RefreshRouting);
        }
    }

    pub fn add_module(&mut self, module_type: ModuleType, pos: GridVec) {
        if matches!(module_type, ModuleType::Output) {
            self.insert_ui_module(OUTPUT_MODULE_ID, String::new(), pos);
            self.placed_module = Some(OUTPUT_MODULE_ID);
            return;
        }

        let tag = self.alloc_tag();

        if self
            .ui_end
            .send_event(UiEvent::AddModule { tag, module_type })
        {
            self.pending_modules.insert(
                tag,
                PendingModule {
                    label: String::new(),
                    position: pos,
                    anchor: true,
                },
            );
        }
    }

    pub fn duplicate_module(&mut self, module_id: ModuleId, position: GridVec) {
        if module_id < MIN_MODULE_ID || !self.routing.modules.contains_key(&module_id) {
            return;
        }

        let tag = self.alloc_tag();

        if !self.ui_end.send_event(UiEvent::DuplicateModule {
            src: module_id,
            tag,
        }) {
            return;
        }

        let label = self.get_module_label(module_id);

        self.pending_modules.insert(
            tag,
            PendingModule {
                label,
                position,
                anchor: false,
            },
        );
    }

    /// Module id placed by the latest add, once the audio thread has allocated it.
    pub fn take_placed_module(&mut self) -> Option<ModuleId> {
        self.placed_module.take()
    }

    fn alloc_tag(&mut self) -> i32 {
        let tag = self.next_tag;

        self.next_tag += 1;
        tag
    }

    fn apply_pending_module(&mut self, tag: Option<i32>, id: ModuleId) {
        let Some(tag) = tag else {
            return;
        };
        let Some(pending) = self.pending_modules.remove(&tag) else {
            return;
        };

        if pending.anchor {
            self.placed_module = Some(id);
        }

        self.insert_ui_module(id, pending.label, pending.position);
    }

    pub fn remove_module(&mut self, module_id: ModuleId) {
        if module_id < MIN_MODULE_ID {
            return;
        }

        let _ = self.ui_end.send_event(UiEvent::RemoveModule(module_id));
    }

    pub fn set_direct_link(&mut self, src: ModuleId, dst: InputId) {
        let _ = self.ui_end.send_event(UiEvent::SetDirectLink { src, dst });
    }

    pub fn add_link(&mut self, src: ModuleId, dst: InputId, amount: StereoSample) {
        let _ = self
            .ui_end
            .send_event(UiEvent::AddMixedLink { src, dst, amount });
    }

    pub fn remove_link(&mut self, src: ModuleId, dst: InputId) {
        let _ = self.ui_end.send_event(UiEvent::RemoveLink { src, dst });
    }

    pub fn remove_input_links(&mut self, dst: InputId) {
        let _ = self.ui_end.send_event(UiEvent::RemoveInputLinks(dst));
    }

    pub fn remove_output_links(&mut self, src: ModuleId) {
        let _ = self.ui_end.send_event(UiEvent::RemoveOutputLinks(src));
    }

    fn insert_ui_module(&mut self, id: ModuleId, label: String, position: GridVec) {
        self.ui_config.modules.insert(
            id,
            UiModuleConfig {
                id,
                label,
                position,
            },
        );
    }

    pub fn set_link_amount(&mut self, src: ModuleId, dst: InputId, amount: StereoSample) {
        if self.ui_end.set_link_amount(src, dst, amount)
            && let Some(sources) = self.routing.routing.get_mut(&dst)
        {
            sources.update_amount(src, amount);
        }
    }

    pub fn set_voices(&mut self, voices: usize) {
        if self.ui_end.set_voices(voices) {
            self.engine_params.num_voices = voices;
        }
    }

    pub fn set_legato(&mut self, legato: bool) {
        if self.ui_end.set_legato(legato) {
            self.engine_params.legato = legato;
        }
    }

    pub fn set_kill_same_note(&mut self, kill_same_note: bool) {
        if self.ui_end.set_kill_same_note(kill_same_note) {
            self.engine_params.kill_same_note = kill_same_note;
        }
    }

    pub fn set_voice_kill_time(&mut self, voice_kill_time: Smp) {
        if self.ui_end.set_voice_kill_time(voice_kill_time) {
            self.engine_params.voice_kill_time = voice_kill_time;
        }
    }

    pub fn set_output_gain(&mut self, output_gain: StereoSample) {
        if self.ui_end.set_output_gain(output_gain) {
            self.engine_params.output_gain = output_gain;
        }
    }

    pub fn get_out_volume(&mut self) -> OutputMeter {
        self.ui_end.get_out_volume()
    }
}

#[cfg(test)]
mod tests;
