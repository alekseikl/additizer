use core::f32;
use std::collections::{HashMap, HashSet};

use itertools::{Itertools, izip};
use nih_plug::util::db_to_gain_fast;
use smallvec::SmallVec;
use topo_sort::{SortResults, TopoSort};

use crate::synth_engine::{
    buffer::{Buffer, HARMONIC_SERIES_BUFFER, SpectralBuffer, ZEROES_BUFFER, make_zero_buffer},
    modules_container::ModulesContainer,
    routing::{
        MAX_VOICES, ModuleId, ModuleInput, ModuleInputSource, ModuleLink, ModuleOutput, Router,
        RoutingNode,
    },
    synth_module::{BufferOutputModule, NoteOffParams, NoteOnParams, ProcessParams, SynthModule},
    types::{ComplexSample, StereoValue},
};

pub mod buffer;
pub mod context;
pub mod envelope;
pub mod modules;
pub mod modules_container;
pub mod routing;
pub mod synth_module;
pub mod types;

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

struct Modules {
    oscillators: ModulesContainer<modules::Oscillator>,
    envelopes: ModulesContainer<modules::Envelope>,
    amplifiers: ModulesContainer<modules::Amplifier>,
}

impl Modules {
    fn new() -> Self {
        Self {
            oscillators: ModulesContainer::new(),
            envelopes: ModulesContainer::new(),
            amplifiers: ModulesContainer::new(),
        }
    }

    fn resolve_node(&self, node: RoutingNode) -> Option<&dyn BufferOutputModule> {
        match node {
            RoutingNode::Oscillator(id) => self
                .oscillators
                .get(id)
                .map(|module| module as &dyn BufferOutputModule),
            RoutingNode::Envelope(id) => self
                .envelopes
                .get(id)
                .map(|module| module as &dyn BufferOutputModule),
            RoutingNode::Amplifier(id) => self
                .amplifiers
                .get(id)
                .map(|module| module as &dyn BufferOutputModule),
            RoutingNode::Output => panic!("RoutingNode::Output don't have corresponding module."),
        }
    }

    fn resolve_node_mut(&mut self, node: RoutingNode) -> Option<&mut dyn SynthModule> {
        match node {
            RoutingNode::Oscillator(id) => self
                .oscillators
                .get_mut(id)
                .map(|module| module as &mut dyn SynthModule),
            RoutingNode::Envelope(id) => self
                .envelopes
                .get_mut(id)
                .map(|module| module as &mut dyn SynthModule),
            RoutingNode::Amplifier(id) => self
                .amplifiers
                .get_mut(id)
                .map(|module| module as &mut dyn SynthModule),
            RoutingNode::Output => panic!("RoutingNode::Output don't have corresponding module."),
        }
    }
}

pub struct SynthEngine {
    next_id: u64,
    next_voice_id: u64,
    sample_rate: f32,
    modules: Modules,
    input_sources: HashMap<ModuleInput, Vec<ModuleInputSource>>,
    execution_order: Vec<RoutingNode>,
    voices: [Voice; MAX_VOICES],
    spectral_buffer: SpectralBuffer,
    tmp_output_buffer: Option<Box<Buffer>>,
}

impl SynthEngine {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            next_voice_id: 1,
            sample_rate: 1000.0,
            modules: Modules::new(),
            input_sources: HashMap::new(),
            execution_order: Vec::new(),
            voices: Default::default(),
            spectral_buffer: HARMONIC_SERIES_BUFFER,
            tmp_output_buffer: Some(Box::new(make_zero_buffer())),
        }
    }

    fn alloc_next_id(&mut self) -> ModuleId {
        let module_id = self.next_id;

        self.next_id += 1;
        module_id
    }

    pub fn add_oscillator(&mut self, mut osc: modules::Oscillator) -> ModuleId {
        let id = self.alloc_next_id();

        osc.set_id(id);
        self.modules.oscillators.add(osc);
        id
    }

    pub fn add_envelope(&mut self, mut env: modules::Envelope) -> ModuleId {
        let id = self.alloc_next_id();

        env.set_id(id);
        self.modules.envelopes.add(env);
        id
    }

    pub fn add_amplifier(&mut self, mut amp: modules::Amplifier) -> ModuleId {
        let id = self.alloc_next_id();

        amp.set_id(id);
        self.modules.amplifiers.add(amp);
        id
    }

    pub fn set_links(&mut self, links: &[ModuleLink]) -> Result<(), String> {
        let execution_order = Self::calc_execution_order(links)?;
        let mut input_sources: HashMap<ModuleInput, Vec<ModuleInputSource>> = HashMap::new();

        for link in links {
            input_sources
                .entry(link.dst)
                .or_default()
                .push(ModuleInputSource {
                    src: link.src,
                    modulation_amount: link.modulation_amount,
                });
        }

        self.input_sources = input_sources;
        self.execution_order = execution_order;

        Ok(())
    }

    pub fn init(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.build_scheme();
    }

    pub fn note_on(
        &mut self,
        voice_id: Option<i32>,
        channel: u8,
        note: u8,
        velocity: f32,
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
            velocity,
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

        let params = NoteOffParams { note, voice_idx };

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
            active_voices: &active_voices,
        };

        for node in &self.execution_order {
            match node {
                RoutingNode::Oscillator(id) => {
                    let mut osc = self.modules.oscillators.take(*id);

                    osc.process(&params, self);
                    self.modules.oscillators.return_back(osc);
                }
                RoutingNode::Envelope(id) => {
                    let mut env = self.modules.envelopes.take(*id);

                    env.process(&params, self);
                    self.modules.envelopes.return_back(env);
                }
                RoutingNode::Amplifier(id) => {
                    let mut amp = self.modules.amplifiers.take(*id);

                    amp.process(&params, self);
                    self.modules.amplifiers.return_back(amp);
                }
                RoutingNode::Output => (),
            }
        }

        self.read_output(&params, outputs);
    }

    fn read_output<'a>(
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

    pub fn update_harmonics(&mut self, harmonics: &Vec<f32>, tail: f32) {
        let range = 1..(harmonics.len() + 1);

        for (out, series_factor, harmonic) in izip!(
            &mut self.spectral_buffer[range.clone()],
            &HARMONIC_SERIES_BUFFER[range],
            harmonics
        ) {
            *out = series_factor * harmonic;
        }

        let range = (harmonics.len() + 1)..self.spectral_buffer.len();

        for (out, series_factor) in self.spectral_buffer[range.clone()]
            .iter_mut()
            .zip(HARMONIC_SERIES_BUFFER[range].iter())
        {
            *out = *series_factor * tail;
        }

        self.spectral_buffer[0] = ComplexSample::ZERO;
    }

    pub fn set_volume(&mut self, volume: f32) {
        let level = db_to_gain_fast(volume);

        for amp in self.modules.amplifiers.modules.values_mut() {
            amp.as_mut().unwrap().set_level(StereoValue::mono(level));
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

    fn build_scheme(&mut self) {
        let mut osc = modules::Oscillator::new();
        // let mut detune_env = EnvelopeModule::new();
        // let mut pitch_env = EnvelopeModule::new();
        let mut amp_env = modules::Envelope::new();
        let amp = modules::Amplifier::new();

        osc.set_unison(3).set_detune(0.01.into());
        // pitch_env
        //     .set_attack(50.0)
        //     .set_decay(50.0)
        //     .set_sustain(1.0)
        //     .set_release(500.0)
        //     .set_channel_sustain(1, 0.5);
        // detune_env
        //     .set_attack(1000.0)
        //     .set_decay(0.0)
        //     .set_sustain(1.0)
        //     .set_channel_sustain(1, 0.5)
        //     .set_release(10000.0)
        //     .set_keep_voice_alive(false);
        amp_env
            .set_attack(StereoValue::mono(10.0))
            .set_decay(20.0.into())
            .set_sustain(1.0.into())
            .set_release(300.0.into());

        let osc_id = self.add_oscillator(osc);
        // let detune_env_id = self.add_envelope(detune_env);
        // let pitch_shift_env_id = self.add_envelope(pitch_env);
        let amp_id = self.add_amplifier(amp);
        let amp_env_id = self.add_envelope(amp_env);

        self.set_links(&[
            ModuleLink::link(ModuleOutput::Amplifier(amp_id), ModuleInput::Output),
            ModuleLink::link(
                ModuleOutput::Oscillator(osc_id),
                ModuleInput::AmplifierInput(amp_id),
            ),
            // ModuleLink::modulation(
            //     ModuleOutput::Envelope(pitch_shift_env_id),
            //     ModuleInput::OscillatorPitchShift(osc_id),
            //     0.125,
            // ),
            // ModuleLink::modulation(
            //     ModuleOutput::Envelope(detune_env_id),
            //     ModuleInput::OscillatorDetune(osc_id),
            //     0.6,
            // ),
            ModuleLink::link(
                ModuleOutput::Envelope(amp_env_id),
                ModuleInput::AmplifierLevel(amp_id),
            ),
        ])
        .unwrap();
    }

    fn resolve_buffer(&self, output: ModuleOutput, voice_idx: usize, channel: usize) -> &Buffer {
        self.modules
            .resolve_node(output.routing_node())
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

        if sources.len() == 1 && sources[0].modulation_amount.is_none() {
            return Some(self.resolve_buffer(sources[0].src, voice_idx, channel));
        }

        input_buffer.fill(0.0);

        let buffs = sources.iter().map(|source| {
            (
                self.resolve_buffer(source.src, voice_idx, channel),
                source.modulation_amount,
            )
        });

        for (buff, mod_amount) in buffs {
            let mod_amount = mod_amount.unwrap_or(1.0);

            for (input, buff) in input_buffer.iter_mut().zip(buff) {
                *input += buff * mod_amount;
            }
        }

        Some(input_buffer)
    }

    fn get_spectral_input(
        &self,
        _voice_idx: usize,
        _channel: usize,
    ) -> Option<(&SpectralBuffer, &SpectralBuffer)> {
        Some((&self.spectral_buffer, &self.spectral_buffer))
    }
}
