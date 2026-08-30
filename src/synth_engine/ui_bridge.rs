use std::ops::DerefMut;

use enum_dispatch::enum_dispatch;

use crate::{
    engine_factory::{EngineHandle, UiConfigHandle},
    synth_engine::{
        InputId, ModuleHandle, ModuleId, ModuleType, ModuleUiBridge, OUTPUT_MODULE_ID, Sample,
        StereoSample,
        amplifier::AmplifierUiBridge,
        config::EngineParams,
        envelope::EnvelopeUiBridge,
        expressions::ExpressionsUiBridge,
        external_param::ExternalParamUiBridge,
        harmonic_editor::HarmonicEditorUiBridge,
        lfo::LfoUiBridge,
        mixer::MixerUiBridge,
        oscillator::OscillatorUiBridge,
        routing::{DataType, Input, InputMeta, InputSource, data_types_compatible},
        spectral_blend::SpectralBlendUiBridge,
        spectral_filter::SpectralFilterUiBridge,
        spectral_mixer::SpectralMixerUiBridge,
        ui_bridge::{routing_state::ModuleIo, ui_config::UiModuleConfig},
        wave_shaper::WaveShaperUiBridge,
    },
    utils::log,
};

mod link;
pub mod routing_state;
pub mod ui_config;

pub use ui_config::GridVec;

pub use link::{AudioEnd, UiEnd, UiEvent, UiUpdate, create_link_pair};
pub use routing_state::{ConnectedInputSource, RoutingState};
use rustc_hash::FxHashMap;

#[enum_dispatch(ModuleUiBridge)]
pub enum ModuleBridge {
    Oscillator(Box<OscillatorUiBridge>),
    Envelope(Box<EnvelopeUiBridge>),
    Amplifier(Box<AmplifierUiBridge>),
    Lfo(Box<LfoUiBridge>),
    Mixer(Box<MixerUiBridge>),
    WaveShaper(Box<WaveShaperUiBridge>),
    SpectralFilter(Box<SpectralFilterUiBridge>),
    SpectralBlend(Box<SpectralBlendUiBridge>),
    SpectralMixer(Box<SpectralMixerUiBridge>),
    HarmonicEditor(Box<HarmonicEditorUiBridge>),
    Expressions(Box<ExpressionsUiBridge>),
    ExternalParam(Box<ExternalParamUiBridge>),
}

#[derive(Clone, Copy, Default)]
pub struct VoicesStatus {
    pub waiting_notes: u8,
    pub playing: u8,
    pub releasing: u8,
    pub killing: u8,
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

pub struct LinkableModulation {
    pub module_id: ModuleId,
    pub label: String,
}

pub struct LinkableInput {
    pub input_type: Input,
    pub is_direct: bool,
    pub modulations: Vec<LinkableModulation>,
}

#[derive(Default, Clone, Copy)]
struct ModulatedInput {
    value: StereoSample,
    normalized: StereoSample,
}

pub struct UiBridge {
    engine: EngineHandle,
    ui_config: UiConfigHandle,
    ui_end: UiEnd,
    routing: RoutingState,
    engine_params: EngineParams,
    voices: VoicesStatus,
    modulated_inputs: FxHashMap<InputId, ModulatedInput>,
    module_bridges: FxHashMap<ModuleId, Option<ModuleBridge>>,
}

impl UiBridge {
    pub fn create(engine: EngineHandle, ui_config: UiConfigHandle) -> Option<Self> {
        let mut engine_lock = engine.lock();

        let ui_end = engine_lock.ui_end.take()?;
        let routing = engine_lock.get_routing_state();
        let engine_params = engine_lock.get_engine_params();

        drop(engine_lock);

        let mut bridges: FxHashMap<ModuleId, Option<ModuleBridge>> = FxHashMap::default();

        for m in routing.modules.values() {
            Self::insert_module_bridge(m.id, &engine, &mut bridges)?;
        }

        Some(Self {
            engine,
            ui_config,
            ui_end,
            routing,
            engine_params,
            voices: VoicesStatus::default(),
            modulated_inputs: FxHashMap::default(),
            module_bridges: bridges,
        })
    }

    fn insert_module_bridge(
        id: ModuleId,
        engine: &EngineHandle,
        bridges: &mut FxHashMap<ModuleId, Option<ModuleBridge>>,
    ) -> Option<()> {
        let mut engine_lock = engine.lock();
        let engine_ref = engine_lock.deref_mut();

        let bridge = match engine_ref.get_module_mut(id)? {
            ModuleHandle::Oscillator(m) => ModuleBridge::Oscillator(Box::new(
                OscillatorUiBridge::try_new(id, engine.clone(), m)?,
            )),
            ModuleHandle::Envelope(m) => {
                ModuleBridge::Envelope(Box::new(EnvelopeUiBridge::try_new(m)?))
            }
            ModuleHandle::Lfo(m) => ModuleBridge::Lfo(Box::new(LfoUiBridge::try_new(m)?)),
            ModuleHandle::Amplifier(m) => {
                ModuleBridge::Amplifier(Box::new(AmplifierUiBridge::try_new(m)?))
            }
            ModuleHandle::Mixer(m) => ModuleBridge::Mixer(Box::new(MixerUiBridge::try_new(m)?)),
            ModuleHandle::WaveShaper(m) => {
                ModuleBridge::WaveShaper(Box::new(WaveShaperUiBridge::try_new(m)?))
            }
            ModuleHandle::SpectralFilter(m) => {
                ModuleBridge::SpectralFilter(Box::new(SpectralFilterUiBridge::try_new(m)?))
            }
            ModuleHandle::SpectralBlend(m) => {
                ModuleBridge::SpectralBlend(Box::new(SpectralBlendUiBridge::try_new(m)?))
            }
            ModuleHandle::SpectralMixer(m) => {
                ModuleBridge::SpectralMixer(Box::new(SpectralMixerUiBridge::try_new(m)?))
            }
            ModuleHandle::HarmonicEditor(m) => {
                ModuleBridge::HarmonicEditor(Box::new(HarmonicEditorUiBridge::try_new(m)?))
            }
            ModuleHandle::Expressions(m) => {
                ModuleBridge::Expressions(Box::new(ExpressionsUiBridge::try_new(m)?))
            }
            ModuleHandle::ExternalParam(m) => {
                ModuleBridge::ExternalParam(Box::new(ExternalParamUiBridge::try_new(m)?))
            }
            ModuleHandle::Output(_) => return Some(()),
        };

        bridges.insert(id, Some(bridge));

        Some(())
    }

    pub fn engine(&self) -> &EngineHandle {
        &self.engine
    }

    pub fn engine_params(&self) -> &EngineParams {
        &self.engine_params
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
                    module_type.default_label().into()
                } else {
                    module.label.clone()
                }
            })
            .unwrap_or_else(|| module_type.default_label().into())
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
        f: impl FnOnce(&mut Self, &mut ModuleBridge),
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

    pub fn get_linkable_inputs(&self, src: ModuleId, dst: ModuleId) -> Vec<LinkableInput> {
        let Some(dst_module) = self.routing.modules.get(&dst) else {
            return Vec::new();
        };

        let Some(src_module) = self.routing.modules.get(&src) else {
            return Vec::new();
        };

        let linkable: Vec<(Input, bool, Vec<ModuleId>)> = dst_module
            .inputs
            .iter()
            .filter_map(|meta| {
                if !self.is_linkable_input(src, dst, src_module.output_type, meta) {
                    return None;
                }

                let modulations = if meta.is_direct {
                    Vec::new()
                } else {
                    let input_id = InputId::new(meta.input_type, dst);

                    self.routing
                        .routing
                        .get(&input_id)
                        .map(|sources| match sources {
                            InputSource::Mixed(mixed) => mixed
                                .iter()
                                .filter(|source| source.modulation != Some(src))
                                .map(|source| source.module_id)
                                .collect(),
                            InputSource::Direct(_) => Vec::new(),
                        })
                        .unwrap_or_default()
                };

                Some((meta.input_type, meta.is_direct, modulations))
            })
            .collect();

        linkable
            .into_iter()
            .map(|(input_type, is_direct, mod_source_ids)| LinkableInput {
                input_type,
                is_direct,
                modulations: mod_source_ids
                    .into_iter()
                    .map(|module_id| LinkableModulation {
                        module_id,
                        label: self.display_module_label(module_id),
                    })
                    .collect(),
            })
            .collect()
    }

    /// Whether `src` can connect to any input on `dst`.
    pub fn has_linkable_input(&self, src: ModuleId, dst: ModuleId) -> bool {
        if src == dst {
            return false;
        }

        let Some(dst_module) = self.routing.modules.get(&dst) else {
            return false;
        };

        let Some(src_module) = self.routing.modules.get(&src) else {
            return false;
        };

        dst_module
            .inputs
            .iter()
            .any(|meta| self.is_linkable_input(src, dst, src_module.output_type, meta))
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
            InputSource::Direct(module_id) => {
                if !self.routing.modules.contains_key(module_id) {
                    return Vec::new();
                }

                vec![ConnectedInputSource {
                    src: *module_id,
                    amount: StereoSample::ONE,
                    label: self.display_module_label(*module_id),
                    modulation: None,
                }]
            }
            InputSource::Mixed(mixed) => mixed
                .iter()
                .filter(|source| self.routing.modules.contains_key(&source.module_id))
                .map(|source| ConnectedInputSource {
                    src: source.module_id,
                    amount: source.amount,
                    label: self.display_module_label(source.module_id),
                    modulation: source.modulation.map(|modulation| {
                        routing_state::InputModulation {
                            src: modulation,
                            label: self.display_module_label(modulation),
                        }
                    }),
                })
                .collect(),
        }
    }

    pub fn get_input_modulated_value(&self, input: InputId) -> Option<ModulatedValue> {
        if self.routing.routing.contains_key(&input)
            && self.has_active_voices()
            && let Some(module) = self.routing.modules.get(&input.module_id)
            && let Some(modulated) = self.modulated_inputs.get(&input).copied()
        {
            let is_mono =
                module.output_type == DataType::Spectral && !self.engine_params.stereo_spectrum;

            Some(ModulatedValue {
                value: modulated.value,
                normalized: modulated.normalized,
                is_stereo: !is_mono,
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

    // Whether `src` may target this destination input
    fn is_linkable_input(
        &self,
        src: ModuleId,
        dst: ModuleId,
        src_output_type: DataType,
        meta: &InputMeta,
    ) -> bool {
        let input_id = InputId::new(meta.input_type, dst);

        src != dst
            && data_types_compatible(src_output_type, meta.data_type)
            && !self.has_cycle(src, dst)
            && !self
                .routing
                .routing
                .get(&input_id)
                .is_some_and(|sources| sources.contains_module(src))
    }

    fn has_cycle(&self, dst_id: ModuleId, src_id: ModuleId) -> bool {
        for (input, sources) in &self.routing.routing {
            if input.module_id == dst_id {
                for source in sources.source_ids() {
                    if source == src_id || self.has_cycle(source, src_id) {
                        return true;
                    }
                }
            }
        }

        false
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
            }
        }

        let mut routing_refresh = false;

        for module in self.module_bridges.values_mut().filter_map(|m| m.as_mut()) {
            routing_refresh |= module.update();
        }

        if routing_refresh {
            self.update_routing();
        }
    }

    pub fn update_routing(&mut self) {
        let mut engine = self.engine.lock();

        if engine.refresh_routing().is_err() {
            log!("Failed to refresh routing");
        }
        self.routing = engine.get_routing_state();
    }

    pub fn add_module(&mut self, module_type: ModuleType, pos: GridVec) -> ModuleId {
        let mut synth = self.engine.lock();

        let id = match module_type {
            ModuleType::Output => OUTPUT_MODULE_ID,
            ModuleType::Amplifier => synth.add_amplifier(),
            ModuleType::Envelope => synth.add_envelope(),
            ModuleType::Mixer => synth.add_mixer(),
            ModuleType::Oscillator => synth.add_oscillator(),
            ModuleType::SpectralFilter => synth.add_spectral_filter(),
            ModuleType::SpectralBlend => synth.add_spectral_blend(),
            ModuleType::SpectralMixer => synth.add_spectral_mixer(),
            ModuleType::HarmonicEditor => synth.add_harmonic_editor(),
            ModuleType::ExternalParam => synth.add_external_param(),
            ModuleType::Lfo => synth.add_lfo(),
            ModuleType::WaveShaper => synth.add_wave_shaper(),
            ModuleType::Expressions => synth.add_expressions(),
        };

        self.routing = synth.get_routing_state();
        drop(synth);

        Self::insert_module_bridge(id, &self.engine, &mut self.module_bridges);

        let mut ui_config = self.ui_config.lock();

        ui_config.modules.insert(
            id,
            UiModuleConfig {
                id,
                label: "".into(),
                position: pos,
            },
        );

        id
    }

    pub fn remove_module(&mut self, module_id: ModuleId) {
        let mut synth = self.engine.lock();

        synth.remove_module(module_id);
        self.routing = synth.get_routing_state();
        self.module_bridges.remove(&module_id);
    }

    pub fn set_direct_link(&mut self, src: ModuleId, dst: InputId) {
        let mut synth = self.engine.lock();

        if let Err(err) = synth.set_direct_link(src, dst) {
            log!("Failed to set direct link: {err}");
        }
        self.routing = synth.get_routing_state();
    }

    pub fn add_link(&mut self, src: ModuleId, dst: InputId, amount: StereoSample) {
        let mut synth = self.engine.lock();

        if let Err(err) = synth.add_mixed_link(src, dst, amount) {
            log!("Failed to add link: {err}");
        }
        self.routing = synth.get_routing_state();
    }

    pub fn remove_link(&mut self, src: ModuleId, dst: InputId) {
        let mut synth = self.engine.lock();

        synth.remove_link(&src, &dst);
        self.routing = synth.get_routing_state();
    }

    pub fn remove_input_links(&mut self, dst: InputId) {
        let mut synth = self.engine.lock();

        synth.remove_input_links(&dst);
        self.routing = synth.get_routing_state();
    }

    pub fn remove_output_links(&mut self, src: ModuleId) {
        let mut synth = self.engine.lock();

        synth.remove_output_links(src);
        self.routing = synth.get_routing_state();
    }

    pub fn set_link_modulation(
        &mut self,
        src_id: ModuleId,
        dst_input: &InputId,
        modulator_id: ModuleId,
    ) {
        let mut synth = self.engine.lock();

        if let Err(err) = synth.set_link_modulation(src_id, dst_input, modulator_id) {
            log!("Failed to set link modulation: {err}");
        }
        self.routing = synth.get_routing_state();
    }

    pub fn remove_link_modulation(&mut self, src_id: ModuleId, dst_input: &InputId) {
        let mut synth = self.engine.lock();

        synth.remove_link_modulation(src_id, dst_input);
        self.routing = synth.get_routing_state();
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

    pub fn set_block_size(&mut self, block_size: usize) {
        if self.ui_end.set_block_size(block_size) {
            self.engine_params.block_size = block_size;
        }
    }

    pub fn set_voice_kill_time(&mut self, voice_kill_time: Sample) {
        if self.ui_end.set_voice_kill_time(voice_kill_time) {
            self.engine_params.voice_kill_time = voice_kill_time;
        }
    }

    pub fn set_oversampling(&mut self, oversampling: bool) {
        if self.ui_end.set_oversampling(oversampling) {
            self.engine_params.oversampling = oversampling;
        }
    }

    pub fn set_stereo_spectrum(&mut self, stereo_spectrum: bool) {
        if self.ui_end.set_stereo_spectrum(stereo_spectrum) {
            self.engine_params.stereo_spectrum = stereo_spectrum;
        }
    }

    pub fn set_bandwidth(&mut self, bandwidth: usize) {
        if self.ui_end.set_bandwidth(bandwidth) {
            self.engine_params.bandwidth = bandwidth;
        }
    }

    pub fn set_output_gain(&mut self, output_gain: StereoSample) {
        if self.ui_end.set_output_gain(output_gain) {
            self.engine_params.output_gain = output_gain;
        }
    }

    pub fn get_out_volume(&mut self) -> StereoSample {
        self.ui_end.get_out_volume()
    }
}

#[cfg(test)]
mod tests;
