use core::f32;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use itertools::Itertools;
use nih_plug::{params::FloatParam, prelude::FloatRange, util::db_to_gain_fast};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use topo_sort::{SortResults, TopoSort};

use crate::synth_engine::{
    buffer::{
        Buffer, SpectralBuffer, ZEROES_BUFFER, append_buffer_slice, fill_or_append_buffer_slice,
        make_zero_buffer,
    },
    config::{ModuleConfig, RoutingConfig},
    modules::{
        AmplifierConfig, EnvelopeConfig, ExternalParamConfig, HarmonicEditorConfig, LfoConfig,
        ModulationFilterConfig, OscillatorConfig, SpectralBlendConfig, SpectralFilterConfig,
    },
    routing::{AvailableInputSourceUI, DataType, MAX_VOICES, Router},
    synth_module::{NoteOffParams, NoteOnParams, ProcessParams, VoiceAlive},
};

pub use buffer::BUFFER_SIZE;
pub use config::Config;
pub use modules::{
    Amplifier, Envelope, EnvelopeCurve, ExternalParam, ExternalParamsBlock, HarmonicEditor, Lfo,
    LfoShape, ModulationFilter, Oscillator, SpectralBlend, SpectralFilter,
};
pub use routing::{
    ConnectedInputSourceUI, Input, ModuleId, ModuleInput, ModuleLink, ModuleType, OUTPUT_MODULE_ID,
};
pub use stereo_sample::StereoSample;
pub use synth_module::SynthModule;
pub use types::Sample;

mod buffer;
mod config;
#[macro_use]
mod synth_module;
mod curves;
mod modules;
mod phase;
mod routing;
mod stereo_sample;
mod types;

#[derive(Debug, Clone, Copy)]
pub struct VoiceId {
    pub voice_id: Option<i32>,
    pub channel: u8,
    pub note: u8,
}

#[derive(Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum VoiceOverride {
    Kill,
    Steal,
}

#[derive(Debug, Default, Clone, Copy)]
pub enum VoiceState {
    NoteOn,
    Release,
    Kill,
    #[default]
    Free,
}

#[derive(Debug, Default, Clone, Copy)]
struct Voice {
    id: u64,
    external_voice_id: Option<i32>,
    channel: u8,
    note: u8,
    state: VoiceState,
}

impl Voice {
    fn get_id(&self) -> VoiceId {
        VoiceId {
            voice_id: self.external_voice_id,
            channel: self.channel,
            note: self.note,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct ModuleInputSource {
    src: ModuleId,
    modulation: StereoSample,
}

pub struct SynthEngine {
    next_id: ModuleId,
    next_voice_id: u64,
    sample_rate: f32,
    buffer_size: usize,
    num_voices: usize,
    voice_override: VoiceOverride,
    config: Arc<Config>,
    modules: HashMap<ModuleId, Option<Box<dyn SynthModule>>>,
    input_sources: HashMap<ModuleInput, Vec<ModuleInputSource>>,
    modules_to_execute: HashSet<ModuleId>,
    execution_order: Vec<ModuleId>,
    voices: [Voice; MAX_VOICES],
    external_params: Option<Arc<ExternalParamsBlock>>,
    output_level: StereoSample,
    output_level_param: Arc<FloatParam>,
    tmp_output_buffer: Option<Box<(Buffer, Buffer)>>,
}

macro_rules! get_module {
    ($self:ident, $module_id:expr) => {
        $self
            .modules
            .get($module_id)
            .and_then(|result| result.as_deref())
    };
}

macro_rules! get_module_mut {
    ($self:ident, $module_id:expr) => {
        $self
            .modules
            .get_mut($module_id)
            .and_then(|result| result.as_deref_mut())
    };
}

macro_rules! add_module_method {
    ($func_name:ident, $module_type:ident, $module_cfg:ident $(, $arg:ident )*) => {
        pub fn $func_name(&mut self) -> ModuleId {
            let id = self.alloc_next_id();
            let config = Arc::new(Mutex::new($module_cfg::default()));
            let mut module = $module_type::new(id, Arc::clone(&config) $(, self.$arg() )*);

            Self::trigger_active_notes(&self.voices, &mut module);
            self.modules.insert(id, Some(Box::new(module)));
            self.config
                .modules
                .lock()
                .insert(id, ModuleConfig::$module_type(Arc::clone(&config)));
            id
        }
    };
}

impl SynthEngine {
    pub fn new() -> Self {
        let default_cfg = RoutingConfig::default();

        Self {
            next_id: default_cfg.next_module_id,
            next_voice_id: 1,
            sample_rate: 1000.0,
            buffer_size: default_cfg.buffer_size,
            num_voices: default_cfg.num_voices,
            voice_override: default_cfg.voice_override,
            config: Default::default(),
            modules: HashMap::new(),
            input_sources: HashMap::new(),
            modules_to_execute: HashSet::new(),
            execution_order: Vec::new(),
            voices: Default::default(),
            external_params: None,
            output_level: StereoSample::splat(0.25),
            output_level_param: Arc::new(FloatParam::new(
                "",
                0.0,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )),
            tmp_output_buffer: Some(Box::new((make_zero_buffer(), make_zero_buffer()))),
        }
    }

    pub fn init(
        &mut self,
        config: Arc<Config>,
        output_level_param: Arc<FloatParam>,
        external_params: ExternalParamsBlock,
        sample_rate: Sample,
    ) {
        self.config = config;
        self.sample_rate = sample_rate;
        self.output_level_param = output_level_param;
        self.external_params = Some(Arc::new(external_params));

        if !self.load_config() {
            self.clear();
        }
    }

    pub fn is_empty(&self) -> bool {
        self.modules.is_empty()
    }

    pub fn get_voices_num(&self) -> usize {
        self.num_voices
    }

    pub fn set_num_voices(&mut self, num_voices: usize) {
        self.num_voices = Self::clamp_num_voices(num_voices);
        self.config.routing.lock().num_voices = self.num_voices;

        self.voices
            .iter_mut()
            .filter(|v| !matches!(v.state, VoiceState::Free))
            .skip(self.num_voices)
            .for_each(|v| v.state = VoiceState::Free);
    }

    pub fn get_buffer_size(&self) -> usize {
        self.buffer_size
    }

    pub fn set_buffer_size(&mut self, buffer_size: usize) {
        self.buffer_size = Self::clamp_buffer_size(buffer_size);
        self.config.routing.lock().buffer_size = self.buffer_size;
    }

    pub fn get_voice_override(&self) -> VoiceOverride {
        self.voice_override
    }

    pub fn set_voice_override(&mut self, voice_override: VoiceOverride) {
        self.voice_override = voice_override;
        self.config.routing.lock().voice_override = voice_override;
    }

    fn clamp_num_voices(num_voices: usize) -> usize {
        num_voices.clamp(1, MAX_VOICES)
    }

    fn clamp_buffer_size(buffer_size: usize) -> usize {
        (buffer_size).clamp(BUFFER_SIZE / 8, BUFFER_SIZE)
    }

    add_module_method!(add_oscillator, Oscillator, OscillatorConfig);
    add_module_method!(add_envelope, Envelope, EnvelopeConfig);
    add_module_method!(add_lfo, Lfo, LfoConfig);
    add_module_method!(add_amplifier, Amplifier, AmplifierConfig);
    add_module_method!(add_spectral_filter, SpectralFilter, SpectralFilterConfig);
    add_module_method!(add_spectral_blend, SpectralBlend, SpectralBlendConfig);
    add_module_method!(add_harmonic_editor, HarmonicEditor, HarmonicEditorConfig);
    add_module_method!(
        add_external_param,
        ExternalParam,
        ExternalParamConfig,
        get_external_params
    );
    add_module_method!(
        add_modulation_filter,
        ModulationFilter,
        ModulationFilterConfig
    );

    fn get_external_params(&self) -> Arc<ExternalParamsBlock> {
        Arc::clone(self.external_params.as_ref().unwrap())
    }

    pub fn remove_module(&mut self, id: ModuleId) {
        if !self.modules.contains_key(&id) {
            return;
        };

        self.modules.remove(&id);
        self.config.modules.lock().remove(&id);

        let new_links: Vec<_> = self
            .get_links()
            .into_iter()
            .filter(|link| !(link.src == id || link.dst.module_id == id))
            .collect();

        self.setup_routing(&new_links).unwrap();
    }

    pub fn has_module_id(&self, module_id: ModuleId) -> bool {
        self.modules.contains_key(&module_id)
    }

    pub fn set_direct_link(&mut self, src: ModuleId, dst: ModuleInput) -> Result<(), String> {
        self.can_be_linked(&src, &dst)?;

        let mut new_links: Vec<_> = self
            .get_links()
            .iter()
            .filter(|link| link.dst != dst)
            .copied()
            .collect();

        new_links.push(ModuleLink::link(src, dst));
        self.setup_routing(&new_links)?;
        self.save_links();
        Ok(())
    }

    pub fn add_modulation(
        &mut self,
        src: ModuleId,
        dst: ModuleInput,
        amount: StereoSample,
    ) -> Result<(), String> {
        self.can_be_linked(&src, &dst)?;

        if self.already_linked(&src, &dst) {
            return Ok(());
        }

        let mut new_links = self.get_links();

        new_links.push(ModuleLink::modulation(src, dst, amount));
        self.setup_routing(&new_links)?;
        self.save_links();
        Ok(())
    }

    pub fn update_modulation(&mut self, src: &ModuleId, dst: &ModuleInput, amount: StereoSample) {
        if let Some(inputs) = self.input_sources.get_mut(dst)
            && let Some(input) = inputs.iter_mut().find(|input| input.src == *src)
        {
            input.modulation = amount;
        }

        self.save_links();
    }

    pub fn add_link(&mut self, src: ModuleId, dst: ModuleInput) -> Result<(), String> {
        self.can_be_linked(&src, &dst)?;

        let mut new_links: Vec<_> = self.get_links();

        new_links.push(ModuleLink::link(src, dst));
        self.setup_routing(&new_links)?;
        self.save_links();
        Ok(())
    }

    pub fn remove_link(&mut self, src: &ModuleId, dst: &ModuleInput) {
        let new_links: Vec<_> = self
            .get_links()
            .into_iter()
            .filter(|link| !(link.src == *src && link.dst == *dst))
            .collect();

        self.setup_routing(&new_links).unwrap();
        self.save_links();
    }

    pub fn get_output_level(&self) -> StereoSample {
        self.output_level
    }

    pub fn set_output_level(&mut self, level: StereoSample) {
        self.output_level = level;
        self.config.routing.lock().output_level = level;
    }

    fn playing_voices(voices: &mut [Voice]) -> SmallVec<[(usize, &mut Voice); MAX_VOICES]> {
        voices
            .iter_mut()
            .enumerate()
            .filter(|(_, v)| matches!(v.state, VoiceState::NoteOn | VoiceState::Release))
            .sorted_by_key(|(_, v)| v.id)
            .collect()
    }

    fn find_free_voice(&self) -> (usize, bool) {
        // Find free voice
        if let Some((voice_idx, _)) = self
            .voices
            .iter()
            .enumerate()
            .filter(|(_, v)| matches!(v.state, VoiceState::Free))
            .min_by_key(|(_, v)| v.id)
        {
            return (voice_idx, false);
        }

        // Steal playing voice
        if let Some((voice_idx, _)) = self
            .voices
            .iter()
            .enumerate()
            .filter(|(_, v)| matches!(v.state, VoiceState::NoteOn | VoiceState::Release))
            .min_by_key(|(_, v)| v.id)
        {
            return (voice_idx, true);
        }

        // Steal killed voice as last resort
        (
            self.voices
                .iter()
                .enumerate()
                .filter(|(_, v)| matches!(v.state, VoiceState::Kill))
                .min_by_key(|(_, v)| v.id)
                .map_or(0, |(voice_idx, _)| voice_idx),
            true,
        )
    }

    fn find_or_steal_voice(&mut self) -> (usize, bool) {
        let playing_voices = Self::playing_voices(&mut self.voices);

        // Steal excess voice
        if playing_voices.len() >= self.num_voices {
            return (
                playing_voices
                    .first()
                    .map_or(0, |(voice_idx, _)| *voice_idx),
                true,
            );
        }

        drop(playing_voices);
        self.find_free_voice()
    }

    fn find_or_kill_voice(&mut self) -> (usize, bool) {
        let mut playing_voices = Self::playing_voices(&mut self.voices);

        // Kill excess voice
        if playing_voices.len() >= self.num_voices
            && let Some((voice_idx, voice)) = playing_voices.first_mut()
        {
            for module_id in &self.execution_order {
                if let Some(module) = get_module_mut!(self, &module_id) {
                    module.kill_voice(*voice_idx);
                }
            }

            voice.state = VoiceState::Kill;
        }

        drop(playing_voices);
        self.find_free_voice()
    }

    pub fn note_on(
        &mut self,
        voice_id: Option<i32>,
        channel: u8,
        note: u8,
        _velocity: f32,
    ) -> Option<VoiceId> {
        let mut terminated_voice: Option<VoiceId> = None;

        let (voice_idx, stolen) = match self.voice_override {
            VoiceOverride::Kill => self.find_or_kill_voice(),
            VoiceOverride::Steal => self.find_or_steal_voice(),
        };
        let dst_voice = &mut self.voices[voice_idx];

        if stolen {
            terminated_voice = Some(dst_voice.get_id());
        }

        *dst_voice = Voice {
            id: self.next_voice_id,
            external_voice_id: voice_id,
            channel,
            note,
            state: VoiceState::NoteOn,
        };

        self.next_voice_id = self.next_voice_id.wrapping_add(1);

        let params = NoteOnParams {
            note: note as f32,
            voice_idx,
            reset: !stolen,
        };

        for module_id in &self.execution_order {
            if let Some(module) = get_module_mut!(self, &module_id) {
                module.note_on(&params);
            }
        }

        terminated_voice
    }

    pub fn note_off(&mut self, note: u8) {
        let Some(voice_idx) = self
            .voices
            .iter()
            .position(|voice| voice.note == note && matches!(voice.state, VoiceState::NoteOn))
        else {
            return;
        };

        self.voices[voice_idx].state = VoiceState::Release;

        let params = NoteOffParams { voice_idx };

        for module_id in &self.execution_order {
            if let Some(module) = get_module_mut!(self, &module_id) {
                module.note_off(&params);
            }
        }
    }

    pub fn choke(&mut self, note: u8) -> Option<VoiceId> {
        let voice = self
            .voices
            .iter_mut()
            .find(|voice| voice.note == note && !matches!(voice.state, VoiceState::Free))?;

        voice.state = VoiceState::Free;
        Some(voice.get_id())
    }

    pub fn process<'a>(
        &mut self,
        samples: usize,
        outputs: impl Iterator<Item = &'a mut [f32]>,
        on_terminate_voice: &mut dyn FnMut(VoiceId),
    ) {
        let mut alive_voices: SmallVec<[VoiceAlive; MAX_VOICES]> = self
            .voices
            .iter()
            .enumerate()
            .filter(|(_, v)| !matches!(v.state, VoiceState::Free))
            .map(|(voice_idx, v)| VoiceAlive::new(voice_idx, v.state))
            .collect();

        self.execution_order
            .iter()
            .filter_map(|id| get_module!(self, id))
            .for_each(|module| module.poll_alive_voices(&mut alive_voices));

        for completed in alive_voices.iter().filter(|alive| !alive.alive()) {
            let voice = &mut self.voices[completed.index()];

            on_terminate_voice(voice.get_id());
            voice.state = VoiceState::Free;
        }

        let active_idx: SmallVec<[usize; MAX_VOICES]> = alive_voices
            .iter()
            .filter(|alive| alive.alive())
            .map(|alive| alive.index())
            .collect();

        let params = ProcessParams {
            samples,
            sample_rate: self.sample_rate,
            buffer_t_step: samples as Sample / self.sample_rate,
            active_voices: &active_idx,
        };

        for module_id in &self.execution_order {
            if let Some(module_box) = self.modules.get_mut(module_id)
                && let Some(mut module) = module_box.take()
            {
                module.process(&params, self);
                self.modules.get_mut(module_id).unwrap().replace(module);
            }
        }

        self.write_output(&params, outputs);
    }

    fn alloc_next_id(&mut self) -> ModuleId {
        let module_id = self.next_id;

        self.next_id += 1;
        self.config.routing.lock().next_module_id = self.next_id;
        module_id
    }

    fn trigger_active_notes(voices: &[Voice], module: &mut dyn SynthModule) {
        let active_voices = voices
            .iter()
            .enumerate()
            .filter(|(_, voice)| !matches!(voice.state, VoiceState::Free))
            .map(|(voice_idx, voice)| NoteOnParams {
                note: voice.note as f32,
                voice_idx,
                reset: true,
            });

        for params in active_voices {
            module.note_on(&params);
        }
    }

    fn data_types_compatible(src: &[DataType], dst: DataType) -> bool {
        src.contains(&dst) || (dst == DataType::Buffer && src.contains(&DataType::Scalar))
    }

    fn can_be_linked_with_output(&self, src: &ModuleId, dst: &ModuleInput) -> Result<(), String> {
        let Some(src_module) = get_module!(self, src) else {
            return Err("Invalid node.".to_string());
        };

        let is_compatible = dst.input_type == Input::Audio
            && Self::data_types_compatible(src_module.outputs(), DataType::Buffer);

        if !is_compatible {
            return Err("Data types mismatch.".to_string());
        }

        Ok(())
    }

    fn can_be_linked(&self, src: &ModuleId, dst: &ModuleInput) -> Result<(), String> {
        if dst.module_id == OUTPUT_MODULE_ID {
            return self.can_be_linked_with_output(src, dst);
        }

        let (Some(src_module), Some(dst_module)) =
            (get_module!(self, src), get_module!(self, &dst.module_id))
        else {
            return Err("Invalid node.".to_string());
        };

        let src_data_types = src_module.outputs();

        let is_compatible = dst_module.inputs().iter().any(|input_info| {
            input_info.input == dst.input_type
                && Self::data_types_compatible(src_data_types, input_info.data_type)
        });

        if !is_compatible {
            return Err("Data types mismatch.".to_string());
        }

        Ok(())
    }

    fn already_linked(&self, src: &ModuleId, dst: &ModuleInput) -> bool {
        if let Some(inputs) = self.input_sources.get(dst) {
            inputs.iter().any(|input| input.src == *src)
        } else {
            false
        }
    }

    fn get_links(&self) -> Vec<ModuleLink> {
        self.input_sources
            .iter()
            .flat_map(|(dst, sources)| {
                sources.iter().map(|src| ModuleLink {
                    dst: *dst,
                    src: src.src,
                    modulation: src.modulation,
                })
            })
            .collect()
    }

    fn setup_routing(&mut self, links: &[ModuleLink]) -> Result<(), String> {
        let execution_order = Self::calc_execution_order(links)?;
        let mut input_sources: HashMap<ModuleInput, Vec<ModuleInputSource>> = HashMap::new();

        for link in links {
            input_sources
                .entry(link.dst)
                .or_default()
                .push(ModuleInputSource {
                    src: link.src,
                    modulation: link.modulation,
                });
        }

        self.input_sources = input_sources;
        self.modules_to_execute = HashSet::from_iter(execution_order.iter().copied());
        self.execution_order = execution_order;
        Ok(())
    }

    fn write_output<'a>(
        &mut self,
        params: &ProcessParams,
        outputs: impl Iterator<Item = &'a mut [f32]>,
    ) {
        let mut tmp_buffers = self.tmp_output_buffer.take().unwrap();

        self.output_level_param.smoothed.next_block_mapped(
            &mut tmp_buffers.0,
            params.samples,
            |_, dbs| db_to_gain_fast(dbs),
        );

        for (channel, (output, level)) in outputs.zip(self.output_level.iter()).enumerate() {
            output.fill(0.0);

            for voice_idx in params.active_voices.iter() {
                let input = self
                    .get_input(
                        ModuleInput::new(Input::Audio, OUTPUT_MODULE_ID),
                        params.samples,
                        *voice_idx,
                        channel,
                        &mut tmp_buffers.1,
                    )
                    .unwrap_or(&ZEROES_BUFFER);

                append_buffer_slice(
                    output,
                    input
                        .iter()
                        .zip(&tmp_buffers.0)
                        .map(|(input, level_mod)| input * level * level_mod),
                );
            }
        }

        self.tmp_output_buffer.replace(tmp_buffers);
    }

    pub fn get_modules(&self) -> Vec<&dyn SynthModule> {
        self.modules
            .values()
            .filter_map(|val| val.as_deref())
            .collect()
    }

    pub fn get_module_mut(&mut self, id: ModuleId) -> Option<&mut dyn SynthModule> {
        get_module_mut!(self, &id)
    }

    pub fn get_available_input_sources(&self, input: ModuleInput) -> Vec<AvailableInputSourceUI> {
        let Some(input_module) = get_module!(self, &input.module_id) else {
            return Vec::new();
        };

        let Some(input_info) = input_module
            .inputs()
            .iter()
            .find(|input_info| input_info.input == input.input_type)
        else {
            return Vec::new();
        };

        self.modules
            .values()
            .filter_map(|module| module.as_deref())
            .filter(|module| {
                module.id() != input.module_id
                    && Self::data_types_compatible(module.outputs(), input_info.data_type)
                    && !self.is_connected_to_source(module.id(), input.module_id)
            })
            .map(|module| AvailableInputSourceUI {
                output: module.id(),
                label: module.label(),
            })
            .collect()
    }

    pub fn get_connected_input_sources(&self, input: ModuleInput) -> Vec<ConnectedInputSourceUI> {
        let Some(sources) = self.input_sources.get(&input) else {
            return Vec::new();
        };

        sources
            .iter()
            .filter_map(|source| get_module!(self, &source.src).map(|module| (module, source)))
            .map(|(module, source)| ConnectedInputSourceUI {
                output: source.src,
                modulation: source.modulation,
                label: module.label(),
            })
            .collect()
    }

    fn is_connected_to_source(&self, dst_id: ModuleId, src_id: ModuleId) -> bool {
        for (input, sources) in &self.input_sources {
            if input.module_id == dst_id {
                for source in sources {
                    if source.src == src_id || self.is_connected_to_source(source.src, src_id) {
                        return true;
                    }
                }
            }
        }

        false
    }

    fn calc_execution_order(links: &[ModuleLink]) -> Result<Vec<ModuleId>, String> {
        let mut dependents: HashMap<ModuleId, HashSet<ModuleId>> = HashMap::new();

        for link in links {
            let src_node = link.src;
            let dst_node = link.dst.module_id;

            dependents.entry(dst_node).or_default().insert(src_node);
            dependents.entry(src_node).or_default();
        }

        let topo_sort = TopoSort::from_map(dependents);

        match topo_sort.into_vec_nodes() {
            SortResults::Full(nodes) => Ok(nodes
                .into_iter()
                .filter(|node| *node != OUTPUT_MODULE_ID)
                .collect()),
            SortResults::Partial(_) => Err("Cycles detected!".to_string()),
        }
    }

    fn clear(&mut self) {
        let default_cfg = RoutingConfig::default();

        self.execution_order.clear();
        self.input_sources.clear();
        self.modules.clear();
        self.next_id = default_cfg.next_module_id;
        self.num_voices = default_cfg.num_voices;
        self.buffer_size = default_cfg.buffer_size;
        self.voice_override = VoiceOverride::Kill;

        *self.config.routing.lock() = default_cfg;
        self.config.modules.lock().clear();
    }

    fn load_config(&mut self) -> bool {
        let routing_arc = Arc::clone(&self.config.routing);
        let routing = routing_arc.lock();
        let modules_arc = Arc::clone(&self.config.modules);
        let modules_cfg = modules_arc.lock();

        if modules_cfg.is_empty() {
            return false;
        }

        self.next_id = routing.next_module_id;
        self.output_level = routing.output_level;
        self.num_voices = Self::clamp_num_voices(routing.num_voices);
        self.buffer_size = Self::clamp_buffer_size(routing.buffer_size);
        self.voice_override = routing.voice_override;

        macro_rules! restore_module {
            ($module_type:ident, $module_id:ident, $cfg:ident $(, $arg:ident )*) => {{
                self.modules.insert(
                    *$module_id,
                    Some(Box::new($module_type::new(*$module_id, Arc::clone($cfg) $(, self.$arg() )*))),
                );
            }};
            ($module_type:ident, $module_id:ident) => {{
                self.modules
                    .insert(*$module_id, Some(Box::new($module_type::new(*$module_id))));
            }};
        }

        for (id, cfg) in modules_cfg.iter() {
            match cfg {
                ModuleConfig::Amplifier(cfg) => restore_module!(Amplifier, id, cfg),
                ModuleConfig::Envelope(cfg) => restore_module!(Envelope, id, cfg),
                ModuleConfig::Oscillator(cfg) => restore_module!(Oscillator, id, cfg),
                ModuleConfig::SpectralFilter(cfg) => restore_module!(SpectralFilter, id, cfg),
                ModuleConfig::SpectralBlend(cfg) => restore_module!(SpectralBlend, id, cfg),
                ModuleConfig::HarmonicEditor(cfg) => restore_module!(HarmonicEditor, id, cfg),
                ModuleConfig::ExternalParam(cfg) => {
                    restore_module!(ExternalParam, id, cfg, get_external_params)
                }
                ModuleConfig::ModulationFilter(cfg) => restore_module!(ModulationFilter, id, cfg),
                ModuleConfig::Lfo(cfg) => restore_module!(Lfo, id, cfg),
            }
        }

        for link in &routing.links {
            if self.can_be_linked(&link.src, &link.dst).is_err() {
                return false;
            }
        }

        self.setup_routing(&routing.links).is_ok()
    }

    fn save_links(&self) {
        self.config.routing.lock().links = self.get_links();
    }
}

impl Router for SynthEngine {
    fn get_input<'a>(
        &'a self,
        input: ModuleInput,
        samples: usize,
        voice_idx: usize,
        channel_idx: usize,
        input_buffer: &'a mut Buffer,
    ) -> Option<&'a Buffer> {
        let sources = self.input_sources.get(&input)?;

        if sources.is_empty() {
            return None;
        }

        if sources.len() == 1
            && let Some(first) = sources.first()
            && first.modulation == StereoSample::ONE
            && let Some(module) = get_module!(self, &first.src)
            && module.outputs().contains(&DataType::Buffer)
        {
            return Some(module.get_buffer_output(voice_idx, channel_idx));
        }

        let result = &mut input_buffer[..samples];

        let modules = sources.iter().filter_map(|source| {
            get_module!(self, &source.src)
                .map(|module| (module, source.modulation, module.outputs()))
        });

        for (mod_idx, (module, modulation, data_types)) in modules.enumerate() {
            let mod_amount = modulation[channel_idx];

            if data_types.contains(&DataType::Buffer) {
                let buff = module.get_buffer_output(voice_idx, channel_idx);

                fill_or_append_buffer_slice(
                    mod_idx == 0,
                    result,
                    buff.iter().map(|sample| sample * mod_amount),
                );
            } else {
                let from_value = module.get_scalar_output(false, voice_idx, channel_idx);
                let to_value = module.get_scalar_output(true, voice_idx, channel_idx);
                let step = (to_value - from_value) * (samples as Sample).recip();

                fill_or_append_buffer_slice(
                    mod_idx == 0,
                    result,
                    (0..samples).map(|idx| (from_value + step * idx as Sample) * mod_amount),
                );
            };
        }

        Some(input_buffer)
    }

    fn get_spectral_input(
        &self,
        input: ModuleInput,
        current: bool,
        voice_idx: usize,
        channel: usize,
    ) -> Option<&SpectralBuffer> {
        let sources = self.input_sources.get(&input)?;

        if sources.is_empty() {
            return None;
        }

        get_module!(self, &sources[0].src)
            .map(|module| module.get_spectral_output(current, voice_idx, channel))
    }

    fn get_scalar_input(
        &self,
        input: ModuleInput,
        current: bool,
        voice_idx: usize,
        channel: usize,
    ) -> Option<Sample> {
        let sources = self.input_sources.get(&input)?;

        if sources.is_empty() {
            return None;
        }

        let mut output: Sample = 0.0;

        let values = sources.iter().filter_map(|source| {
            get_module!(self, &source.src).map(|module| {
                (
                    module.get_scalar_output(current, voice_idx, channel),
                    source.modulation,
                )
            })
        });

        for (value, mod_amount) in values {
            output += value * mod_amount[channel];
        }

        Some(output)
    }
}
