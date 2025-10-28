use core::f32;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use itertools::{Itertools, izip};
use nih_plug::util::db_to_gain_fast;
use smallvec::SmallVec;
use topo_sort::{SortResults, TopoSort};

use crate::synth_engine::{
    buffer::{Buffer, ZEROES_BUFFER, make_zero_buffer},
    modules_container::ModulesContainer,
    routing::{
        MAX_VOICES, ModuleId, ModuleInput, ModuleInputSource, ModuleLink, ModuleOutput, Router,
        RoutingNode,
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
mod modules_container;
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
    next_id: u64,
    next_voice_id: u64,
    sample_rate: f32,
    config: Arc<Config>,
    modules: ModulesContainer,
    input_sources: HashMap<ModuleInput, Vec<ModuleInputSource>>,
    execution_order: Vec<RoutingNode>,
    voices: [Voice; MAX_VOICES],
    tmp_output_buffer: Option<Box<Buffer>>,
}

macro_rules! create_node_method {
    ($func_name:ident, $node_type:ident) => {
        pub fn $func_name(&mut self) -> Result<ModuleId, String> {
            let id = self.alloc_next_id();
            let node = RoutingNode::$node_type(id);
            let module = self.modules.add_node(&node, &self.config)?;

            Self::trigger_active_notes(&self.voices, module);
            self.save_config();
            Ok(id)
        }
    };
}

impl SynthEngine {
    pub fn new(config: Arc<Config>) -> Self {
        Self {
            next_id: 1,
            next_voice_id: 1,
            sample_rate: 1000.0,
            config,
            modules: ModulesContainer::new(),
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

    create_node_method!(add_oscillator, Oscillator);
    create_node_method!(add_envelope, Envelope);
    create_node_method!(add_amplifier, Amplifier);
    create_node_method!(add_spectral_filter, SpectralFilter);

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

        if !self.modules.is_node_exists(link.src.routing_node())
            || !self.modules.is_node_exists(link.dst.routing_node())
        {
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

        for node in &self.execution_order {
            self.modules
                .resolve_node_mut(*node)
                .unwrap()
                .note_on(&params);
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

        for node in &self.execution_order {
            self.modules
                .resolve_node_mut(*node)
                .unwrap()
                .note_off(&params);
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

        for env in self.modules.envelopes.modules.values() {
            env.as_ref().unwrap().check_activity(&mut env_activity);
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

        macro_rules! process_node {
            ($id:ident, $container:ident) => {{
                let mut module = self.modules.$container.take(*$id);

                module.process(&params, self);
                self.modules.$container.return_back(module);
            }};
        }

        for node in &self.execution_order {
            match node {
                RoutingNode::Oscillator(id) => process_node!(id, oscillators),
                RoutingNode::Envelope(id) => process_node!(id, envelopes),
                RoutingNode::Amplifier(id) => process_node!(id, amplifiers),
                RoutingNode::SpectralFilter(id) => {
                    process_node!(id, spectral_filters)
                }
                RoutingNode::Output => (),
            }
        }

        self.write_output(&params, outputs);
    }

    fn alloc_next_id(&mut self) -> ModuleId {
        let module_id = self.next_id;

        self.next_id += 1;
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
                    .get_input(ModuleInput::Output, *voice_idx, channel, &mut tmp_buffer)
                    .unwrap_or(&ZEROES_BUFFER);

                for (out, input, _) in izip!(output.iter_mut(), input, 0..params.samples) {
                    *out += input;
                }
            }
        }

        self.tmp_output_buffer.replace(tmp_buffer);
    }

    pub fn update_harmonics(&mut self, harmonics: &[StereoSample], tail: StereoSample) {
        for filter in &mut self.modules.spectral_filters.modules.values_mut() {
            filter.as_mut().unwrap().set_harmonics(harmonics, tail);
        }
    }

    pub fn set_volume(&mut self, volume: f32) {
        let level = db_to_gain_fast(volume);

        for amp in self.modules.amplifiers.modules.values_mut() {
            amp.as_mut().unwrap().set_level(StereoSample::mono(level));
        }
    }

    pub fn set_unison(&mut self, unison: usize) {
        for osc in self.modules.oscillators.modules.values_mut() {
            osc.as_mut().unwrap().set_unison(unison);
        }
    }

    pub fn set_detune(&mut self, detune: f32) {
        let detune = 0.01 * detune;

        for osc in self.modules.oscillators.modules.values_mut() {
            osc.as_mut().unwrap().set_detune(detune.into());
        }
    }

    pub fn set_cutoff(&mut self, cutoff: Sample) {
        for filter in self.modules.spectral_filters.modules.values_mut() {
            filter.as_mut().unwrap().set_cutoff_harmonic(cutoff.into());
        }
    }

    fn build_scheme(&mut self) {
        let filter_env_id = self.add_envelope().unwrap();
        let filter_id = self.add_spectral_filter().unwrap();
        let osc_id = self.add_oscillator().unwrap();
        let amp_id = self.add_amplifier().unwrap();
        let amp_env_id = self.add_envelope().unwrap();

        self.modules
            .envelopes
            .get_mut(filter_env_id)
            .unwrap()
            .set_attack(0.0.into())
            .set_decay(500.0.into())
            .set_sustain(0.0.into())
            .set_release(100.0.into());

        self.modules
            .spectral_filters
            .get_mut(filter_id)
            .unwrap()
            .set_cutoff_harmonic(2.0.into());

        self.modules
            .oscillators
            .get_mut(osc_id)
            .unwrap()
            .set_unison(3)
            .set_detune(0.01.into());

        self.modules
            .envelopes
            .get_mut(amp_env_id)
            .unwrap()
            .set_attack(StereoSample::mono(10.0))
            .set_decay(20.0.into())
            .set_sustain(1.0.into())
            .set_release(300.0.into());

        self.set_link(ModuleLink::link(
            ModuleOutput::Amplifier(amp_id),
            ModuleInput::Output,
        ))
        .unwrap();
        self.set_link(ModuleLink::modulation(
            ModuleOutput::EnvelopeScalar(filter_env_id),
            ModuleInput::SpectralFilterCutoff(filter_id),
            50.0,
        ))
        .unwrap();
        self.set_link(ModuleLink::link(
            ModuleOutput::SpectralFilter(filter_id),
            ModuleInput::OscillatorSpectrum(osc_id),
        ))
        .unwrap();
        self.set_link(ModuleLink::link(
            ModuleOutput::Oscillator(osc_id),
            ModuleInput::AmplifierInput(amp_id),
        ))
        .unwrap();
        self.set_link(ModuleLink::link(
            ModuleOutput::Envelope(amp_env_id),
            ModuleInput::AmplifierLevel(amp_id),
        ))
        .unwrap();
    }

    fn resolve_buffer(&self, output: ModuleOutput, voice_idx: usize, channel: usize) -> &Buffer {
        self.modules
            .resolve_buffer_output_node(output.routing_node())
            .unwrap()
            .get_output(voice_idx, channel)
    }

    fn calc_execution_order(links: &[ModuleLink]) -> Result<Vec<RoutingNode>, String> {
        let mut dependents: HashMap<RoutingNode, HashSet<RoutingNode>> = HashMap::new();

        for link in links {
            let src_node = link.src.routing_node();
            let dst_node = link.dst.routing_node();

            dependents.entry(dst_node).or_default().insert(src_node);
            dependents.entry(src_node).or_default();
        }

        let topo_sort = TopoSort::from_map(dependents);

        match topo_sort.into_vec_nodes() {
            SortResults::Full(nodes) => Ok(nodes
                .into_iter()
                .filter(|node| *node != RoutingNode::Output)
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
        let config = self.config.routing.lock();
        let nodes = config.nodes.clone();
        let links = config.links.clone();

        self.next_id = config.last_module_id + 1;
        drop(config);

        if nodes.is_empty() {
            return false;
        }

        for node in &nodes {
            if self.modules.add_node(node, &self.config).is_err() {
                return false;
            }
        }

        self.setup_routing(&links).is_ok()
    }

    fn save_config(&self) {
        let mut config = self.config.routing.lock();

        config.last_module_id = self.next_id;
        config.nodes = self.modules.get_routing_nodes();
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
            return Some(self.resolve_buffer(sources[0].src, voice_idx, channel));
        }

        let buffs = sources.iter().map(|source| {
            (
                self.resolve_buffer(source.src, voice_idx, channel),
                source.modulation,
            )
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

        let module = self
            .modules
            .resolve_spectral_output_node(sources[0].src.routing_node());

        Some(module.unwrap().get_output(voice_idx, channel))
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

        let values = sources.iter().map(|source| {
            (
                self.modules
                    .resolve_scalar_output_node(source.src.routing_node())
                    .unwrap()
                    .get_output(voice_idx, channel),
                source.modulation,
            )
        });

        for (value, mod_amount) in values {
            let mod_amount = mod_amount.map_or(1.0, |stereo_amount| stereo_amount[channel]);

            outputs.first += value.first * mod_amount;
            outputs.current += value.current * mod_amount;
        }

        Some(outputs)
    }
}
