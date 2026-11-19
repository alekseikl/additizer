use enum_dispatch::enum_dispatch;
use rustc_hash::{FxHashMap, FxHashSet};

use addi_engine::{
    InputId, MIN_MODULE_ID, ModuleId, ModuleType, ModuleUiLink, OUTPUT_MODULE_ID, PITCH_MODULE_ID,
    Smp, StereoSample,
    config::EngineParams,
    engine_io::EngineLinks,
    routing::{DataType, Input, InputMeta, InputSource, data_types_compatible},
    routing_state::{ConnectedInputSource, ModuleIo, RoutingState},
};

use crate::engine_factory::{EngineHandle, UiConfigHandle};

pub mod modules;
pub mod ui_config;

pub use ui_config::GridVec;

pub use addi_engine::engine_io::{EngineUiEnd, OutputMeter, UiEvent, UiUpdate, VoicesStatus};
use modules::{
    amplifier::AmplifierUiBridge, envelope::EnvelopeUiBridge, expressions::ExpressionsUiBridge,
    external_param::ExternalParamUiBridge, harmonic_editor::HarmonicEditorUiBridge,
    lfo::LfoUiBridge, mixer::MixerUiBridge, mod_map::ModMapUiBridge,
    multi_envelope::MultiEnvelopeUiBridge, oscillator::OscillatorUiBridge, pitch::PitchUiBridge,
    segments_envelope::SegmentsEnvelopeUiBridge, spectral_band_select::SpectralBandSelectUiBridge,
    spectral_blend::SpectralBlendUiBridge, spectral_eq::SpectralEqUiBridge,
    spectral_filter::SpectralFilterUiBridge, spectral_mixer::SpectralMixerUiBridge,
    spectral_noise::SpectralNoiseUiBridge, svf::SvfUiBridge, wave_shaper::WaveShaperUiBridge,
};
use ui_config::{UiConfig, UiModuleConfig};

pub fn module_default_label(module_type: ModuleType) -> &'static str {
    match module_type {
        ModuleType::Output => "Output",
        ModuleType::Amplifier => "Amplifier",
        ModuleType::Envelope => "Envelope",
        ModuleType::Mixer => "Mixer",
        ModuleType::Oscillator => "Oscillator",
        ModuleType::SpectralFilter => "Spectral Filter",
        ModuleType::SpectralEq => "Spectral EQ",
        ModuleType::SpectralBandSelect => "Band Select",
        ModuleType::SpectralBlend => "Spectral Blend",
        ModuleType::SpectralMixer => "Spectral Mixer",
        ModuleType::HarmonicEditor => "Harmonics",
        ModuleType::SpectralNoise => "Spectral Noise",
        ModuleType::ExternalParam => "Ext Param",
        ModuleType::Lfo => "LFO",
        ModuleType::ModMap => "Mod Map",
        ModuleType::MultiEnvelope => "Multi Envelope",
        ModuleType::SegmentsEnvelope => "Seg Envelope",
        ModuleType::Pitch => "Pitch",
        ModuleType::WaveShaper => "Waveshaper",
        ModuleType::Svf => "SVF",
        ModuleType::Expressions => "Expressions",
    }
}

#[enum_dispatch(ModuleUiBridge)]
pub enum ModuleBridge<E: EngineLinks = crate::links::PluginLinks> {
    Oscillator(Box<OscillatorUiBridge<E::Oscillator>>),
    Envelope(Box<EnvelopeUiBridge<E::Envelope>>),
    Amplifier(Box<AmplifierUiBridge<E::Amplifier>>),
    Lfo(Box<LfoUiBridge<E::Lfo>>),
    ModMap(Box<ModMapUiBridge<E::ModMap>>),
    MultiEnvelope(Box<MultiEnvelopeUiBridge<E::MultiEnvelope>>),
    SegmentsEnvelope(Box<SegmentsEnvelopeUiBridge<E::SegmentsEnvelope>>),
    Pitch(Box<PitchUiBridge<E::Pitch>>),
    Mixer(Box<MixerUiBridge<E::Mixer>>),
    WaveShaper(Box<WaveShaperUiBridge<E::WaveShaper>>),
    Svf(Box<SvfUiBridge<E::Svf>>),
    SpectralFilter(Box<SpectralFilterUiBridge<E::SpectralFilter>>),
    SpectralEq(Box<SpectralEqUiBridge<E::SpectralEq>>),
    SpectralBandSelect(Box<SpectralBandSelectUiBridge<E::SpectralBandSelect>>),
    SpectralBlend(Box<SpectralBlendUiBridge<E::SpectralBlend>>),
    SpectralMixer(Box<SpectralMixerUiBridge<E::SpectralMixer>>),
    HarmonicEditor(Box<HarmonicEditorUiBridge<E::HarmonicEditor>>),
    SpectralNoise(Box<SpectralNoiseUiBridge<E::SpectralNoise>>),
    Expressions(Box<ExpressionsUiBridge<E::Expressions>>),
    ExternalParam(Box<ExternalParamUiBridge<E::ExternalParam>>),
}

#[enum_dispatch]
#[auto_impl::auto_impl(Box)]
pub trait ModuleUiBridge: Send {
    // Return true when routing needs to be update
    fn update(&mut self) -> bool;
}

pub struct ModuleItem {
    pub id: ModuleId,
    pub module_type: ModuleType,
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
    engine: EngineHandle<E>,
    ui_config: UiConfigHandle,
    ui_end: E::UiEnd,
    routing: RoutingState,
    engine_params: EngineParams,
    block_size: usize,
    voices: VoicesStatus,
    modulated_inputs: FxHashMap<InputId, ModulatedInput>,
    module_bridges: FxHashMap<ModuleId, Option<ModuleBridge<E>>>,
    next_tag: i32,
    pending_modules: FxHashMap<i32, PendingModule>,
    placed_module: Option<ModuleId>,
    reach_stack: Vec<ModuleId>,
    reach_seen: FxHashSet<ModuleId>,
}

impl<E: EngineLinks> UiBridge<E> {
    pub fn create(engine: EngineHandle<E>, ui_config: UiConfigHandle) -> Option<Self> {
        let mut engine_lock = engine.lock();

        let mut ui_end = engine_lock.take_ui_end()?;
        let engine_params = engine_lock.get_engine_params();
        let block_size = engine_lock.block_size();

        drop(engine_lock);

        Self::ensure_fixed_pitch_module(&mut ui_config.lock());

        let mut bridges: FxHashMap<ModuleId, Option<ModuleBridge<E>>> = FxHashMap::default();
        let mut routing = ui_end.read_routing().clone();

        while let Some(update) = ui_end.pop_update() {
            if let UiUpdate::Topology { added, removed, .. } = update {
                routing = ui_end.read_routing().clone();
                Self::apply_topology_modules(&mut bridges, added, removed);
            }
        }

        Some(Self {
            engine,
            ui_config,
            ui_end,
            routing,
            engine_params,
            block_size,
            voices: VoicesStatus::default(),
            modulated_inputs: FxHashMap::default(),
            module_bridges: bridges,
            next_tag: 0,
            pending_modules: FxHashMap::default(),
            placed_module: None,
            reach_stack: Vec::new(),
            reach_seen: FxHashSet::default(),
        })
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

    fn apply_topology_modules(
        bridges: &mut FxHashMap<ModuleId, Option<ModuleBridge<E>>>,
        added: Option<ModuleUiLink<E>>,
        removed: Option<ModuleId>,
    ) {
        if let Some(id) = removed {
            bridges.remove(&id);
        }

        if let Some(link) = added {
            let id = link.id();

            bridges.insert(id, Some(module_bridge(link)));
        }
    }

    pub fn engine(&self) -> &EngineHandle<E> {
        &self.engine
    }

    pub fn engine_params(&self) -> &EngineParams {
        &self.engine_params
    }

    pub fn block_size(&self) -> usize {
        self.block_size
    }

    pub fn voices_status(&self) -> &VoicesStatus {
        &self.voices
    }

    pub fn display_module_label(&self, module_id: ModuleId) -> String {
        let Some(module_type) = self.routing.modules.get(&module_id).map(|m| m.module_type) else {
            return String::new();
        };

        let ui_config = self.ui_config.lock();

        ui_config
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
        let ui_config = self.ui_config.lock();
        ui_config
            .modules
            .get(&module_id)
            .map(|m| m.position)
            .unwrap_or_default()
    }

    pub fn set_module_position(&mut self, module_id: ModuleId, position: GridVec) {
        let mut ui_config = self.ui_config.lock();
        if let Some(module) = ui_config.modules.get_mut(&module_id) {
            module.position = position;
        }
    }

    pub fn get_module_label(&self, module_id: ModuleId) -> String {
        let ui_config = self.ui_config.lock();

        ui_config
            .modules
            .get(&module_id)
            .map(|module| module.label.clone())
            .unwrap_or_default()
    }

    pub fn set_module_label(&mut self, module_id: ModuleId, label: String) {
        let mut ui_config = self.ui_config.lock();
        let Some(module) = ui_config.modules.get_mut(&module_id) else {
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
                UiUpdate::Topology {
                    added,
                    removed,
                    tag,
                } => {
                    self.routing = self.ui_end.read_routing().clone();

                    if let Some(id) = removed {
                        self.module_bridges.remove(&id);
                        self.ui_config.lock().modules.remove(&id);
                        self.modulated_inputs
                            .retain(|input_id, _| input_id.module_id != id);
                    }

                    if let Some(link) = added {
                        let id = link.id();

                        self.module_bridges.insert(id, Some(module_bridge(link)));
                        self.apply_pending_module(tag, id);
                    }
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
        self.ui_config.lock().modules.insert(
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

fn module_bridge<E: EngineLinks>(link: ModuleUiLink<E>) -> ModuleBridge<E> {
    match link {
        ModuleUiLink::Oscillator { ui_end, config } => {
            ModuleBridge::Oscillator(Box::new(OscillatorUiBridge::new(ui_end, *config)))
        }
        ModuleUiLink::Envelope { ui_end, config } => {
            ModuleBridge::Envelope(Box::new(EnvelopeUiBridge::new(ui_end, *config)))
        }
        ModuleUiLink::Lfo { ui_end, config } => {
            ModuleBridge::Lfo(Box::new(LfoUiBridge::new(ui_end, *config)))
        }
        ModuleUiLink::ModMap { ui_end, config } => {
            ModuleBridge::ModMap(Box::new(ModMapUiBridge::new(ui_end, *config)))
        }
        ModuleUiLink::MultiEnvelope { ui_end, config } => {
            ModuleBridge::MultiEnvelope(Box::new(MultiEnvelopeUiBridge::new(ui_end, *config)))
        }
        ModuleUiLink::SegmentsEnvelope { ui_end, config } => {
            ModuleBridge::SegmentsEnvelope(Box::new(SegmentsEnvelopeUiBridge::new(ui_end, *config)))
        }
        ModuleUiLink::Pitch { ui_end, config } => {
            ModuleBridge::Pitch(Box::new(PitchUiBridge::new(ui_end, *config)))
        }
        ModuleUiLink::Amplifier { ui_end, config } => {
            ModuleBridge::Amplifier(Box::new(AmplifierUiBridge::new(ui_end, *config)))
        }
        ModuleUiLink::Mixer { ui_end, config } => {
            ModuleBridge::Mixer(Box::new(MixerUiBridge::new(ui_end, *config)))
        }
        ModuleUiLink::WaveShaper { ui_end, config } => {
            ModuleBridge::WaveShaper(Box::new(WaveShaperUiBridge::new(ui_end, *config)))
        }
        ModuleUiLink::Svf { ui_end, config } => {
            ModuleBridge::Svf(Box::new(SvfUiBridge::new(ui_end, *config)))
        }
        ModuleUiLink::SpectralFilter { ui_end, config } => {
            ModuleBridge::SpectralFilter(Box::new(SpectralFilterUiBridge::new(ui_end, *config)))
        }
        ModuleUiLink::SpectralEq { ui_end, config } => {
            ModuleBridge::SpectralEq(Box::new(SpectralEqUiBridge::new(ui_end, *config)))
        }
        ModuleUiLink::SpectralBandSelect { ui_end, config } => ModuleBridge::SpectralBandSelect(
            Box::new(SpectralBandSelectUiBridge::new(ui_end, *config)),
        ),
        ModuleUiLink::SpectralBlend { ui_end, config } => {
            ModuleBridge::SpectralBlend(Box::new(SpectralBlendUiBridge::new(ui_end, *config)))
        }
        ModuleUiLink::SpectralMixer { ui_end, config } => {
            ModuleBridge::SpectralMixer(Box::new(SpectralMixerUiBridge::new(ui_end, *config)))
        }
        ModuleUiLink::HarmonicEditor { ui_end, config } => {
            ModuleBridge::HarmonicEditor(Box::new(HarmonicEditorUiBridge::new(ui_end, *config)))
        }
        ModuleUiLink::SpectralNoise { ui_end, config } => {
            ModuleBridge::SpectralNoise(Box::new(SpectralNoiseUiBridge::new(ui_end, *config)))
        }
        ModuleUiLink::Expressions { ui_end, config } => {
            ModuleBridge::Expressions(Box::new(ExpressionsUiBridge::new(ui_end, *config)))
        }
        ModuleUiLink::ExternalParam { ui_end, config } => {
            ModuleBridge::ExternalParam(Box::new(ExternalParamUiBridge::new(ui_end, *config)))
        }
    }
}

#[cfg(test)]
mod tests;
