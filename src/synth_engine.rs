use core::f32;
use std::{
    any::Any,
    collections::{HashMap, HashSet},
    sync::Arc,
};

use nih_plug::params::FloatParam;
use rustc_hash::FxHashMap;
use smallvec::SmallVec;
use std::assert_matches;
use topo_sort::{SortResults, TopoSort};

use crate::synth_engine::{
    buffer::{add_to_buffer, copy_or_add_to_buffer},
    modules::Output,
    outputs_arena::{InputSlots, OutputsArena, SamplesInputSrc, SpectralInputSlot},
    routing::{DataType, LinkModulation, MIN_MODULE_ID, data_types_compatible},
    voices_handler::{
        DecayingVoices, MAX_AVAILABLE_VOICES, PlayingVoices, VoiceEvents, VoicesHandler,
    },
};

pub use buffer::{Buffer, HARMONIC_SERIES_BUFFER, SPECTRAL_BUFFER_SIZE, SpectralBuffer};
pub use config::{EngineConfig, EngineParams, LinkConfig, ModuleConfig};
pub use modules::{
    Amplifier, Envelope, EnvelopeCurve, Expressions, ExternalParam, ExternalParamsBlock, Lfo,
    LfoShape, Mixer, Oscillator, ShaperType, SpectralBlend, SpectralFilter, SpectralFilterType,
    SpectralMixer, WaveShaper,
    amplifier::{self},
    envelope::{self},
    expressions::{self},
    external_param::{self},
    harmonic_editor::{self, HarmonicEditor},
    lfo::{self},
    mixer::{self},
    oscillator::{self},
    spectral_blend::{self},
    spectral_filter::{self},
    spectral_mixer::{self},
    wave_shaper::{self},
};
pub use routing::{
    Expression, Input, MixType, ModuleId, ModuleInput, ModuleLink, ModuleType, NUM_CHANNELS,
    OUTPUT_MODULE_ID, Router, VoiceEvent, VolumeType,
};
pub use smooth::SmoothedSampleParams;
pub use stereo_sample::StereoSample;
pub use synth_module::{ModuleUiBridge, ProcessParams, SynthModule};
pub use types::Sample;

mod buffer;
mod config;
#[macro_use]
mod synth_module;
mod biquad_filter;
mod curves;
mod iir_decimator;
mod modules;
mod outputs_arena;
mod phase;
mod routing;
mod smooth;
mod stereo_sample;
mod types;
pub mod ui_bridge;
mod voices_handler;

#[cfg(test)]
mod tests;

pub const MAX_BLOCK_SIZE: usize = 128;

#[derive(Debug, Clone, Copy)]
pub struct ModuleInputSource {
    module_id: ModuleId,
    amount: StereoSample,
    modulation: Option<LinkModulation>,
}

impl ModuleInputSource {
    fn source_ids(&self) -> impl Iterator<Item = ModuleId> {
        let mut ids: SmallVec<[ModuleId; 2]> = SmallVec::new();

        ids.push(self.module_id);

        if let Some(modulation) = self.modulation {
            ids.push(modulation);
        }

        ids.into_iter()
    }
}

type ModulesMap = FxHashMap<ModuleId, Option<Box<dyn SynthModule>>>;
type RoutingMap = FxHashMap<ModuleInput, Vec<ModuleInputSource>>;

trait ModuleAccess {
    fn get_module(&self, id: ModuleId) -> Option<&dyn SynthModule>;

    fn get_typed_module<T: SynthModule>(&self, id: ModuleId) -> Option<&T> {
        self.get_module(id)
            .and_then(|module| (module as &dyn Any).downcast_ref())
    }

    fn get_module_mut(&mut self, id: ModuleId) -> Option<&mut dyn SynthModule>;

    fn get_typed_module_mut<T: SynthModule>(&mut self, id: ModuleId) -> Option<&mut T> {
        self.get_module_mut(id)
            .and_then(|module| (module as &mut dyn Any).downcast_mut())
    }
}

impl ModuleAccess for ModulesMap {
    fn get_module(&self, id: ModuleId) -> Option<&dyn SynthModule> {
        self.get(&id).and_then(|module| module.as_deref())
    }

    fn get_module_mut(&mut self, id: ModuleId) -> Option<&mut dyn SynthModule> {
        self.get_mut(&id).and_then(|module| module.as_deref_mut())
    }
}

pub struct SynthEngine {
    next_id: ModuleId,
    host_sample_rate: f32,
    block_size: usize,
    oversampling: bool,
    spectrum_channels: usize,
    modules: ModulesMap,
    input_sources: RoutingMap,
    execution_order: Vec<ModuleId>,
    voices_handler: VoicesHandler,
    external_params: Option<Arc<ExternalParamsBlock>>,
    audio_end: ui_bridge::AudioEnd,
    ui_end: Option<ui_bridge::UiEnd>,
    outputs_arena: OutputsArena,
}

macro_rules! add_module_method {
    ($func_name:ident, $module_type:ident $(, $arg:ident )*) => {
        pub fn $func_name(&mut self) -> ModuleId {
            let id = self.alloc_module_id();
            let module = Box::new($module_type::new(id $(, self.$arg() )*));

            self.modules.insert(id, Some(module));
            id
        }
    };
}

impl SynthEngine {
    pub const AVAILABLE_VOICES: usize = MAX_AVAILABLE_VOICES;

    pub fn try_new(
        cfg: &EngineConfig,
        output_level_param: Arc<FloatParam>,
        external_params: Arc<ExternalParamsBlock>,
        host_sample_rate: Sample,
    ) -> Option<Self> {
        let (audio_end, ui_end) = ui_bridge::create_link_pair();

        let mut engine = Self {
            next_id: 1,
            host_sample_rate,
            block_size: Self::clamp_block_size(cfg.engine.block_size),
            oversampling: cfg.engine.oversampling,
            spectrum_channels: Self::stereo_spectrum_channels(cfg.engine.stereo_spectrum),
            modules: ModulesMap::default(),
            input_sources: RoutingMap::default(),
            execution_order: Vec::new(),
            voices_handler: VoicesHandler::new(
                Self::clamp_num_voices(cfg.engine.num_voices),
                cfg.engine.legato,
            ),
            external_params: Some(external_params.clone()),
            audio_end,
            ui_end: Some(ui_end),
            outputs_arena: OutputsArena::new(),
        };

        engine.modules.insert(
            OUTPUT_MODULE_ID,
            Some(Box::new(Output::new(
                cfg.engine.output_gain,
                cfg.engine.voice_kill_time,
                output_level_param,
            ))),
        );

        macro_rules! from_cfg {
            ($module_type:ident, $cfg:expr) => {
                Box::new($module_type::from_config($cfg)) as Box<dyn SynthModule>
            };
        }

        let mut max_module_id = MIN_MODULE_ID;

        for module_cfg in cfg.modules.iter() {
            let module = match module_cfg {
                ModuleConfig::Oscillator(cfg) => from_cfg!(Oscillator, cfg),
                ModuleConfig::Envelope(cfg) => from_cfg!(Envelope, cfg),
                ModuleConfig::Lfo(cfg) => from_cfg!(Lfo, cfg),
                ModuleConfig::Amplifier(cfg) => from_cfg!(Amplifier, cfg),
                ModuleConfig::Mixer(cfg) => from_cfg!(Mixer, cfg),
                ModuleConfig::WaveShaper(cfg) => from_cfg!(WaveShaper, cfg),
                ModuleConfig::SpectralFilter(cfg) => from_cfg!(SpectralFilter, cfg),
                ModuleConfig::SpectralBlend(cfg) => from_cfg!(SpectralBlend, cfg),
                ModuleConfig::SpectralMixer(cfg) => from_cfg!(SpectralMixer, cfg),
                ModuleConfig::HarmonicEditor(cfg) => from_cfg!(HarmonicEditor, cfg),
                ModuleConfig::Expressions(cfg) => from_cfg!(Expressions, cfg),
                ModuleConfig::ExternalParam(cfg) => {
                    Box::new(ExternalParam::from_config(cfg, external_params.clone()))
                        as Box<dyn SynthModule>
                }
            };

            let module_id = module.id();

            if module_id < MIN_MODULE_ID || engine.modules.contains_key(&module_id) {
                return None;
            }

            if module_id > max_module_id {
                max_module_id = module_id;
            }

            engine.modules.insert(module_id, Some(module));
        }

        engine.next_id = max_module_id + 1;

        for link in cfg.links.iter() {
            if !engine.add_config_link(link) {
                return None;
            }
        }

        Some(engine)
    }

    pub fn get_config(&self) -> EngineConfig {
        let mut module_ids: Vec<_> = self.modules.keys().copied().collect();

        module_ids.sort_unstable();

        macro_rules! to_cfg {
            ($module_type:ident, $id:expr) => {
                self.get_typed_module::<$module_type>($id)
                    .map(|module| ModuleConfig::$module_type(Box::new(module.get_config())))
            };
        }

        let modules = module_ids
            .iter()
            .filter_map(|&id| match self.modules.get_module(id)?.module_type() {
                ModuleType::Oscillator => to_cfg!(Oscillator, id),
                ModuleType::Envelope => to_cfg!(Envelope, id),
                ModuleType::Lfo => to_cfg!(Lfo, id),
                ModuleType::Amplifier => to_cfg!(Amplifier, id),
                ModuleType::Mixer => to_cfg!(Mixer, id),
                ModuleType::WaveShaper => to_cfg!(WaveShaper, id),
                ModuleType::SpectralFilter => to_cfg!(SpectralFilter, id),
                ModuleType::SpectralBlend => to_cfg!(SpectralBlend, id),
                ModuleType::SpectralMixer => to_cfg!(SpectralMixer, id),
                ModuleType::HarmonicEditor => to_cfg!(HarmonicEditor, id),
                ModuleType::Expressions => to_cfg!(Expressions, id),
                ModuleType::ExternalParam => to_cfg!(ExternalParam, id),
                ModuleType::Output => None,
            })
            .collect();

        EngineConfig {
            engine: self.get_engine_params(),
            modules,
            links: self
                .get_links()
                .into_iter()
                .map(|link| LinkConfig {
                    src_id: link.src,
                    dst_id: link.dst.module_id,
                    dst_input: link.dst.input_type,
                    amount: link.amount,
                    modulator_id: link.modulation,
                })
                .collect(),
        }
    }

    fn sample_rate(&self) -> Sample {
        if self.oversampling {
            2.0 * self.host_sample_rate
        } else {
            self.host_sample_rate
        }
    }

    fn get_engine_params(&self) -> EngineParams {
        let voices = self.voices_handler.get_ui_state();

        EngineParams {
            num_voices: voices.num_voices,
            legato: voices.legato,
            block_size: self.block_size,
            oversampling: self.oversampling,
            stereo_spectrum: self.spectrum_channels == NUM_CHANNELS,
            voice_kill_time: self.get_voice_kill_time(),
            output_gain: self.get_output_gain(),
        }
    }

    fn get_routing_state(&self) -> ui_bridge::RoutingState {
        ui_bridge::RoutingState::new(
            self.modules
                .values()
                .filter_map(|m| m.as_deref())
                .filter(|m| !matches!(m.module_type(), ModuleType::Output))
                .map(|m| (m.id(), ui_bridge::routing_state::Module::new(m)))
                .collect(),
            self.input_sources.clone(),
        )
    }

    pub fn set_num_voices(&mut self, num_voices: usize) {
        self.voices_handler
            .set_num_voices(Self::clamp_num_voices(num_voices));
    }

    pub fn set_legato(&mut self, legato: bool) {
        self.voices_handler.set_legato(legato);
    }

    pub fn block_size(&self) -> usize {
        self.block_size
    }

    pub fn set_block_size(&mut self, block_size: usize) {
        self.block_size = Self::clamp_block_size(block_size);
    }

    pub fn set_voice_kill_time(&mut self, voice_kill_time: Sample) {
        if let Some(output) = self
            .modules
            .get_typed_module_mut::<Output>(OUTPUT_MODULE_ID)
        {
            output.set_voice_kill_time(voice_kill_time);
        }
    }

    pub fn set_oversampling(&mut self, oversampling: bool) {
        self.oversampling = oversampling;
    }

    pub fn set_stereo_spectrum(&mut self, stereo_spectrum: bool) {
        self.spectrum_channels = Self::stereo_spectrum_channels(stereo_spectrum);
    }

    pub fn get_output_gain(&self) -> StereoSample {
        self.modules
            .get_typed_module::<Output>(OUTPUT_MODULE_ID)
            .map_or(StereoSample::ZERO, |output| output.get_gain())
    }

    pub fn get_voice_kill_time(&self) -> Sample {
        self.modules
            .get_typed_module::<Output>(OUTPUT_MODULE_ID)
            .map_or(0.0, |output| output.get_voice_kill_time())
    }

    pub fn set_output_gain(&mut self, level: StereoSample) {
        if let Some(output) = self
            .modules
            .get_typed_module_mut::<Output>(OUTPUT_MODULE_ID)
        {
            output.set_gain(level);
        }
    }

    fn clamp_num_voices(num_voices: usize) -> usize {
        num_voices.clamp(1, Self::AVAILABLE_VOICES)
    }

    fn clamp_block_size(block_size: usize) -> usize {
        (block_size).clamp(4, MAX_BLOCK_SIZE)
    }

    add_module_method!(add_oscillator, Oscillator);
    add_module_method!(add_envelope, Envelope);
    add_module_method!(add_lfo, Lfo);
    add_module_method!(add_amplifier, Amplifier);
    add_module_method!(add_mixer, Mixer);
    add_module_method!(add_wave_shaper, WaveShaper);
    add_module_method!(add_spectral_filter, SpectralFilter);
    add_module_method!(add_spectral_blend, SpectralBlend);
    add_module_method!(add_spectral_mixer, SpectralMixer);
    add_module_method!(add_harmonic_editor, HarmonicEditor);
    add_module_method!(add_expressions, Expressions);
    add_module_method!(add_external_param, ExternalParam, get_external_params);

    fn get_external_params(&self) -> Arc<ExternalParamsBlock> {
        Arc::clone(self.external_params.as_ref().unwrap())
    }

    pub fn remove_module(&mut self, id: ModuleId) {
        if !self.modules.contains_key(&id) {
            return;
        };

        self.modules.remove(&id);

        let new_links: Vec<_> = self
            .get_links()
            .into_iter()
            .filter(|link| !(link.src == id || link.dst.module_id == id))
            .collect();

        self.setup_routing(&new_links).unwrap();
    }

    fn add_config_link(&mut self, link: &LinkConfig) -> bool {
        let src = link.src_id;
        let dst = ModuleInput::new(link.dst_input, link.dst_id);

        if self.can_be_linked(&src, &dst).is_err() {
            return false;
        }

        if self.already_linked(&src, &dst) {
            return true;
        }

        let mut new_links = self.get_links();

        new_links.push(ModuleLink {
            src,
            dst,
            amount: link.amount,
            modulation: link.modulator_id,
        });

        self.setup_routing(&new_links).is_ok()
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
        Ok(())
    }

    pub fn add_link(
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

        new_links.push(ModuleLink::scaled(src, dst, amount));
        self.setup_routing(&new_links)?;
        Ok(())
    }

    pub fn update_link_amount(&mut self, src: &ModuleId, dst: &ModuleInput, amount: StereoSample) {
        if let Some(inputs) = self.input_sources.get_mut(dst)
            && let Some(input) = inputs.iter_mut().find(|input| input.module_id == *src)
        {
            input.amount = amount;
        }
    }

    pub fn set_link_modulation(
        &mut self,
        src_id: ModuleId,
        dst_input: &ModuleInput,
        modulator_id: ModuleId,
    ) -> Result<(), String> {
        self.can_be_linked(&modulator_id, dst_input)?;

        if let Some(sources) = self.input_sources.get_mut(dst_input)
            && let Some(source) = sources.iter_mut().find(|src| src.module_id == src_id)
        {
            source.modulation = Some(modulator_id);
            self.setup_routing(&self.get_links())?;

            Ok(())
        } else {
            Err("Invalid node.".to_string())
        }
    }

    pub fn remove_link_modulation(&mut self, src_id: ModuleId, dst_input: &ModuleInput) {
        if let Some(sources) = self.input_sources.get_mut(dst_input)
            && let Some(source) = sources.iter_mut().find(|src| src.module_id == src_id)
        {
            source.modulation = None;
            self.setup_routing(&self.get_links()).unwrap();
        }
    }

    pub fn remove_link(&mut self, src: &ModuleId, dst: &ModuleInput) {
        let new_links: Vec<_> = self
            .get_links()
            .into_iter()
            .filter(|link| !(link.src == *src && link.dst == *dst))
            .collect();

        self.setup_routing(&new_links).unwrap();
    }

    fn process_voice_events(&mut self, events: &[VoiceEvent]) {
        for module_id in &self.execution_order {
            if let Some(module) = self.modules.get_module_mut(*module_id) {
                module.handle_events(events);
            }
        }
    }

    pub fn handle_note_on(&mut self, channel: u8, note: u8, velocity: f32) {
        let mut voice_events = VoiceEvents::new();

        self.voices_handler
            .handle_note_on(channel, note, velocity, &mut voice_events);

        self.process_voice_events(voice_events.events());
    }

    pub fn handle_note_off(&mut self, channel: u8, note: u8, velocity: f32) {
        let mut voice_events = VoiceEvents::new();

        self.voices_handler
            .handle_note_off(channel, note, velocity, &mut voice_events);

        self.process_voice_events(voice_events.events());
    }

    pub fn handle_note_expression(
        &mut self,
        channel: u8,
        note: u8,
        expression: Expression,
        value: Sample,
    ) {
        let mut voice_events = VoiceEvents::new();

        self.voices_handler
            .handle_expression(channel, note, expression, value, &mut voice_events);

        self.process_voice_events(voice_events.events());
    }

    pub fn handle_choke(&mut self, channel: u8, note: u8) {
        self.voices_handler.handle_choke(channel, note);
    }

    fn handle_ui_events(&mut self) {
        use ui_bridge::UiEvent;

        while let Some(event) = self.audio_end.pop_event() {
            match event {
                UiEvent::LinkAmount { src, dst, amount } => {
                    self.update_link_amount(&src, &dst, amount);
                }
                UiEvent::Voices(voices) => self.set_num_voices(voices),
                UiEvent::Legato(legato) => self.set_legato(legato),
                UiEvent::BlockSize(block_size) => self.set_block_size(block_size),
                UiEvent::VoiceKillTime(voice_kill_time) => {
                    self.set_voice_kill_time(voice_kill_time);
                }
                UiEvent::Oversampling(oversampling) => self.set_oversampling(oversampling),
                UiEvent::StereoSpectrum(stereo_spectrum) => {
                    self.set_stereo_spectrum(stereo_spectrum);
                }
                UiEvent::OutputGain(output_gain) => self.set_output_gain(output_gain),
            }
        }

        self.modules
            .values_mut()
            .filter_map(|m| m.as_deref_mut())
            .for_each(|m| m.handle_ui_events());
    }

    pub fn process<'a>(
        &mut self,
        samples: usize,
        update_ui: bool,
        outputs: impl Iterator<Item = &'a mut [f32]>,
    ) {
        self.handle_ui_events();

        {
            let mut decaying_voices = DecayingVoices::new();

            self.voices_handler
                .get_decaying_voices(&mut decaying_voices);

            self.execution_order
                .iter()
                .filter_map(|id| self.modules.get_module(*id))
                .for_each(|module| module.poll_decaying_voices(&mut decaying_voices));

            self.voices_handler.update_decaying_voices(&decaying_voices);
        }

        if update_ui {
            self.audio_end
                .update_voices_status(&self.voices_handler.get_ui_state());
        }

        let mut playing_voices = PlayingVoices::new();

        self.voices_handler.get_playing_voices(&mut playing_voices);

        let samples = if self.oversampling {
            2 * samples
        } else {
            samples
        };
        let sample_rate = self.sample_rate();

        let params = ProcessParams {
            samples,
            sample_rate,
            buffer_t_step: samples as Sample / sample_rate,
            smooth_params: SmoothedSampleParams::new(sample_rate),
            needs_update_ui: update_ui,
            spectrum_channels: self.spectrum_channels,
            active_voices: &playing_voices,
        };

        for i in 0..self.execution_order.len() {
            let module_id = self.execution_order[i];

            if let Some(module_box) = self.modules.get_mut(&module_id)
                && let Some(mut module) = module_box.take()
            {
                module.process(&params, self);
                self.modules.get_mut(&module_id).unwrap().replace(module);
            }
        }

        if let Some(output) = self
            .modules
            .get_typed_module_mut::<Output>(OUTPUT_MODULE_ID)
        {
            output.read_output(self.oversampling, outputs);
        }
    }

    fn alloc_module_id(&mut self) -> ModuleId {
        let module_id = self.next_id;

        self.next_id += 1;
        module_id
    }

    fn can_be_linked_with_output(&self, src: &ModuleId, dst: &ModuleInput) -> Result<(), String> {
        let Some(src_module) = self.modules.get_module(*src) else {
            return Err("Invalid node.".to_string());
        };

        let is_compatible = dst.input_type == Input::Audio
            && data_types_compatible(src_module.output(), DataType::Audio);

        if !is_compatible {
            return Err("Data types mismatch.".to_string());
        }

        Ok(())
    }

    fn can_be_linked(&self, src: &ModuleId, dst: &ModuleInput) -> Result<(), String> {
        if dst.module_id == OUTPUT_MODULE_ID {
            return self.can_be_linked_with_output(src, dst);
        }

        let (Some(src_module), Some(dst_module)) = (
            self.modules.get_module(*src),
            self.modules.get_module(dst.module_id),
        ) else {
            return Err("Invalid node.".to_string());
        };

        let src_data_types = src_module.output();

        let is_compatible = dst_module.inputs().iter().any(|input_info| {
            input_info.input == dst.input_type
                && data_types_compatible(src_data_types, input_info.data_type)
        });

        if !is_compatible {
            return Err("Data types mismatch.".to_string());
        }

        Ok(())
    }

    fn already_linked(&self, src: &ModuleId, dst: &ModuleInput) -> bool {
        if let Some(inputs) = self.input_sources.get(dst) {
            inputs.iter().any(|input| input.module_id == *src)
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
                    src: src.module_id,
                    amount: src.amount,
                    modulation: src.modulation,
                })
            })
            .collect()
    }

    // pub fn get_module(&self, id: ModuleId) -> Option<&dyn SynthModule> {
    //     self.modules.get_module(id)
    // }

    pub fn get_typed_module<T: SynthModule>(&self, id: ModuleId) -> Option<&T> {
        self.modules.get_typed_module(id)
    }

    // pub fn get_module_mut(&mut self, id: ModuleId) -> Option<&mut dyn SynthModule> {
    //     self.modules.get_module_mut(id)
    // }

    pub fn get_typed_module_mut<T: SynthModule>(&mut self, id: ModuleId) -> Option<&mut T> {
        self.modules.get_typed_module_mut(id)
    }

    fn calc_execution_order(links: &[ModuleLink]) -> Result<Vec<ModuleId>, String> {
        let mut dependents: HashMap<ModuleId, HashSet<ModuleId>> = HashMap::new();

        for link in links {
            let src_node = link.src;
            let dst_node = link.dst.module_id;

            dependents.entry(dst_node).or_default().insert(src_node);
            dependents.entry(src_node).or_default();

            if let Some(modulation) = link.modulation {
                dependents.entry(dst_node).or_default().insert(modulation);
                dependents.entry(modulation).or_default();
            }
        }

        let topo_sort = TopoSort::from_map(dependents);

        match topo_sort.into_vec_nodes() {
            SortResults::Full(nodes) => {
                let mut order: Vec<_> = nodes
                    .into_iter()
                    .filter(|id| *id != OUTPUT_MODULE_ID)
                    .collect();

                order.push(OUTPUT_MODULE_ID);
                Ok(order)
            }
            SortResults::Partial(_) => Err("Cycles detected!".to_string()),
        }
    }

    fn setup_slots(&mut self) {
        struct ModuleSlots {
            data_type: DataType,
            output_slot: usize,
            inputs: Vec<InputSlots>,
            spectral_inputs: Vec<SpectralInputSlot>,
        }

        let mut samples_slots = 0;
        let mut spectral_slots = 0;

        let mut modules_slots: FxHashMap<_, _> = self
            .modules
            .iter()
            .filter_map(|(&mod_id, m)| {
                m.as_deref().map(|m| {
                    (
                        mod_id,
                        ModuleSlots {
                            data_type: m.output(),
                            output_slot: 0,
                            inputs: Default::default(),
                            spectral_inputs: Default::default(),
                        },
                    )
                })
            })
            .collect();

        for mod_id in self.execution_order.iter() {
            let mod_slots = modules_slots
                .get_mut(mod_id)
                .expect("module should be in place");

            match mod_slots.data_type {
                DataType::Audio | DataType::Control => {
                    mod_slots.output_slot = samples_slots;
                    samples_slots += 1;
                }

                DataType::Spectral => {
                    mod_slots.output_slot = spectral_slots;
                    spectral_slots += 1;
                }
            }
        }

        self.outputs_arena
            .set_num_slots(samples_slots, spectral_slots);

        for (input, sources) in self.input_sources.iter() {
            if sources.len() == 1
                && modules_slots
                    .get(&sources[0].module_id)
                    .expect("should be in place")
                    .data_type
                    == DataType::Spectral
            {
                let src_output_slot = modules_slots
                    .get(&sources[0].module_id)
                    .expect("should be in place")
                    .output_slot;

                let dst_module = modules_slots
                    .get_mut(&input.module_id)
                    .expect("should be in place");

                dst_module.spectral_inputs.push(SpectralInputSlot {
                    input_type: input.input_type,
                    slot: src_output_slot,
                });

                continue;
            }

            let mut input_slots = InputSlots {
                input_type: input.input_type,
                slots: Vec::new(),
            };

            for src in sources {
                let mut input_src = SamplesInputSrc {
                    src_slot: 0,
                    modulation_slot: None,
                    amount: src.amount,
                };

                let src_module = modules_slots
                    .get(&src.module_id)
                    .expect("should be in place");

                assert_matches!(src_module.data_type, DataType::Audio | DataType::Control);

                input_src.src_slot = src_module.output_slot;

                if let Some(modulation_src) = src.modulation {
                    let modulation_module = modules_slots
                        .get(&modulation_src)
                        .expect("should be in place");

                    assert_matches!(
                        modulation_module.data_type,
                        DataType::Audio | DataType::Control
                    );

                    input_src.modulation_slot = Some(modulation_module.output_slot);
                }

                input_slots.slots.push(input_src);
            }

            for (module_id, mod_slots) in modules_slots.iter() {
                let module = self
                    .modules
                    .get_mut(module_id)
                    .and_then(|m| m.as_deref_mut())
                    .expect("module should be in place");

                module.set_slots(
                    &mod_slots.inputs,
                    &mod_slots.spectral_inputs,
                    mod_slots.output_slot,
                );
            }
        }
    }

    fn setup_routing(&mut self, links: &[ModuleLink]) -> Result<(), String> {
        let execution_order = Self::calc_execution_order(links)?;
        let mut input_sources: FxHashMap<ModuleInput, Vec<ModuleInputSource>> =
            FxHashMap::default();

        for link in links {
            input_sources
                .entry(link.dst)
                .or_default()
                .push(ModuleInputSource {
                    module_id: link.src,
                    amount: link.amount,
                    modulation: link.modulation,
                });
        }

        self.input_sources = input_sources;
        self.execution_order = execution_order;
        self.setup_slots();
        Ok(())
    }

    fn stereo_spectrum_channels(stereo_spectrum: bool) -> usize {
        if stereo_spectrum { NUM_CHANNELS } else { 1 }
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

        if sources.len() == 1
            && let Some(first) = sources.first()
            && first.amount == StereoSample::ONE
            && first.modulation.is_none()
            && let Some(module) = self.modules.get_module(first.module_id)
        {
            return Some(module.get_buffer_output(voice_idx, channel_idx));
        }

        if sources.is_empty() {
            return None;
        }

        let result = &mut input_buffer[..samples];

        let modules = sources.iter().filter_map(|source| {
            self.modules
                .get_module(source.module_id)
                .map(|module| (module, source.amount, source.modulation))
        });

        for (mod_idx, (module, amount, modulation)) in modules.enumerate() {
            let amount = amount[channel_idx];
            let input = module
                .get_buffer_output(voice_idx, channel_idx)
                .iter()
                .map(|sample| sample * amount);

            if let Some(modulation) = modulation
                && let Some(module) = self.modules.get_module(modulation)
            {
                let input_mod = module.get_buffer_output(voice_idx, channel_idx).iter();

                copy_or_add_to_buffer(
                    mod_idx == 0,
                    result,
                    input
                        .zip(input_mod)
                        .map(|(input, input_mod)| input * input_mod),
                );
            } else {
                copy_or_add_to_buffer(mod_idx == 0, result, input);
            }
        }

        Some(input_buffer)
    }

    fn add_input_to(
        &self,
        input: ModuleInput,
        voice_idx: usize,
        channel_idx: usize,
        result: &mut [Sample],
    ) -> bool {
        let Some(sources) = self.input_sources.get(&input) else {
            return false;
        };

        let modules = sources.iter().filter_map(|source| {
            self.modules
                .get_module(source.module_id)
                .map(|module| (module, source.amount, source.modulation))
        });

        for (module, amount, modulation) in modules {
            let amount = amount[channel_idx];
            let input = module
                .get_buffer_output(voice_idx, channel_idx)
                .iter()
                .map(|sample| sample * amount);

            if let Some(modulation) = modulation
                && let Some(module) = self.modules.get_module(modulation)
            {
                let input_mod = module.get_buffer_output(voice_idx, channel_idx).iter();

                add_to_buffer(
                    result,
                    input
                        .zip(input_mod)
                        .map(|(input, input_mod)| input * input_mod),
                );
            } else {
                add_to_buffer(result, input);
            }
        }

        true
    }

    fn read_unmodulated_input(
        &self,
        input: ModuleInput,
        samples: usize,
        voice_idx: usize,
        channel_idx: usize,
        input_buffer: &mut Buffer,
    ) {
        let result = &mut input_buffer[..samples];

        if let Some(sources) = self.input_sources.get(&input)
            && !sources.is_empty()
        {
            let modules = sources
                .iter()
                .filter_map(|source| self.modules.get_module(source.module_id));

            for (mod_idx, module) in modules.enumerate() {
                let pairs = result
                    .iter_mut()
                    .zip(module.get_buffer_output(voice_idx, channel_idx));

                if mod_idx == 0 {
                    pairs.for_each(|(out, sample)| *out = *sample);
                } else {
                    pairs.for_each(|(out, sample)| *out += *sample);
                }
            }
        } else {
            result.fill(0.0);
        }
    }

    fn get_spectral_input(
        &self,
        input: ModuleInput,
        current: bool,
        voice_idx: usize,
        channel_idx: usize,
    ) -> Option<&SpectralBuffer> {
        let sources = self.input_sources.get(&input)?;

        if let Some(first) = sources.first()
            && let Some(module) = self.modules.get_module(first.module_id)
        {
            Some(module.get_spectral_output(current, voice_idx, channel_idx))
        } else {
            None
        }
    }

    fn get_scalar_input(
        &self,
        input: ModuleInput,
        current: bool,
        voice_idx: usize,
        channel_idx: usize,
    ) -> Option<Sample> {
        let sources = self.input_sources.get(&input)?;

        if sources.is_empty() {
            return None;
        }

        let mut output: Sample = 0.0;

        let values = sources.iter().filter_map(|source| {
            self.modules.get_module(source.module_id).map(|module| {
                (
                    module.get_scalar_output(current, voice_idx, channel_idx),
                    source.amount,
                    source.modulation,
                )
            })
        });

        for (value, amount, modulation) in values {
            let mut input = value * amount[channel_idx];

            if let Some(modulation) = modulation
                && let Some(module) = self.modules.get_module(modulation)
            {
                input *= module.get_scalar_output(current, voice_idx, channel_idx);
            }

            output += input;
        }

        Some(output)
    }

    fn update_modulated_input(
        &mut self,
        module_id: ModuleId,
        input: Input,
        channel_idx: usize,
        value: Sample,
    ) {
        self.audio_end
            .update_modulated_input(module_id, input, channel_idx as u8, value);
    }

    fn update_output(&mut self, module_id: ModuleId, channel_idx: usize, value: Sample) {
        self.audio_end
            .update_output(module_id, channel_idx as u8, value);
    }
}
