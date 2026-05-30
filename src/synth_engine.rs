use core::f32;
use std::{
    any::Any,
    collections::{HashMap, HashSet},
    sync::Arc,
};

use nih_plug::params::FloatParam;
use parking_lot::Mutex;
use smallvec::SmallVec;
use topo_sort::{SortResults, TopoSort};

use crate::synth_engine::{
    buffer::{Buffer, SpectralBuffer, add_to_buffer, copy_or_add_to_buffer},
    config::{ModuleConfig, RoutingConfig},
    modules::{
        AmplifierConfig, EnvelopeConfig, ExpressionsConfig, ExternalParamConfig, LfoConfig,
        MixerConfig, Output, OutputConfig, SpectralBlendConfig, SpectralFilterConfig,
        SpectralMixerConfig, WaveShaperConfig,
        harmonic_editor::HarmonicEditorConfig,
        oscillator::{Oscillator, OscillatorConfig},
    },
    routing::{DataType, InputModulationUI, LinkModulation, NUM_CHANNELS, Router, VoiceEvent},
    smooth::SmoothedSampleParams,
    synth_module::ProcessParams,
    voices_handler::{
        DecayingVoices, MAX_AVAILABLE_VOICES, PlayingVoices, VoiceEvents, VoicesHandler,
    },
};

pub use buffer::SPECTRAL_BUFFER_SIZE;
pub use config::Config;
pub use modules::{
    Amplifier, Envelope, EnvelopeCurve, Expressions, ExternalParam, ExternalParamsBlock, Lfo,
    LfoShape, Mixer, ShaperType, SpectralBlend, SpectralFilter, SpectralFilterType, SpectralMixer,
    WaveShaper,
    harmonic_editor::{self, HarmonicEditor},
    oscillator::{self},
};
pub use routing::{
    AvailableInputSourceUI, ConnectedInputSourceUI, Expression, Input, MixType, ModuleId,
    ModuleInput, ModuleLink, ModuleType, OUTPUT_MODULE_ID, VolumeType,
};
pub use stereo_sample::StereoSample;
pub use synth_module::SynthModule;
pub use types::Sample;

mod buffer;
mod config;
#[macro_use]
mod synth_module;
mod biquad_filter;
mod curves;
mod iir_decimator;
mod modules;
mod phase;
mod routing;
mod smooth;
mod stereo_sample;
mod types;
mod voices_handler;

pub const MAX_BLOCK_SIZE: usize = 128;

pub struct SynthEngineUiData {
    pub voices: usize,
    pub legato: bool,
    pub block_size: usize,
    pub voice_kill_time: Sample,
    pub oversampling: bool,
    pub stereo_spectrum: bool,
    pub waiting_notes: usize,
    pub playing_voices: usize,
    pub releasing_voices: usize,
    pub killing_voices: usize,
}

#[derive(Debug, Clone, Copy)]
struct ModuleInputSource {
    src: ModuleId,
    amount: StereoSample,
    modulation: Option<LinkModulation>,
}

impl ModuleInputSource {
    fn source_ids(&self) -> impl Iterator<Item = ModuleId> {
        let mut ids: SmallVec<[ModuleId; 2]> = SmallVec::new();

        ids.push(self.src);

        if let Some(modulation) = self.modulation {
            ids.push(modulation.src);
        }

        ids.into_iter()
    }
}

pub struct SynthEngine {
    next_id: ModuleId,
    host_sample_rate: f32,
    block_size: usize,
    oversampling: bool,
    spectrum_channels: usize,
    config: Arc<Mutex<Config>>,
    modules: HashMap<ModuleId, Option<Box<dyn SynthModule>>>,
    input_sources: HashMap<ModuleInput, Vec<ModuleInputSource>>,
    needs_audio_rate: HashSet<ModuleId>,
    execution_order: Vec<ModuleId>,
    voices_handler: VoicesHandler,
    external_params: Option<Arc<ExternalParamsBlock>>,
    output_level_param: Option<Arc<FloatParam>>,
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

macro_rules! get_typed_module {
    ($self:ident, $module_type:ident, $module_id:expr) => {
        $self
            .modules
            .get($module_id)
            .and_then(|result| result.as_deref())
            .and_then($module_type::downcast)
    };
}

macro_rules! add_module_method {
    ($func_name:ident, $module_type:ident, $module_cfg:ident $(, $arg:ident )*) => {
        pub fn $func_name(&mut self) -> ModuleId {
            let id = self.alloc_module_id();
            let config = Arc::new(Mutex::new($module_cfg::default()));
            let module = Box::new($module_type::new(id, Arc::clone(&config) $(, self.$arg() )*));

            self.modules.insert(id, Some(module));
            self.config
                .lock()
                .modules
                .insert(id, ModuleConfig::$module_type(Arc::clone(&config)));
            id
        }
    };
}

impl SynthEngine {
    pub const AVAILABLE_VOICES: usize = MAX_AVAILABLE_VOICES;

    pub fn new() -> Self {
        Self {
            next_id: 0,
            host_sample_rate: 0.0,
            block_size: 0,
            oversampling: false,
            spectrum_channels: NUM_CHANNELS,
            config: Default::default(),
            modules: HashMap::new(),
            input_sources: HashMap::new(),
            needs_audio_rate: HashSet::new(),
            execution_order: Vec::new(),
            voices_handler: VoicesHandler::new(),
            external_params: None,
            output_level_param: None,
        }
    }

    pub fn init(
        &mut self,
        config: Arc<Mutex<Config>>,
        output_level_param: Arc<FloatParam>,
        external_params: ExternalParamsBlock,
        host_sample_rate: Sample,
    ) {
        self.config = config;
        self.host_sample_rate = host_sample_rate;
        self.external_params = Some(Arc::new(external_params));
        self.output_level_param = Some(Arc::clone(&output_level_param));

        self.reset();

        if !self.load_config() {
            self.reset_config();
            self.reset();
        }
    }

    fn sample_rate(&self) -> Sample {
        if self.oversampling {
            2.0 * self.host_sample_rate
        } else {
            self.host_sample_rate
        }
    }

    pub fn get_config(&self) -> Config {
        self.config.lock().clone()
    }

    pub fn set_config(&mut self, config: &Config) -> bool {
        let prev_config = self.config.lock().clone();

        *self.config.lock() = config.clone();
        self.reset();

        if !self.load_config() {
            *self.config.lock() = prev_config;
            self.reset();
            self.load_config();
            false
        } else {
            true
        }
    }

    pub fn is_empty(&self) -> bool {
        self.modules.len() == 1
    }

    pub fn get_ui(&self) -> SynthEngineUiData {
        let voices_ui_data = self.voices_handler.get_ui_data();

        SynthEngineUiData {
            voices: voices_ui_data.num_voices,
            legato: voices_ui_data.legato,
            block_size: self.block_size,
            voice_kill_time: get_typed_module!(self, Output, &OUTPUT_MODULE_ID)
                .map_or(0.0, |output| output.get_voice_kill_time()),
            oversampling: self.oversampling,
            stereo_spectrum: self.spectrum_channels == NUM_CHANNELS,
            waiting_notes: voices_ui_data.waiting,
            playing_voices: voices_ui_data.playing,
            releasing_voices: voices_ui_data.releasing,
            killing_voices: voices_ui_data.killing,
        }
    }

    pub fn set_num_voices(&mut self, num_voices: usize) {
        let num_voices = Self::clamp_num_voices(num_voices);

        self.voices_handler.set_num_voices(num_voices);
        self.config.lock().routing.num_voices = num_voices;
    }

    pub fn set_legato(&mut self, legato: bool) {
        self.voices_handler.set_legato(legato);
        self.config.lock().routing.legato = legato;
    }

    pub fn block_size(&self) -> usize {
        self.block_size
    }

    pub fn set_block_size(&mut self, block_size: usize) {
        self.block_size = Self::clamp_block_size(block_size);
        self.config.lock().routing.block_size = self.block_size;
    }

    pub fn set_voice_kill_time(&mut self, voice_kill_time: Sample) {
        if let Some(output) = self.get_typed_module_mut::<Output>(OUTPUT_MODULE_ID) {
            output.set_voice_kill_time(voice_kill_time);
        }
    }

    pub fn set_oversampling(&mut self, oversampling: bool) {
        self.oversampling = oversampling;
        self.config.lock().routing.oversampling = oversampling;
    }

    pub fn set_stereo_spectrum(&mut self, stereo_spectrum: bool) {
        self.spectrum_channels = Self::stereo_spectrum_channels(stereo_spectrum);
        self.config.lock().routing.stereo_spectrum = stereo_spectrum;
    }

    pub fn get_output_level(&self) -> StereoSample {
        get_typed_module!(self, Output, &OUTPUT_MODULE_ID)
            .map_or(StereoSample::ZERO, |output| output.get_gain())
    }

    pub fn set_output_level(&mut self, level: StereoSample) {
        if let Some(output) = self.get_typed_module_mut::<Output>(OUTPUT_MODULE_ID) {
            output.set_gain(level);
        }
    }

    fn clamp_num_voices(num_voices: usize) -> usize {
        num_voices.clamp(1, Self::AVAILABLE_VOICES)
    }

    fn clamp_block_size(block_size: usize) -> usize {
        (block_size).clamp(4, MAX_BLOCK_SIZE)
    }

    add_module_method!(add_oscillator, Oscillator, OscillatorConfig);
    add_module_method!(add_envelope, Envelope, EnvelopeConfig);
    add_module_method!(add_lfo, Lfo, LfoConfig);
    add_module_method!(add_amplifier, Amplifier, AmplifierConfig);
    add_module_method!(add_mixer, Mixer, MixerConfig);
    add_module_method!(add_wave_shaper, WaveShaper, WaveShaperConfig);
    add_module_method!(add_spectral_filter, SpectralFilter, SpectralFilterConfig);
    add_module_method!(add_spectral_blend, SpectralBlend, SpectralBlendConfig);
    add_module_method!(add_spectral_mixer, SpectralMixer, SpectralMixerConfig);
    add_module_method!(add_harmonic_editor, HarmonicEditor, HarmonicEditorConfig);
    add_module_method!(add_expressions, Expressions, ExpressionsConfig);
    add_module_method!(
        add_external_param,
        ExternalParam,
        ExternalParamConfig,
        get_external_params
    );

    fn get_external_params(&self) -> Arc<ExternalParamsBlock> {
        Arc::clone(self.external_params.as_ref().unwrap())
    }

    pub fn remove_module(&mut self, id: ModuleId) {
        if !self.modules.contains_key(&id) {
            return;
        };

        self.modules.remove(&id);
        self.config.lock().modules.remove(&id);

        let new_links: Vec<_> = self
            .get_links()
            .into_iter()
            .filter(|link| !(link.src == id || link.dst.module_id == id))
            .collect();

        self.setup_routing(&new_links).unwrap();
        self.save_links();
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

        new_links.push(ModuleLink::modulation(src, dst, amount));
        self.setup_routing(&new_links)?;
        self.save_links();
        Ok(())
    }

    pub fn update_link_amount(&mut self, src: &ModuleId, dst: &ModuleInput, amount: StereoSample) {
        if let Some(inputs) = self.input_sources.get_mut(dst)
            && let Some(input) = inputs.iter_mut().find(|input| input.src == *src)
        {
            input.amount = amount;
            self.save_links();
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
            && let Some(source) = sources.iter_mut().find(|src| src.src == src_id)
        {
            source.modulation = Some(LinkModulation { src: modulator_id });
            self.setup_routing(&self.get_links())?;
            self.save_links();

            Ok(())
        } else {
            Err("Invalid node.".to_string())
        }
    }

    pub fn remove_link_modulation(&mut self, src_id: ModuleId, dst_input: &ModuleInput) {
        if let Some(sources) = self.input_sources.get_mut(dst_input)
            && let Some(source) = sources.iter_mut().find(|src| src.src == src_id)
        {
            source.modulation = None;
            self.setup_routing(&self.get_links()).unwrap();
            self.save_links();
        }
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

    fn process_voice_events(&mut self, events: &[VoiceEvent]) {
        for module_id in &self.execution_order {
            if let Some(module) = get_module_mut!(self, &module_id) {
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

    pub fn process<'a>(&mut self, samples: usize, outputs: impl Iterator<Item = &'a mut [f32]>) {
        {
            let mut decaying_voices = DecayingVoices::new();

            self.voices_handler
                .get_decaying_voices(&mut decaying_voices);

            self.execution_order
                .iter()
                .filter_map(|id| get_module!(self, id))
                .for_each(|module| module.poll_decaying_voices(&mut decaying_voices));

            self.voices_handler.update_decaying_voices(&decaying_voices);
        }

        let mut playing_voices = PlayingVoices::new();

        self.voices_handler.get_playing_voices(&mut playing_voices);

        let samples = if self.oversampling {
            2 * samples
        } else {
            samples
        };
        let sample_rate = self.sample_rate();

        let mut params = ProcessParams {
            samples,
            sample_rate,
            buffer_t_step: samples as Sample / sample_rate,
            smooth_params: SmoothedSampleParams::new(sample_rate),
            needs_audio_rate: false,
            needs_update_ui: true,
            spectrum_channels: self.spectrum_channels,
            active_voices: &playing_voices,
        };

        for module_id in &self.execution_order {
            if let Some(module_box) = self.modules.get_mut(module_id)
                && let Some(mut module) = module_box.take()
            {
                params.needs_audio_rate = self.needs_audio_rate.contains(module_id);
                module.process(&params, self);
                self.modules.get_mut(module_id).unwrap().replace(module);
            }
        }

        let oversampling = self.oversampling;

        if let Some(output) = self.get_typed_module_mut::<Output>(OUTPUT_MODULE_ID) {
            output.read_output(oversampling, outputs);
        }
    }

    fn alloc_module_id(&mut self) -> ModuleId {
        let module_id = self.next_id;

        self.next_id += 1;
        self.config.lock().routing.next_module_id = self.next_id;
        module_id
    }

    fn data_types_compatible(src: DataType, dst: DataType) -> bool {
        src == dst || (dst == DataType::Buffer && src == DataType::Scalar)
    }

    fn can_be_linked_with_output(&self, src: &ModuleId, dst: &ModuleInput) -> Result<(), String> {
        let Some(src_module) = get_module!(self, src) else {
            return Err("Invalid node.".to_string());
        };

        let is_compatible = dst.input_type == Input::Audio
            && Self::data_types_compatible(src_module.output(), DataType::Buffer);

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

        let src_data_types = src_module.output();

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
                    amount: src.amount,
                    modulation: src.modulation,
                })
            })
            .collect()
    }

    pub fn get_modules(&self) -> Vec<&dyn SynthModule> {
        self.modules
            .values()
            .filter_map(|val| val.as_deref())
            .filter(|m| m.module_type() != ModuleType::Output)
            .collect()
    }

    pub fn get_module_mut(&mut self, id: ModuleId) -> Option<&mut dyn SynthModule> {
        get_module_mut!(self, &id)
    }

    pub fn get_typed_module_mut<T: SynthModule>(&mut self, id: ModuleId) -> Option<&mut T> {
        self.get_module_mut(id)
            .and_then(|module| (module as &mut dyn Any).downcast_mut())
    }

    pub fn get_available_input_sources(&self, input: ModuleInput) -> Vec<AvailableInputSourceUI> {
        let dst_data_type =
            if input.module_id == OUTPUT_MODULE_ID && input.input_type == Input::Audio {
                DataType::Buffer
            } else if let Some(input_module) = get_module!(self, &input.module_id)
                && let Some(input_info) = input_module
                    .inputs()
                    .iter()
                    .find(|input_info| input_info.input == input.input_type)
            {
                input_info.data_type
            } else {
                return Vec::new();
            };

        self.modules
            .values()
            .filter_map(|module| module.as_deref())
            .filter(|module| {
                module.id() != input.module_id
                    && Self::data_types_compatible(module.output(), dst_data_type)
                    && !self.is_connected_to_source(module.id(), input.module_id)
            })
            .map(|module| AvailableInputSourceUI {
                src: module.id(),
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
                src: source.src,
                amount: source.amount,
                label: module.label(),
                modulation: source
                    .modulation
                    .and_then(|modulation| {
                        get_module!(self, &modulation.src).map(|module| (module, modulation))
                    })
                    .map(|(module, modulation)| InputModulationUI {
                        src: modulation.src,
                        label: module.label(),
                    }),
            })
            .collect()
    }

    fn is_connected_to_source(&self, dst_id: ModuleId, src_id: ModuleId) -> bool {
        for (input, sources) in &self.input_sources {
            if input.module_id == dst_id {
                for source in sources.iter().flat_map(|src| src.source_ids()) {
                    if source == src_id || self.is_connected_to_source(source, src_id) {
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

            if let Some(modulation) = link.modulation {
                dependents
                    .entry(dst_node)
                    .or_default()
                    .insert(modulation.src);
                dependents.entry(modulation.src).or_default();
            }
        }

        let topo_sort = TopoSort::from_map(dependents);

        match topo_sort.into_vec_nodes() {
            SortResults::Full(nodes) => Ok(nodes.into_iter().collect()),
            SortResults::Partial(_) => Err("Cycles detected!".to_string()),
        }
    }

    fn setup_routing(&mut self, links: &[ModuleLink]) -> Result<(), String> {
        fn mark_needs_audio_rate(
            sources: &[ModuleInputSource],
            input_sources: &HashMap<ModuleInput, Vec<ModuleInputSource>>,
            modules: &HashMap<ModuleId, Option<Box<dyn SynthModule>>>,
            needs_audio_rate: &mut HashSet<ModuleId>,
        ) {
            const NEEDS_BUFFER_TYPES: &[DataType] = &[DataType::Buffer, DataType::Scalar];

            for module_id in sources.iter().flat_map(|src| src.source_ids()) {
                if let Some(src_module) =
                    modules.get(&module_id).and_then(|result| result.as_deref())
                    && NEEDS_BUFFER_TYPES.contains(&src_module.output())
                {
                    needs_audio_rate.insert(module_id);

                    for (input, sources) in input_sources {
                        if input.module_id == module_id {
                            mark_needs_audio_rate(
                                sources,
                                input_sources,
                                modules,
                                needs_audio_rate,
                            );
                        }
                    }
                }
            }
        }

        let execution_order = Self::calc_execution_order(links)?;
        let mut input_sources: HashMap<ModuleInput, Vec<ModuleInputSource>> = HashMap::new();
        let mut needs_audio_rate: HashSet<ModuleId> = HashSet::new();

        for link in links {
            input_sources
                .entry(link.dst)
                .or_default()
                .push(ModuleInputSource {
                    src: link.src,
                    amount: link.amount,
                    modulation: link.modulation,
                });
        }

        if let Some(output_sources) =
            input_sources.get(&ModuleInput::new(Input::Audio, OUTPUT_MODULE_ID))
        {
            mark_needs_audio_rate(
                output_sources,
                &input_sources,
                &self.modules,
                &mut needs_audio_rate,
            );
        }

        self.input_sources = input_sources;
        self.needs_audio_rate = needs_audio_rate;
        self.execution_order = execution_order;
        Ok(())
    }

    fn stereo_spectrum_channels(stereo_spectrum: bool) -> usize {
        if stereo_spectrum { NUM_CHANNELS } else { 1 }
    }

    fn reset(&mut self) {
        let default_cfg = RoutingConfig::default();

        self.execution_order.clear();
        self.input_sources.clear();
        self.modules.clear();
        self.next_id = default_cfg.next_module_id;
        self.block_size = default_cfg.block_size;
        self.oversampling = default_cfg.oversampling;
        self.spectrum_channels = Self::stereo_spectrum_channels(default_cfg.stereo_spectrum);
        self.voices_handler.set_num_voices(default_cfg.num_voices);
        self.voices_handler.set_legato(default_cfg.legato);

        self.modules.insert(
            OUTPUT_MODULE_ID,
            Some(Box::new(Output::new(
                Arc::clone(&self.config.lock().output),
                Arc::clone(self.output_level_param.as_ref().unwrap()),
            ))),
        );
    }

    fn reset_config(&mut self) {
        let mut cfg = self.config.lock();

        cfg.routing = RoutingConfig::default();
        cfg.modules.clear();
        *cfg.output.lock() = OutputConfig::default();
    }

    fn load_config(&mut self) -> bool {
        let cfg = Arc::clone(&self.config);
        let cfg = cfg.lock();

        if cfg.modules.is_empty() {
            return false;
        }

        self.next_id = cfg.routing.next_module_id;
        self.block_size = Self::clamp_block_size(cfg.routing.block_size);
        self.oversampling = cfg.routing.oversampling;
        self.spectrum_channels = Self::stereo_spectrum_channels(cfg.routing.stereo_spectrum);
        self.voices_handler
            .set_num_voices(Self::clamp_num_voices(cfg.routing.num_voices));
        self.voices_handler.set_legato(cfg.routing.legato);

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

        for (id, cfg) in cfg.modules.iter() {
            match cfg {
                ModuleConfig::Amplifier(cfg) => restore_module!(Amplifier, id, cfg),
                ModuleConfig::Envelope(cfg) => restore_module!(Envelope, id, cfg),
                ModuleConfig::Oscillator(cfg) => restore_module!(Oscillator, id, cfg),
                ModuleConfig::SpectralFilter(cfg) => restore_module!(SpectralFilter, id, cfg),
                ModuleConfig::SpectralBlend(cfg) => restore_module!(SpectralBlend, id, cfg),
                ModuleConfig::SpectralMixer(cfg) => restore_module!(SpectralMixer, id, cfg),
                ModuleConfig::HarmonicEditor(cfg) => restore_module!(HarmonicEditor, id, cfg),
                ModuleConfig::ExternalParam(cfg) => {
                    restore_module!(ExternalParam, id, cfg, get_external_params)
                }
                ModuleConfig::Lfo(cfg) => restore_module!(Lfo, id, cfg),
                ModuleConfig::WaveShaper(cfg) => restore_module!(WaveShaper, id, cfg),
                ModuleConfig::Mixer(cfg) => restore_module!(Mixer, id, cfg),
                ModuleConfig::Expressions(cfg) => restore_module!(Expressions, id, cfg),
            }
        }

        for link in &cfg.routing.links {
            if self.can_be_linked(&link.src, &link.dst).is_err() {
                return false;
            }

            if let Some(modulation) = link.modulation
                && self.can_be_linked(&modulation.src, &link.dst).is_err()
            {
                return false;
            }
        }

        self.setup_routing(&cfg.routing.links).is_ok()
    }

    fn save_links(&self) {
        self.config.lock().routing.links = self.get_links();
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
            && let Some(module) = get_module!(self, &first.src)
        {
            return Some(module.get_buffer_output(voice_idx, channel_idx));
        }

        if sources.is_empty() {
            return None;
        }

        let result = &mut input_buffer[..samples];

        let modules = sources.iter().filter_map(|source| {
            get_module!(self, &source.src).map(|module| (module, source.amount, source.modulation))
        });

        for (mod_idx, (module, amount, modulation)) in modules.enumerate() {
            let amount = amount[channel_idx];
            let input = module
                .get_buffer_output(voice_idx, channel_idx)
                .iter()
                .map(|sample| sample * amount);

            if let Some(modulation) = modulation
                && let Some(module) = get_module!(self, &modulation.src)
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
            get_module!(self, &source.src).map(|module| (module, source.amount, source.modulation))
        });

        for (module, amount, modulation) in modules {
            let amount = amount[channel_idx];
            let input = module
                .get_buffer_output(voice_idx, channel_idx)
                .iter()
                .map(|sample| sample * amount);

            if let Some(modulation) = modulation
                && let Some(module) = get_module!(self, &modulation.src)
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
                .filter_map(|source| get_module!(self, &source.src));

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
            && let Some(module) = get_module!(self, &first.src)
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
            get_module!(self, &source.src).map(|module| {
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
                && let Some(module) = get_module!(self, &modulation.src)
            {
                input *= module.get_scalar_output(current, voice_idx, channel_idx);
            }

            output += input;
        }

        Some(output)
    }
}
