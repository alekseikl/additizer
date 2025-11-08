use core::f32;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use itertools::{Itertools, izip};
use nih_plug::{params::FloatParam, prelude::FloatRange, util::db_to_gain_fast};
use parking_lot::Mutex;
use smallvec::SmallVec;
use topo_sort::{SortResults, TopoSort};

use crate::{
    synth_engine::{
        buffer::{Buffer, ZEROES_BUFFER, make_zero_buffer},
        config::ModuleConfig,
        modules::{
            AmplifierConfig, EnvelopeConfig, HarmonicEditorConfig, OscillatorConfig,
            SpectralFilterConfig,
        },
        routing::{
            AvailableInputSourceUI, ConnectedInputSourceUI, MAX_VOICES, MIN_MODULE_ID,
            OUTPUT_MODULE_ID, Router,
        },
        synth_module::{
            NoteOffParams, NoteOnParams, ProcessParams, ScalarOutputs, SpectralOutputs,
        },
    },
    utils::{from_ms, st_to_octave},
};

pub use buffer::BUFFER_SIZE;
pub use config::Config;
pub use modules::{Amplifier, Envelope, HarmonicEditor, Oscillator, SpectralFilter};
pub use routing::{InputType, ModuleId, ModuleInput, ModuleLink, ModuleOutput, ModuleType};
pub use synth_module::SynthModule;
pub use types::{Sample, StereoSample};

mod buffer;
mod config;
mod envelope;
#[macro_use]
mod synth_module;
mod modules;
mod routing;
mod types;

#[derive(Debug, Clone, Copy)]
pub struct VoiceId {
    pub voice_id: Option<i32>,
    pub channel: u8,
    pub note: u8,
}

#[derive(Debug, Default)]
struct Voice {
    id: u64,
    external_voice_id: Option<i32>,
    channel: u8,
    note: u8,
    active: bool,
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
    src: ModuleOutput,
    modulation: Option<StereoSample>,
}

pub struct SynthEngine {
    next_id: ModuleId,
    next_voice_id: u64,
    sample_rate: f32,
    config: Arc<Config>,
    modules: HashMap<ModuleId, Option<Box<dyn SynthModule>>>,
    input_sources: HashMap<ModuleInput, Vec<ModuleInputSource>>,
    execution_order: Vec<ModuleId>,
    voices: [Voice; MAX_VOICES],
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

macro_rules! typed_modules {
    ($self:ident, $module_type:ident) => {
        $self
            .modules
            .values()
            .filter_map(|item| {
                item.as_deref()
                    .filter(|mod_box| mod_box.module_type() == ModuleType::$module_type)
            })
            .filter_map($module_type::downcast)
    };
}

// macro_rules! typed_modules_mut {
//     ($self:ident, $module_type:ident) => {
//         $self
//             .modules
//             .values_mut()
//             .filter_map(|item| {
//                 item.as_deref_mut()
//                     .filter(|mod_box| mod_box.module_type() == ModuleType::$module_type)
//             })
//             .filter_map($module_type::downcast_mut)
//     };
// }

macro_rules! add_module_method {
    ($func_name:ident, $module_type:ident, $module_cfg:ident) => {
        pub fn $func_name(&mut self) -> ModuleId {
            let id = self.alloc_next_id();
            let config = Arc::new(Mutex::new($module_cfg::default()));
            let mut module = $module_type::new(id, Arc::clone(&config));

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
        Self {
            next_id: MIN_MODULE_ID,
            next_voice_id: 1,
            sample_rate: 1000.0,
            config: Default::default(),
            modules: HashMap::new(),
            input_sources: HashMap::new(),
            execution_order: Vec::new(),
            voices: Default::default(),
            output_level: StereoSample::splat(1.0),
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
        sample_rate: f32,
    ) {
        self.config = config;
        self.sample_rate = sample_rate;
        self.output_level_param = output_level_param;

        if !self.load_config() {
            self.clear();
            self.build_scheme();
        }
    }

    add_module_method!(add_oscillator, Oscillator, OscillatorConfig);
    add_module_method!(add_envelope, Envelope, EnvelopeConfig);
    add_module_method!(add_amplifier, Amplifier, AmplifierConfig);
    add_module_method!(add_spectral_filter, SpectralFilter, SpectralFilterConfig);
    add_module_method!(add_harmonic_editor, HarmonicEditor, HarmonicEditorConfig);

    pub fn remove_module(&mut self, id: ModuleId) {
        if !self.modules.contains_key(&id) {
            return;
        };

        self.modules.remove(&id);
        self.config.modules.lock().remove(&id);

        let new_links: Vec<_> = self
            .get_links()
            .into_iter()
            .filter(|link| link.src.module_id == id || link.dst.module_id == id)
            .collect();

        self.setup_routing(&new_links).unwrap();
    }

    pub fn set_direct_link(&mut self, src: ModuleOutput, dst: ModuleInput) -> Result<(), String> {
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

    pub fn set_link(&mut self, link: ModuleLink) -> Result<(), String> {
        if let Some(inputs) = self.input_sources.get_mut(&link.dst)
            && let Some(input) = inputs.iter_mut().find(|input| input.src == link.src)
        {
            input.modulation = link.modulation;
            return Ok(());
        }

        if link.src.data_type() != link.dst.data_type() {
            return Err("Data types mismatch.".to_string());
        }

        if !self.input_exists(&link.dst) || !self.output_exists(&link.src) {
            return Err("Invalid node.".to_string());
        }

        let mut new_links = self.get_links();

        new_links.push(link);
        self.setup_routing(&new_links)?;
        self.save_links();
        Ok(())
    }

    pub fn remove_link(&mut self, src: ModuleOutput, dst: ModuleInput) {
        let new_links: Vec<_> = self
            .get_links()
            .into_iter()
            .filter(|link| !(link.src == src && link.dst == dst))
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

    pub fn note_on(
        &mut self,
        voice_id: Option<i32>,
        channel: u8,
        note: u8,
        _velocity: f32,
    ) -> Option<VoiceId> {
        let new_voice = Voice {
            id: self.next_voice_id,
            external_voice_id: voice_id,
            channel,
            note,
            active: true,
        };
        let mut terminated_voice: Option<VoiceId> = None;
        let (voice_idx, same_note) = if let Some(voice_idx) = self
            .voices
            .iter()
            .position(|voice| voice.active && voice.note == note)
        {
            terminated_voice = Some(self.voices[voice_idx].get_id());
            (voice_idx, true)
        } else if let Some(voice_idx) = self.voices.iter().position(|voice| !voice.active) {
            (voice_idx, false)
        } else {
            let voice_idx = self
                .voices
                .iter()
                .position_min_by_key(|voice| voice.id)
                .unwrap();

            terminated_voice = Some(self.voices[voice_idx].get_id());
            (voice_idx, false)
        };

        self.voices[voice_idx] = new_voice;
        self.next_voice_id = self.next_voice_id.wrapping_add(1);

        let params = NoteOnParams {
            note: note as f32,
            // velocity,
            voice_idx,
            same_note_retrigger: same_note,
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
            .position(|voice| voice.active && voice.note == note)
        else {
            return;
        };

        let params = NoteOffParams {
            //note,
            voice_idx,
        };

        for module_id in &self.execution_order {
            if let Some(module) = get_module_mut!(self, &module_id) {
                module.note_off(&params);
            }
        }
    }

    pub fn choke(&mut self, note: u8) -> Option<VoiceId> {
        let voice_idx = self
            .voices
            .iter()
            .position(|voice| voice.active && voice.note == note)?;

        let voice = &mut self.voices[voice_idx];

        voice.active = false;
        Some(voice.get_id())
    }

    pub fn process<'a>(
        &mut self,
        samples: usize,
        outputs: impl Iterator<Item = &'a mut [f32]>,
        mut on_terminate_voice: impl FnMut(VoiceId),
    ) {
        let mut env_activity: SmallVec<[envelope::EnvelopeActivityState; MAX_VOICES]> = self
            .voices
            .iter()
            .enumerate()
            .filter(|(_, voice)| voice.active)
            .map(|(voice_idx, _)| envelope::EnvelopeActivityState {
                voice_idx,
                active: false,
            })
            .collect();

        for env in typed_modules!(self, Envelope) {
            env.check_activity(&mut env_activity);
        }

        for activity in &env_activity {
            if !activity.active {
                let voice = &mut self.voices[activity.voice_idx];

                on_terminate_voice(voice.get_id());
                voice.active = false;
            }
        }

        let active_voices: SmallVec<[usize; MAX_VOICES]> = env_activity
            .iter()
            .filter(|activity| activity.active)
            .map(|activity| activity.voice_idx)
            .collect();

        let params = ProcessParams {
            samples,
            sample_rate: self.sample_rate,
            t_step: self.sample_rate.recip(),
            // buffer_t_step: samples as Sample / self.sample_rate,
            active_voices: &active_voices,
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
        self.config.routing.lock().last_module_id = self.next_id;
        module_id
    }

    fn trigger_active_notes(voices: &[Voice], module: &mut dyn SynthModule) {
        let active_voices = voices
            .iter()
            .enumerate()
            .filter(|(_, voice)| voice.active)
            .map(|(voice_idx, voice)| NoteOnParams {
                note: voice.note as f32,
                voice_idx,
                same_note_retrigger: false,
            });

        for params in active_voices {
            module.note_on(&params);
        }
    }

    fn input_exists(&self, input: &ModuleInput) -> bool {
        if input.module_id == OUTPUT_MODULE_ID {
            input.input_type == InputType::Input
        } else if let Some(module) = get_module!(self, &input.module_id) {
            module.inputs().contains(&input.input_type)
        } else {
            false
        }
    }

    fn output_exists(&self, output: &ModuleOutput) -> bool {
        if let Some(module) = get_module!(self, &output.module_id) {
            module.outputs().contains(&output.output_type)
        } else {
            false
        }
    }

    fn can_be_linked(&self, src: &ModuleOutput, dst: &ModuleInput) -> Result<(), String> {
        if src.data_type() != dst.data_type() {
            return Err("Data types mismatch.".to_string());
        }

        if !self.input_exists(dst) || !self.output_exists(src) {
            return Err("Invalid node.".to_string());
        }

        Ok(())
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

            for voice_idx in params.active_voices {
                let input = self
                    .get_input(
                        ModuleInput::input(OUTPUT_MODULE_ID),
                        *voice_idx,
                        channel,
                        &mut tmp_buffers.1,
                    )
                    .unwrap_or(&ZEROES_BUFFER);

                for (out, input, level_mod, _) in
                    izip!(output.iter_mut(), input, &tmp_buffers.0, 0..params.samples)
                {
                    *out += input * level * level_mod;
                }
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

    pub fn get_module(&mut self, id: ModuleId) -> Option<&dyn SynthModule> {
        get_module!(self, &id)
    }

    pub fn get_module_mut(&mut self, id: ModuleId) -> Option<&mut dyn SynthModule> {
        get_module_mut!(self, &id)
    }

    pub fn get_available_input_sources(&self, input: ModuleInput) -> Vec<AvailableInputSourceUI> {
        let input_data_type = input.data_type();

        self.modules
            .values()
            .filter_map(|module| module.as_deref())
            .filter(|module| {
                module.id() != input.module_id
                    && module
                        .outputs()
                        .iter()
                        .any(|output| output.data_type() == input_data_type)
                    && !self.is_connected_to_source(module.id(), input.module_id)
            })
            .flat_map(|module| {
                module
                    .outputs()
                    .iter()
                    .filter(|output| output.data_type() == input_data_type)
                    .map(|output| AvailableInputSourceUI {
                        output: ModuleOutput::new(*output, module.id()),
                        label: module.label(),
                    })
            })
            .collect()
    }

    pub fn get_connected_input_sources(&self, input: ModuleInput) -> Vec<ConnectedInputSourceUI> {
        let Some(sources) = self.input_sources.get(&input) else {
            return Vec::new();
        };

        sources
            .iter()
            .filter_map(|source| {
                get_module!(self, &source.src.module_id).map(|module| (module, source))
            })
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
                    if source.src.module_id == src_id
                        || self.is_connected_to_source(source.src.module_id, src_id)
                    {
                        return true;
                    }
                }
            }
        }

        false
    }

    fn build_scheme(&mut self) {
        let harmonic_editor_id = self.add_harmonic_editor();
        let filter_env_id = self.add_envelope();
        let filter_id = self.add_spectral_filter();
        let osc_id = self.add_oscillator();
        let amp_id = self.add_amplifier();
        let amp_env_id = self.add_envelope();

        macro_rules! typed_module_mut {
            ($module_id:expr, $module_type:ident) => {
                self.modules
                    .get_mut($module_id)
                    .and_then(|result| result.as_deref_mut())
                    .and_then(|module| $module_type::downcast_mut(module))
            };
        }

        typed_module_mut!(&filter_env_id, Envelope)
            .unwrap()
            .set_attack(0.0.into())
            .set_decay(from_ms(500.0).into())
            .set_sustain(0.0.into())
            .set_release(from_ms(100.0).into());

        typed_module_mut!(&filter_id, SpectralFilter)
            .unwrap()
            .set_cutoff(2.0.into());

        typed_module_mut!(&osc_id, Oscillator)
            .unwrap()
            .set_unison(3)
            .set_detune(st_to_octave(0.01).into());

        typed_module_mut!(&amp_env_id, Envelope)
            .unwrap()
            .set_attack(StereoSample::splat(from_ms(10.0)))
            .set_decay(from_ms(20.0).into())
            .set_sustain(1.0.into())
            .set_release(from_ms(300.0).into());

        self.set_link(ModuleLink::link(
            ModuleOutput::spectrum(harmonic_editor_id),
            ModuleInput::spectrum(filter_id),
        ))
        .unwrap();
        self.set_link(ModuleLink::modulation(
            ModuleOutput::scalar(filter_env_id),
            ModuleInput::cutoff_scalar(filter_id),
            st_to_octave(64.0),
        ))
        .unwrap();
        self.set_link(ModuleLink::link(
            ModuleOutput::spectrum(filter_id),
            ModuleInput::spectrum(osc_id),
        ))
        .unwrap();
        self.set_link(ModuleLink::link(
            ModuleOutput::output(osc_id),
            ModuleInput::input(amp_id),
        ))
        .unwrap();
        self.set_link(ModuleLink::link(
            ModuleOutput::output(amp_env_id),
            ModuleInput::level(amp_id),
        ))
        .unwrap();
        self.set_link(ModuleLink::link(
            ModuleOutput::output(amp_id),
            ModuleInput::input(OUTPUT_MODULE_ID),
        ))
        .unwrap();
    }

    fn calc_execution_order(links: &[ModuleLink]) -> Result<Vec<ModuleId>, String> {
        let mut dependents: HashMap<ModuleId, HashSet<ModuleId>> = HashMap::new();

        for link in links {
            let src_node = link.src.module_id;
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
        self.execution_order.clear();
        self.input_sources.clear();
        self.modules.clear();
        self.next_id = MIN_MODULE_ID;

        self.config.routing.lock().last_module_id = MIN_MODULE_ID;
        self.config.routing.lock().links.clear();
        self.config.modules.lock().clear();
    }

    fn load_config(&mut self) -> bool {
        let routing_arc = Arc::clone(&self.config.routing);
        let routing = routing_arc.lock();
        let modules_arc = Arc::clone(&self.config.modules);
        let modules_cfg = modules_arc.lock();

        macro_rules! restore_module {
            ($module_type:ident, $module_id:ident, $cfg:ident) => {{
                self.modules.insert(
                    *$module_id,
                    Some(Box::new($module_type::new(*$module_id, Arc::clone($cfg)))),
                );
            }};
        }

        if modules_cfg.is_empty() {
            return false;
        }

        for (id, cfg) in modules_cfg.iter() {
            match cfg {
                ModuleConfig::Amplifier(cfg) => restore_module!(Amplifier, id, cfg),
                ModuleConfig::Envelope(cfg) => restore_module!(Envelope, id, cfg),
                ModuleConfig::Oscillator(cfg) => restore_module!(Oscillator, id, cfg),
                ModuleConfig::SpectralFilter(cfg) => restore_module!(SpectralFilter, id, cfg),
                ModuleConfig::HarmonicEditor(cfg) => restore_module!(HarmonicEditor, id, cfg),
            }
        }

        self.next_id = routing.last_module_id;
        self.output_level = routing.output_level;
        self.setup_routing(&routing.links).is_ok()
    }

    fn save_links(&self) {
        let mut routing = self.config.routing.lock();

        routing.links = self.get_links();
    }
}

impl Router for SynthEngine {
    fn get_input<'a>(
        &'a self,
        input: ModuleInput,
        voice_idx: usize,
        channel: usize,
        input_buffer: &'a mut Buffer,
    ) -> Option<&'a Buffer> {
        let sources = self.input_sources.get(&input)?;

        if sources.is_empty() {
            return None;
        }

        if sources.len() == 1 && sources[0].modulation.is_none() {
            return get_module!(self, &sources[0].src.module_id)
                .map(|module| module.get_buffer_output(voice_idx, channel));
        }

        let buffs = sources.iter().filter_map(|source| {
            get_module!(self, &source.src.module_id).map(|module| {
                (
                    module.get_buffer_output(voice_idx, channel),
                    source.modulation,
                )
            })
        });

        for (idx, (buff, mod_amount)) in buffs.enumerate() {
            let mod_amount = mod_amount.map_or(1.0, |stereo_amount| stereo_amount[channel]);

            macro_rules! merge_buff {
                ($op:tt) => {
                    for (input, buff) in input_buffer.iter_mut().zip(buff) {
                        *input $op buff * mod_amount;
                    }
                };
            }

            if idx == 0 {
                merge_buff!(=);
            } else {
                merge_buff!(+=);
            }
        }

        Some(input_buffer)
    }

    fn get_spectral_input(
        &self,
        input: ModuleInput,
        voice_idx: usize,
        channel: usize,
    ) -> Option<SpectralOutputs<'_>> {
        let sources = self.input_sources.get(&input)?;

        if sources.is_empty() {
            return None;
        }

        get_module!(self, &sources[0].src.module_id)
            .map(|module| module.get_spectral_output(voice_idx, channel))
    }

    fn get_scalar_input(
        &self,
        input: ModuleInput,
        voice_idx: usize,
        channel: usize,
    ) -> Option<ScalarOutputs> {
        let sources = self.input_sources.get(&input)?;

        if sources.is_empty() {
            return None;
        }

        let mut outputs = ScalarOutputs::zero();

        let values = sources.iter().filter_map(|source| {
            get_module!(self, &source.src.module_id).map(|module| {
                (
                    module.get_scalar_output(voice_idx, channel),
                    source.modulation,
                )
            })
        });

        for (value, mod_amount) in values {
            let mod_amount = mod_amount.map_or(1.0, |stereo_amount| stereo_amount[channel]);

            outputs.first += value.first * mod_amount;
            outputs.current += value.current * mod_amount;
        }

        Some(outputs)
    }
}
