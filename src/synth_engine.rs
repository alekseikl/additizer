use core::f32;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use nih_plug::params::FloatParam;
use rustc_hash::FxHashMap;
use std::assert_matches;
use topo_sort::{SortResults, TopoSort};

use crate::synth_engine::{
    level_ballistics::StereoLevelBallistics,
    module_handle::ModuleHandle,
    modules::Output,
    routing::{
        InputMeta, InputSlot, InputSlots, MIN_MODULE_ID, MixedSource, ModuleLink, OutputsArena,
        ProcessContext, ProcessParams, SpectralInputSlot, data_types_compatible,
    },
    synth_module::SynthModule,
    voices_handler::{
        DecayingVoices, MAX_AVAILABLE_VOICES, PlayingVoices, VoiceEvents, VoicesHandler,
    },
};

pub use buffer::{Buffer, HARMONIC_SERIES_BUFFER, SPECTRAL_BUFFER_SIZE, SpectralBuffer};
pub use config::{EngineConfig, EngineParams, LinkConfig, MAX_BANDWIDTH, ModuleConfig};
pub use module_handle::ModuleType;
pub use modules::{
    Amplifier, Envelope, Expressions, ExternalParam, ExternalParamsBlock, FilterType, Lfo,
    LfoShape, Mixer, Oscillator, ShaperType, SpectralBlend, SpectralFilter, SpectralMixer,
    WaveShaper,
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
    DataType, Expression, Input, InputId, InputSource, MixType, ModuleId, NUM_CHANNELS,
    OUTPUT_MODULE_ID, VoiceEvent, VolumeType,
};
pub use smooth::{SmoothedSampleParams, Smoother};
pub use stereo_sample::StereoSample;
pub use synth_module::ModuleUiBridge;
pub use types::{ComplexSample, Sample};

mod buffer;
mod config;
#[macro_use]
mod synth_module;
pub mod biquad_filter;
mod curves;
pub mod filters;
mod iir_decimator;
mod level_ballistics;
mod module_handle;
mod modules;
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

type ModulesMap = FxHashMap<ModuleId, ModuleHandle>;
type RoutingMap = FxHashMap<InputId, InputSource>;

pub struct SynthEngine {
    next_id: ModuleId,
    host_sample_rate: f32,
    block_size: usize,
    oversampling: bool,
    bandwidth: usize,
    spectrum_channels: usize,
    modules: ModulesMap,
    input_sources: RoutingMap,
    execution_order: Vec<ModuleId>,
    voices_handler: VoicesHandler,
    external_params: Option<Arc<ExternalParamsBlock>>,
    audio_end: ui_bridge::AudioEnd,
    ui_end: Option<ui_bridge::UiEnd>,
    outputs_arena: OutputsArena,
    out_volume_ballistics: StereoLevelBallistics,
}

macro_rules! add_module_method {
    ($func_name:ident, $module_type:ident $(, $arg:ident )*) => {
        pub fn $func_name(&mut self) -> ModuleId {
            let id = self.alloc_module_id();
            let mut module = ModuleHandle::$module_type(Box::new($module_type::new(id $(, self.$arg() )*)));

            self.outputs_arena.allocate_slot(&mut module);
            self.modules.insert(id, module);
            self.execution_order.push(id);
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
            bandwidth: Self::clamp_bandwidth(cfg.engine.bandwidth),
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
            out_volume_ballistics: StereoLevelBallistics::default(),
        };

        engine.modules.insert(
            OUTPUT_MODULE_ID,
            ModuleHandle::Output(Box::new(Output::new(
                cfg.engine.output_gain,
                cfg.engine.voice_kill_time,
                output_level_param,
            ))),
        );

        let mut max_module_id = MIN_MODULE_ID;

        for module_cfg in cfg.modules.iter() {
            let mut module = match module_cfg {
                ModuleConfig::Oscillator(cfg) => {
                    ModuleHandle::Oscillator(Box::new(Oscillator::from_config(cfg)))
                }
                ModuleConfig::Envelope(cfg) => {
                    ModuleHandle::Envelope(Box::new(Envelope::from_config(cfg)))
                }
                ModuleConfig::Lfo(cfg) => ModuleHandle::Lfo(Box::new(Lfo::from_config(cfg))),
                ModuleConfig::Amplifier(cfg) => {
                    ModuleHandle::Amplifier(Box::new(Amplifier::from_config(cfg)))
                }
                ModuleConfig::Mixer(cfg) => ModuleHandle::Mixer(Box::new(Mixer::from_config(cfg))),
                ModuleConfig::WaveShaper(cfg) => {
                    ModuleHandle::WaveShaper(Box::new(WaveShaper::from_config(cfg)))
                }
                ModuleConfig::SpectralFilter(cfg) => {
                    ModuleHandle::SpectralFilter(Box::new(SpectralFilter::from_config(cfg)))
                }
                ModuleConfig::SpectralBlend(cfg) => {
                    ModuleHandle::SpectralBlend(Box::new(SpectralBlend::from_config(cfg)))
                }
                ModuleConfig::SpectralMixer(cfg) => {
                    ModuleHandle::SpectralMixer(Box::new(SpectralMixer::from_config(cfg)))
                }
                ModuleConfig::HarmonicEditor(cfg) => {
                    ModuleHandle::HarmonicEditor(Box::new(HarmonicEditor::from_config(cfg)))
                }
                ModuleConfig::Expressions(cfg) => {
                    ModuleHandle::Expressions(Box::new(Expressions::from_config(cfg)))
                }
                ModuleConfig::ExternalParam(cfg) => ModuleHandle::ExternalParam(Box::new(
                    ExternalParam::from_config(cfg, external_params.clone()),
                )),
            };

            let module_id = module.id();

            if module_id < MIN_MODULE_ID || engine.modules.contains_key(&module_id) {
                return None;
            }

            if module_id > max_module_id {
                max_module_id = module_id;
            }

            engine.outputs_arena.allocate_slot(&mut module);
            engine.modules.insert(module_id, module);
        }

        engine.next_id = max_module_id + 1;

        if !engine.set_config_links(&cfg.links) {
            return None;
        }

        Some(engine)
    }

    pub fn get_config(&self) -> EngineConfig {
        let mut module_ids: Vec<_> = self.modules.keys().copied().collect();

        module_ids.sort_unstable();

        let modules = module_ids
            .iter()
            .filter_map(|&id| {
                let module = self.modules.get(&id)?;
                match module {
                    ModuleHandle::Output(_) => None,
                    ModuleHandle::Oscillator(m) => {
                        Some(ModuleConfig::Oscillator(Box::new(m.get_config())))
                    }
                    ModuleHandle::Envelope(m) => {
                        Some(ModuleConfig::Envelope(Box::new(m.get_config())))
                    }
                    ModuleHandle::Lfo(m) => Some(ModuleConfig::Lfo(Box::new(m.get_config()))),
                    ModuleHandle::Amplifier(m) => {
                        Some(ModuleConfig::Amplifier(Box::new(m.get_config())))
                    }
                    ModuleHandle::Mixer(m) => Some(ModuleConfig::Mixer(Box::new(m.get_config()))),
                    ModuleHandle::WaveShaper(m) => {
                        Some(ModuleConfig::WaveShaper(Box::new(m.get_config())))
                    }
                    ModuleHandle::SpectralFilter(m) => {
                        Some(ModuleConfig::SpectralFilter(Box::new(m.get_config())))
                    }
                    ModuleHandle::SpectralBlend(m) => {
                        Some(ModuleConfig::SpectralBlend(Box::new(m.get_config())))
                    }
                    ModuleHandle::SpectralMixer(m) => {
                        Some(ModuleConfig::SpectralMixer(Box::new(m.get_config())))
                    }
                    ModuleHandle::HarmonicEditor(m) => {
                        Some(ModuleConfig::HarmonicEditor(Box::new(m.get_config())))
                    }
                    ModuleHandle::Expressions(m) => {
                        Some(ModuleConfig::Expressions(Box::new(m.get_config())))
                    }
                    ModuleHandle::ExternalParam(m) => {
                        Some(ModuleConfig::ExternalParam(Box::new(m.get_config())))
                    }
                }
            })
            .collect();

        EngineConfig {
            engine: self.get_engine_params(),
            modules,
            links: self
                .get_links()
                .into_iter()
                .map(|link| link.config())
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
            bandwidth: self.bandwidth,
        }
    }

    fn get_routing_state(&self) -> ui_bridge::RoutingState {
        ui_bridge::RoutingState::new(
            self.modules
                .values()
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
        if let Some(ModuleHandle::Output(output)) = self.modules.get_mut(&OUTPUT_MODULE_ID) {
            output.set_voice_kill_time(voice_kill_time);
        }
    }

    pub fn set_oversampling(&mut self, oversampling: bool) {
        self.oversampling = oversampling;
    }

    pub fn set_stereo_spectrum(&mut self, stereo_spectrum: bool) {
        self.spectrum_channels = Self::stereo_spectrum_channels(stereo_spectrum);
    }

    pub fn set_bandwidth(&mut self, bandwidth: usize) {
        self.bandwidth = Self::clamp_bandwidth(bandwidth);
    }

    pub fn get_output_gain(&self) -> StereoSample {
        match self.modules.get(&OUTPUT_MODULE_ID) {
            Some(ModuleHandle::Output(output)) => output.get_gain(),
            _ => StereoSample::ZERO,
        }
    }

    pub fn get_voice_kill_time(&self) -> Sample {
        match self.modules.get(&OUTPUT_MODULE_ID) {
            Some(ModuleHandle::Output(output)) => output.get_voice_kill_time(),
            _ => 0.0,
        }
    }

    pub fn set_output_gain(&mut self, level: StereoSample) {
        if let Some(ModuleHandle::Output(output)) = self.modules.get_mut(&OUTPUT_MODULE_ID) {
            output.set_gain(level);
        }
    }

    fn clamp_num_voices(num_voices: usize) -> usize {
        num_voices.clamp(1, Self::AVAILABLE_VOICES)
    }

    fn clamp_block_size(block_size: usize) -> usize {
        (block_size).clamp(4, MAX_BLOCK_SIZE)
    }

    fn clamp_bandwidth(bandwidth: usize) -> usize {
        bandwidth.clamp(0, MAX_BANDWIDTH)
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
        let Some(module) = self.modules.get(&id) else {
            return;
        };

        self.outputs_arena.free_slot(module);
        self.modules.remove(&id);

        let new_links: Vec<_> = self
            .get_links()
            .into_iter()
            .filter(|link| link.src() != id && link.dst().module_id != id)
            .map(|mut link| {
                if link.modulation() == Some(id) {
                    link.clear_modulation();
                }
                link
            })
            .collect();

        self.setup_routing(&new_links)
            .expect("routing should be consistent after a module is removed");
    }

    fn set_config_links(&mut self, links: &[LinkConfig]) -> bool {
        let mut new_links = self.get_links();

        for link in links.iter() {
            let src = link.src_id();
            let dst = InputId::new(link.dst_input(), link.dst_id());

            if self.can_be_linked(&src, &dst).is_err()
                || link
                    .modulator_id()
                    .is_some_and(|id| self.can_be_linked(&id, &dst).is_err())
                || self
                    .input_meta_for(&dst)
                    .is_none_or(|meta| matches!(link, LinkConfig::Direct { .. }) != meta.is_direct)
            {
                return false;
            }

            if new_links
                .iter()
                .any(|existing| existing.src() == src && existing.dst() == dst)
            {
                continue;
            }

            new_links.push(ModuleLink::from_config(link));
        }

        self.setup_routing(&new_links).is_ok()
    }

    pub fn set_direct_link(&mut self, src: ModuleId, dst: InputId) -> Result<(), String> {
        self.can_be_linked(&src, &dst)?;

        let Some(meta) = self.input_meta_for(&dst) else {
            return Err("Invalid destination input.".to_string());
        };

        if !meta.is_direct {
            return Err("Mixed inputs require add_mixed_link.".to_string());
        }

        let mut new_links: Vec<_> = self
            .get_links()
            .iter()
            .filter(|link| link.dst() != dst)
            .copied()
            .collect();

        new_links.push(ModuleLink::direct(src, dst));
        self.setup_routing(&new_links)?;
        Ok(())
    }

    pub fn add_mixed_link(
        &mut self,
        src: ModuleId,
        dst: InputId,
        amount: StereoSample,
    ) -> Result<(), String> {
        self.can_be_linked(&src, &dst)?;

        let Some(meta) = self.input_meta_for(&dst) else {
            return Err("Invalid destination input.".to_string());
        };

        if meta.is_direct {
            return Err("Direct inputs require set_direct_link.".to_string());
        }

        let mut new_links = self.get_links();

        // Disconnect src from modulations on this destination
        for link in &mut new_links {
            if link.dst() == dst && link.modulation() == Some(src) {
                link.clear_modulation();
            }
        }

        new_links.retain(|link| !(link.src() == src && link.dst() == dst));
        new_links.push(ModuleLink::mixed(src, dst, amount));

        self.setup_routing(&new_links)?;
        Ok(())
    }

    pub fn update_link_amount(&mut self, src: &ModuleId, dst: &InputId, amount: StereoSample) {
        if let Some(inputs) = self.input_sources.get_mut(dst)
            && inputs.update_amount(*src, amount)
            && let Some(src_slot) = self.modules.get(src).map(|m| m.output_slot())
            && let Some(dst_module) = self.modules.get_mut(&dst.module_id)
        {
            dst_module.update_input_amount(dst.input_type, src_slot, amount);
        }
    }

    pub fn set_link_modulation(
        &mut self,
        src_id: ModuleId,
        dst_input: &InputId,
        modulator_id: ModuleId,
    ) -> Result<(), String> {
        self.can_be_linked(&modulator_id, dst_input)?;

        if !self.already_linked(&src_id, dst_input) {
            return Err("Invalid node.".to_string());
        }

        let mut new_links = self.get_links();
        let link = new_links
            .iter_mut()
            .find(|link| link.src() == src_id && link.dst() == *dst_input)
            .expect("link checked above");

        if !link.set_modulation(modulator_id) {
            return Err("Direct links cannot be modulated.".to_string());
        }

        self.setup_routing(&new_links)?;
        Ok(())
    }

    pub fn remove_link_modulation(&mut self, src_id: ModuleId, dst_input: &InputId) {
        if let Some(sources) = self.input_sources.get_mut(dst_input)
            && sources.clear_modulation(src_id)
        {
            self.setup_routing(&self.get_links())
                .expect("routing should be consistent after a modulation is removed");
        }
    }

    pub fn remove_link(&mut self, src: &ModuleId, dst: &InputId) {
        let new_links: Vec<_> = self
            .get_links()
            .into_iter()
            .filter(|link| !(link.src() == *src && link.dst() == *dst))
            .collect();

        self.setup_routing(&new_links)
            .expect("routing should be consistent after a link is removed");
    }

    pub fn remove_input_links(&mut self, dst: &InputId) {
        let new_links: Vec<_> = self
            .get_links()
            .into_iter()
            .filter(|link| link.dst() != *dst)
            .collect();

        self.setup_routing(&new_links)
            .expect("routing should be consistent after links are removed");
    }

    pub fn remove_output_links(&mut self, src: ModuleId) {
        let new_links: Vec<_> = self
            .get_links()
            .into_iter()
            .filter(|link| link.src() != src)
            .map(|mut link| {
                if link.modulation() == Some(src) {
                    link.clear_modulation();
                }
                link
            })
            .collect();

        self.setup_routing(&new_links)
            .expect("routing should be consistent after links are removed");
    }

    fn process_voice_events(&mut self, events: &[VoiceEvent]) {
        for module_id in &self.execution_order {
            if let Some(module) = self.modules.get_mut(module_id) {
                module.process_events(events);
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
                UiEvent::Bandwidth(bandwidth) => self.set_bandwidth(bandwidth),
                UiEvent::OutputGain(output_gain) => self.set_output_gain(output_gain),
            }
        }

        self.modules
            .values_mut()
            .for_each(|m| m.process_ui_events());
    }

    pub fn process(&mut self, samples: usize, update_ui: bool, outputs: &mut [&mut [f32]]) {
        self.handle_ui_events();

        {
            let mut decaying_voices = DecayingVoices::new();

            self.voices_handler
                .get_decaying_voices(&mut decaying_voices);

            self.execution_order
                .iter()
                .filter_map(|id| self.modules.get(id))
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

        let mut ctx = ProcessContext {
            outputs_arena: &mut self.outputs_arena,
            audio_end: &mut self.audio_end,
            params: ProcessParams {
                samples,
                sample_rate,
                smooth_params: SmoothedSampleParams::new(sample_rate),
                needs_update_ui: update_ui,
                spectrum_channels: self.spectrum_channels,
                bandwidth: self.bandwidth,
                active_voices: &playing_voices,
            },
        };

        for module_id in &self.execution_order {
            if let Some(module) = self.modules.get_mut(module_id) {
                module.process(&mut ctx);
            }
        }

        if let Some(ModuleHandle::Output(output)) = self.modules.get_mut(&OUTPUT_MODULE_ID) {
            output.read_output(self.oversampling, outputs);

            if update_ui {
                let (left, right) = outputs.split_at_mut(1);
                let levels = self
                    .out_volume_ballistics
                    .process([left[0], right[0]], self.host_sample_rate);

                self.audio_end
                    .update_out_volume(StereoSample::from_iter(levels));
            }
        }
    }

    fn alloc_module_id(&mut self) -> ModuleId {
        let module_id = self.next_id;

        self.next_id += 1;
        module_id
    }

    fn can_be_linked(&self, src: &ModuleId, dst: &InputId) -> Result<(), String> {
        let (Some(src_module), Some(dst_module)) =
            (self.modules.get(src), self.modules.get(&dst.module_id))
        else {
            return Err("Invalid node.".to_string());
        };

        let src_data_type = src_module.output_type();

        let is_compatible = dst_module.inputs().iter().any(|input_info| {
            input_info.input_type == dst.input_type
                && data_types_compatible(src_data_type, input_info.data_type)
        });

        if !is_compatible {
            return Err("Data types mismatch.".to_string());
        }

        Ok(())
    }

    fn already_linked(&self, src: &ModuleId, dst: &InputId) -> bool {
        self.input_sources
            .get(dst)
            .is_some_and(|inputs| inputs.contains_module(*src))
    }

    fn get_links(&self) -> Vec<ModuleLink> {
        self.input_sources
            .iter()
            .flat_map(|(dst, sources)| sources.links(*dst))
            .collect()
    }

    fn input_meta_for(&self, dst: &InputId) -> Option<InputMeta> {
        self.modules
            .get(&dst.module_id)?
            .inputs()
            .iter()
            .find(|meta| meta.input_type == dst.input_type)
            .copied()
    }

    pub fn get_module(&self, id: ModuleId) -> Option<&ModuleHandle> {
        self.modules.get(&id)
    }

    pub fn get_module_mut(&mut self, id: ModuleId) -> Option<&mut ModuleHandle> {
        self.modules.get_mut(&id)
    }

    fn calc_execution_order(
        links: &[ModuleLink],
        all_modules: impl IntoIterator<Item = ModuleId>,
    ) -> Result<Vec<ModuleId>, String> {
        let mut dependents: HashMap<ModuleId, HashSet<ModuleId>> = HashMap::new();

        for id in all_modules {
            dependents.entry(id).or_default();
        }

        for link in links {
            let src_node = link.src();
            let dst_node = link.dst().module_id;

            dependents.entry(dst_node).or_default().insert(src_node);
            dependents.entry(src_node).or_default();

            if let Some(modulation) = link.modulation() {
                dependents.entry(dst_node).or_default().insert(modulation);
                dependents.entry(modulation).or_default();
            }
        }

        let topo_sort = TopoSort::from_map(dependents);

        match topo_sort.into_vec_nodes() {
            SortResults::Full(nodes) => Ok(nodes),
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

        let mut modules_slots: FxHashMap<_, _> = self
            .modules
            .iter()
            .map(|(&mod_id, m)| {
                (
                    mod_id,
                    ModuleSlots {
                        data_type: m.output_type(),
                        output_slot: m.output_slot(),
                        inputs: Default::default(),
                        spectral_inputs: Default::default(),
                    },
                )
            })
            .collect();

        for (input, sources) in self.input_sources.iter() {
            match sources {
                InputSource::Direct(module_id) => {
                    let src_output_slot = modules_slots
                        .get(module_id)
                        .expect("should be in place")
                        .output_slot;
                    let src_data_type = modules_slots
                        .get(module_id)
                        .expect("should be in place")
                        .data_type;

                    let dst_module = modules_slots
                        .get_mut(&input.module_id)
                        .expect("should be in place");

                    if src_data_type == DataType::Spectral {
                        dst_module.spectral_inputs.push(SpectralInputSlot {
                            input_type: input.input_type,
                            slot: src_output_slot,
                        });
                    } else {
                        assert_matches!(src_data_type, DataType::Audio | DataType::Control);

                        dst_module.inputs.push(InputSlots {
                            input_type: input.input_type,
                            slots: vec![InputSlot {
                                src_slot: src_output_slot,
                                modulation_slot: None,
                                amount: StereoSample::ONE,
                            }],
                        });
                    }
                }
                InputSource::Mixed(mixed) => {
                    let mut input_slots = InputSlots {
                        input_type: input.input_type,
                        slots: Vec::new(),
                    };

                    for src in mixed {
                        let mut input_src = InputSlot {
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

                    let dst_module = modules_slots
                        .get_mut(&input.module_id)
                        .expect("should be in place");

                    dst_module.inputs.push(input_slots);
                }
            }
        }

        for (module_id, mod_slots) in modules_slots.iter() {
            let module = self
                .modules
                .get_mut(module_id)
                .expect("module should be in place");

            module.set_input_slots(&mod_slots.inputs, &mod_slots.spectral_inputs);
        }
    }

    fn setup_routing(&mut self, links: &[ModuleLink]) -> Result<(), String> {
        let execution_order = Self::calc_execution_order(links, self.modules.keys().copied())?;
        let mut routing_map = RoutingMap::default();

        for link in links {
            match link {
                ModuleLink::Direct { src, dst } => {
                    routing_map.insert(*dst, InputSource::Direct(*src));
                }
                ModuleLink::Mixed {
                    src,
                    dst,
                    amount,
                    modulation,
                } => {
                    let InputSource::Mixed(sources) = routing_map
                        .entry(*dst)
                        .or_insert(InputSource::Mixed(Vec::new()))
                    else {
                        return Err("routing error".into());
                    };

                    sources.push(MixedSource {
                        module_id: *src,
                        amount: *amount,
                        modulation: *modulation,
                    });
                }
            }
        }

        self.input_sources = routing_map;
        self.execution_order = execution_order;
        self.setup_slots();
        Ok(())
    }

    fn stereo_spectrum_channels(stereo_spectrum: bool) -> usize {
        if stereo_spectrum { NUM_CHANNELS } else { 1 }
    }
}
