use core::f32;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use itertools::{Itertools, izip};
use nih_plug::util::db_to_gain_fast;
use parking_lot::lock_api::Mutex;
use smallvec::SmallVec;
use topo_sort::{SortResults, TopoSort};

use crate::synth_engine::{
    buffer::{Buffer, ZEROES_BUFFER, make_zero_buffer},
    config::ModuleConfig,
    modules::{AmplifierConfig, EnvelopeConfig, OscillatorConfig, SpectralFilterConfig},
    routing::{
        InputType, MAX_VOICES, MIN_MODULE_ID, ModuleId, ModuleInput, ModuleInputSource, ModuleLink,
        ModuleOutput, ModuleType, OUTPUT_MODULE_ID, Router,
    },
    synth_module::{
        NoteOffParams, NoteOnParams, ProcessParams, ScalarOutputs, SpectralOutputs, SynthModule,
    },
};

pub use buffer::BUFFER_SIZE;
pub use config::Config;
pub use modules::{Amplifier, Envelope, Oscillator, SpectralFilter};
pub use types::{Sample, StereoSample};

mod buffer;
mod config;
mod envelope;
mod modules;
// mod modules_container;
mod routing;
mod synth_module;
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

pub struct SynthEngine {
    next_id: ModuleId,
    next_voice_id: u64,
    sample_rate: f32,
    config: Arc<Config>,
    modules: HashMap<ModuleId, Option<Box<dyn SynthModule>>>,
    input_sources: HashMap<ModuleInput, Vec<ModuleInputSource>>,
    execution_order: Vec<ModuleId>,
    voices: [Voice; MAX_VOICES],
    tmp_output_buffer: Option<Box<Buffer>>,
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

macro_rules! typed_modules_mut {
    ($self:ident, $module_type:ident) => {
        $self
            .modules
            .values_mut()
            .filter_map(|item| {
                item.as_deref_mut()
                    .filter(|mod_box| mod_box.module_type() == ModuleType::$module_type)
            })
            .filter_map($module_type::downcast_mut)
    };
}

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
    pub fn new(config: Arc<Config>) -> Self {
        Self {
            next_id: MIN_MODULE_ID,
            next_voice_id: 1,
            sample_rate: 1000.0,
            config,
            modules: HashMap::new(),
            input_sources: HashMap::new(),
            execution_order: Vec::new(),
            voices: Default::default(),
            tmp_output_buffer: Some(Box::new(make_zero_buffer())),
        }
    }

    pub fn init(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;

        if !self.load_config() {
            self.clear();
            self.build_scheme();
        }
    }

    add_module_method!(add_oscillator, Oscillator, OscillatorConfig);
    add_module_method!(add_envelope, Envelope, EnvelopeConfig);
    add_module_method!(add_amplifier, Amplifier, AmplifierConfig);
    add_module_method!(add_spectral_filter, SpectralFilter, SpectralFilterConfig);

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
        self.save_config();
        Ok(())
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
        let mut tmp_buffer = self.tmp_output_buffer.take().unwrap();

        for (channel, output) in outputs.enumerate() {
            output.fill(0.0);

            for voice_idx in params.active_voices {
                let input = self
                    .get_input(
                        ModuleInput::input(OUTPUT_MODULE_ID),
                        *voice_idx,
                        channel,
                        &mut tmp_buffer,
                    )
                    .unwrap_or(&ZEROES_BUFFER);

                for (out, input, _) in izip!(output.iter_mut(), input, 0..params.samples) {
                    *out += input;
                }
            }
        }

        self.tmp_output_buffer.replace(tmp_buffer);
    }

    pub fn update_harmonics(&mut self, harmonics: &[StereoSample], tail: StereoSample) {
        for filter in typed_modules_mut!(self, SpectralFilter) {
            filter.set_harmonics(harmonics, tail);
        }
    }

    pub fn set_volume(&mut self, volume: f32) {
        let level = db_to_gain_fast(volume);

        for amp in typed_modules_mut!(self, Amplifier) {
            amp.set_level(StereoSample::mono(level));
        }
    }

    pub fn set_unison(&mut self, unison: usize) {
        for osc in typed_modules_mut!(self, Oscillator) {
            osc.set_unison(unison);
        }
    }

    pub fn set_detune(&mut self, detune: f32) {
        let detune = 0.01 * detune;

        for osc in typed_modules_mut!(self, Oscillator) {
            osc.set_detune(detune.into());
        }
    }

    pub fn set_cutoff(&mut self, cutoff: Sample) {
        for filter in typed_modules_mut!(self, SpectralFilter) {
            filter.set_cutoff_harmonic(cutoff.into());
        }
    }

    fn build_scheme(&mut self) {
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
            .set_decay(500.0.into())
            .set_sustain(0.0.into())
            .set_release(100.0.into());

        typed_module_mut!(&filter_id, SpectralFilter)
            .unwrap()
            .set_cutoff_harmonic(2.0.into());

        typed_module_mut!(&osc_id, Oscillator)
            .unwrap()
            .set_unison(3)
            .set_detune(0.01.into());

        typed_module_mut!(&amp_env_id, Envelope)
            .unwrap()
            .set_attack(StereoSample::mono(10.0))
            .set_decay(20.0.into())
            .set_sustain(1.0.into())
            .set_release(300.0.into());

        self.set_link(ModuleLink::link(
            ModuleOutput::output(amp_id),
            ModuleInput::input(OUTPUT_MODULE_ID),
        ))
        .unwrap();
        self.set_link(ModuleLink::modulation(
            ModuleOutput::scalar(filter_env_id),
            ModuleInput::cutoff_scalar(filter_id),
            50.0,
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
        self.next_id = 1;
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
            }
        }

        self.next_id = routing.last_module_id;
        self.setup_routing(&routing.links).is_ok()
    }

    fn save_config(&self) {
        let mut config = self.config.routing.lock();

        config.last_module_id = self.next_id;
        config.links = self.get_links();
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

            if idx == 0 {
                for (input, buff) in input_buffer.iter_mut().zip(buff) {
                    *input = buff * mod_amount;
                }
            } else {
                for (input, buff) in input_buffer.iter_mut().zip(buff) {
                    *input += buff * mod_amount;
                }
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
