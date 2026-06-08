use std::sync::Arc;

use crate::{
    engine_factory::{EngineFactory, EngineHandle, UiConfigHandle},
    synth_engine::{
        Input, ModuleId, ModuleInput, ModuleType, OUTPUT_MODULE_ID, Sample, StereoSample,
        config::EngineParams,
        routing::{DataType, data_types_compatible},
    },
};

mod link;
pub mod routing_state;
pub mod ui_config;

pub use link::{AudioEnd, UiEnd, UiEvent, UiUpdate, create_link_pair};
pub use routing_state::{AvailableInputSource, ConnectedInputSource, RoutingState};
use rustc_hash::FxHashMap;

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
    pub label: String,
}

pub struct UiBridge {
    factory: Arc<EngineFactory>,
    engine: EngineHandle,
    ui_config: UiConfigHandle,
    ui_end: Option<UiEnd>,
    routing: RoutingState,
    engine_params: EngineParams,
    voices: VoicesStatus,
    modulated_inputs: FxHashMap<ModuleInput, StereoSample>,
    outputs: FxHashMap<ModuleId, StereoSample>,
}

impl UiBridge {
    pub fn create(factory: Arc<EngineFactory>) -> Option<Self> {
        let engine = factory.get_engine();
        let ui_config = factory.get_ui_config();
        let mut engine_lock = engine.lock();

        let ui_end = engine_lock.ui_end.take()?;
        let routing = engine_lock.get_routing_state();
        let engine_params = engine_lock.get_engine_params();

        drop(engine_lock);

        Some(Self {
            factory,
            engine,
            ui_config,
            ui_end: Some(ui_end),
            routing,
            engine_params,
            voices: VoicesStatus::default(),
            modulated_inputs: FxHashMap::default(),
            outputs: FxHashMap::default(),
        })
    }

    pub fn engine(&self) -> &EngineHandle {
        &self.engine
    }

    // pub fn with_synth<R, F>(&self, f: F) -> R
    // where
    //     F: FnOnce(&mut SynthEngine) -> R,
    // {
    //     f(&mut self.synth.lock())
    // }

    pub fn engine_params(&self) -> &EngineParams {
        &self.engine_params
    }

    pub fn voices_status(&self) -> &VoicesStatus {
        &self.voices
    }

    fn module_label(ui_config: &ui_config::UiConfig, module_id: ModuleId) -> String {
        ui_config
            .modules
            .get(&module_id)
            .map(|module| module.label.clone())
            .unwrap_or_default()
    }

    pub fn get_modules(&self) -> Vec<ModuleItem> {
        let ui_config = self.ui_config.lock();

        self.routing
            .modules
            .values()
            .map(|m| ModuleItem {
                id: m.id,
                module_type: m.module_type,
                label: Self::module_label(&ui_config, m.id),
            })
            .collect()
    }

    pub fn has_module_id(&self, module_id: ModuleId) -> bool {
        self.routing.modules.contains_key(&module_id)
    }

    pub fn get_module_label(&self, module_id: ModuleId) -> String {
        let ui_config = self.ui_config.lock();
        Self::module_label(&ui_config, module_id)
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

    pub fn get_available_input_sources(&self, input: ModuleInput) -> Vec<AvailableInputSource> {
        let ui_config = self.ui_config.lock();

        let dst_data_type =
            if input.module_id == OUTPUT_MODULE_ID && input.input_type == Input::Audio {
                DataType::Buffer
            } else if let Some(input_module) = self.routing.modules.get(&input.module_id)
                && let Some(input_info) = input_module
                    .inputs
                    .iter()
                    .find(|input_info| input_info.input == input.input_type)
            {
                input_info.data_type
            } else {
                return Vec::new();
            };

        self.routing
            .modules
            .values()
            .filter(|module| {
                module.id != input.module_id
                    && data_types_compatible(module.output, dst_data_type)
                    && !self.is_connected_to_source(module.id, input.module_id)
            })
            .map(|module| AvailableInputSource {
                src: module.id,
                label: Self::module_label(&ui_config, module.id),
            })
            .collect()
    }

    pub fn get_connected_input_sources(&self, input: ModuleInput) -> Vec<ConnectedInputSource> {
        let ui_config = self.ui_config.lock();

        let Some(sources) = self.routing.routing.get(&input) else {
            return Vec::new();
        };

        sources
            .iter()
            .filter_map(|source| {
                self.routing
                    .modules
                    .get(&source.src)
                    .map(|module| (module, source))
            })
            .map(|(_module, source)| ConnectedInputSource {
                src: source.src,
                amount: source.amount,
                label: Self::module_label(&ui_config, source.src),
                modulation: source
                    .modulation
                    .map(|modulation| routing_state::InputModulation {
                        src: modulation.src,
                        label: Self::module_label(&ui_config, modulation.src),
                    }),
            })
            .collect()
    }

    pub fn get_input_modulated_value(&self, input: ModuleInput) -> Option<StereoSample> {
        if self.routing.routing.contains_key(&input) && self.has_active_voices() {
            self.modulated_inputs.get(&input).copied()
        } else {
            None
        }
    }

    fn is_connected_to_source(&self, dst_id: ModuleId, src_id: ModuleId) -> bool {
        for (input, sources) in &self.routing.routing {
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

    fn sync(&mut self) {
        let synth = self.engine.lock();

        self.routing = synth.get_routing_state();
        self.engine_params = synth.get_engine_params();
        self.voices = VoicesStatus::default();
    }

    // Returns true when UI needs to be reset
    pub fn update(&mut self) -> bool {
        let mut need_reset = false;

        if self.factory.engine_changed(&self.engine) {
            self.engine = self.factory.get_engine();
            self.ui_config = self.factory.get_ui_config();
            self.sync();
            need_reset = true;
        }

        let ui_end = self.ui_end.as_mut().unwrap();

        while let Some(update) = ui_end.pop_update() {
            match update {
                UiUpdate::ModulatedInput {
                    module_id,
                    input,
                    channel,
                    value,
                } => {
                    self.modulated_inputs
                        .entry(ModuleInput::new(input, module_id))
                        .or_insert(StereoSample::ZERO)[channel as usize] = value;
                }
                UiUpdate::Output {
                    module_id,
                    channel,
                    value,
                } => {
                    self.outputs.entry(module_id).or_insert(StereoSample::ZERO)[channel as usize] =
                        value;
                }
                UiUpdate::VoicesStatus(status) => self.voices = status,
            }
        }

        need_reset
    }

    pub fn add_module(&mut self, module_type: ModuleType) -> ModuleId {
        let mut synth = self.engine.lock();

        let result = match module_type {
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
        result
    }

    pub fn remove_module(&mut self, module_id: ModuleId) {
        let mut synth = self.engine.lock();

        synth.remove_module(module_id);
        self.routing = synth.get_routing_state();
    }

    pub fn set_direct_link(&mut self, src: ModuleId, dst: ModuleInput) {
        let mut synth = self.engine.lock();

        let _ = synth.set_direct_link(src, dst);
        self.routing = synth.get_routing_state();
    }

    pub fn add_link(&mut self, src: ModuleId, dst: ModuleInput, amount: StereoSample) {
        let mut synth = self.engine.lock();

        if let Err(err) = synth.add_link(src, dst, amount) {
            println!("Failed to add link: {err}");
        }
        self.routing = synth.get_routing_state();
    }

    pub fn remove_link(&mut self, src: ModuleId, dst: ModuleInput) {
        let mut synth = self.engine.lock();

        synth.remove_link(&src, &dst);
        self.routing = synth.get_routing_state();
    }

    pub fn set_link_modulation(
        &mut self,
        src_id: ModuleId,
        dst_input: &ModuleInput,
        modulator_id: ModuleId,
    ) {
        let mut synth = self.engine.lock();

        let _ = synth.set_link_modulation(src_id, dst_input, modulator_id);
        self.routing = synth.get_routing_state();
    }

    pub fn remove_link_modulation(&mut self, src_id: ModuleId, dst_input: &ModuleInput) {
        let mut synth = self.engine.lock();

        synth.remove_link_modulation(src_id, dst_input);
        self.routing = synth.get_routing_state();
    }

    pub fn set_link_amount(&mut self, src: ModuleId, dst: ModuleInput, amount: StereoSample) {
        if self
            .ui_end
            .as_mut()
            .unwrap()
            .set_link_amount(src, dst, amount)
            && let Some(sources) = self.routing.routing.get_mut(&dst)
            && let Some(source) = sources.iter_mut().find(|s| s.src == src)
        {
            source.amount = amount;
        }
    }

    pub fn set_voices(&mut self, voices: usize) {
        if self.ui_end.as_mut().unwrap().set_voices(voices) {
            self.engine_params.num_voices = voices;
        }
    }

    pub fn set_legato(&mut self, legato: bool) {
        if self.ui_end.as_mut().unwrap().set_legato(legato) {
            self.engine_params.legato = legato;
        }
    }

    pub fn set_block_size(&mut self, block_size: usize) {
        if self.ui_end.as_mut().unwrap().set_block_size(block_size) {
            self.engine_params.block_size = block_size;
        }
    }

    pub fn set_voice_kill_time(&mut self, voice_kill_time: Sample) {
        if self
            .ui_end
            .as_mut()
            .unwrap()
            .set_voice_kill_time(voice_kill_time)
        {
            self.engine_params.voice_kill_time = voice_kill_time;
        }
    }

    pub fn set_oversampling(&mut self, oversampling: bool) {
        if self.ui_end.as_mut().unwrap().set_oversampling(oversampling) {
            self.engine_params.oversampling = oversampling;
        }
    }

    pub fn set_stereo_spectrum(&mut self, stereo_spectrum: bool) {
        if self
            .ui_end
            .as_mut()
            .unwrap()
            .set_stereo_spectrum(stereo_spectrum)
        {
            self.engine_params.stereo_spectrum = stereo_spectrum;
        }
    }

    pub fn set_output_gain(&mut self, output_gain: StereoSample) {
        if self.ui_end.as_mut().unwrap().set_output_gain(output_gain) {
            self.engine_params.output_gain = output_gain;
        }
    }
}

impl Drop for UiBridge {
    fn drop(&mut self) {
        let mut synth_lock = self.engine.lock();

        assert!(synth_lock.ui_end.is_none());

        synth_lock.ui_end = Some(self.ui_end.take().unwrap());
    }
}
