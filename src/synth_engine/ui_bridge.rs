use std::sync::Arc;

use crate::synth_engine::{
    ModuleId, ModuleInput, ModuleType, OUTPUT_MODULE_ID, Sample, StereoSample, SynthEngine,
};

mod link;
pub mod routing_state;

pub use link::{AudioEnd, UiEnd, UiEvent, UiUpdate, create_link_pair};
use parking_lot::Mutex;
pub use routing_state::{AvailableInputSource, ConnectedInputSource, ModuleItem, RoutingState};
use rustc_hash::FxHashMap;

pub struct ControlsState {
    pub voices: usize,
    pub legato: bool,
    pub block_size: usize,
    pub voice_kill_time: Sample,
    pub oversampling: bool,
    pub stereo_spectrum: bool,
    pub output_gain: StereoSample,
}

#[derive(Clone, Copy, Default)]
pub struct VoicesStatus {
    pub waiting_notes: u8,
    pub playing: u8,
    pub releasing: u8,
    pub killing: u8,
}

pub struct UiBridge {
    synth: Arc<Mutex<SynthEngine>>,
    ui_end: Option<UiEnd>,
    routing: RoutingState,
    controls: ControlsState,
    voices: VoicesStatus,
    modulated_inputs: FxHashMap<ModuleInput, StereoSample>,
    outputs: FxHashMap<ModuleId, StereoSample>,
}

impl UiBridge {
    pub fn create(synth: Arc<Mutex<SynthEngine>>) -> Option<Self> {
        let mut synth_lock = synth.lock();

        let ui_end = synth_lock.ui_end.take()?;
        let routing = synth_lock.get_routing_state();
        let controls = synth_lock.get_ui_state();

        drop(synth_lock);

        Some(Self {
            synth,
            ui_end: Some(ui_end),
            routing,
            controls,
            voices: VoicesStatus::default(),
            modulated_inputs: FxHashMap::default(),
            outputs: FxHashMap::default(),
        })
    }

    pub fn synth(&self) -> &Arc<Mutex<SynthEngine>> {
        &self.synth
    }

    // pub fn with_synth<R, F>(&self, f: F) -> R
    // where
    //     F: FnOnce(&mut SynthEngine) -> R,
    // {
    //     f(&mut self.synth.lock())
    // }

    pub fn controls(&self) -> &ControlsState {
        &self.controls
    }

    pub fn voices_status(&self) -> &VoicesStatus {
        &self.voices
    }

    pub fn get_modules(&self) -> Vec<ModuleItem> {
        self.routing.get_modules()
    }

    pub fn has_module_id(&self, module_id: ModuleId) -> bool {
        self.routing.has_module_id(module_id)
    }

    pub fn get_module_label(&self, module_id: ModuleId) -> String {
        self.routing.get_module_label(module_id)
    }

    pub fn set_module_label(&mut self, module_id: ModuleId, label: String) {
        let mut synth = self.synth.lock();

        if let Some(m) = synth.get_module_mut(module_id) {
            m.set_label(label.clone());
            self.routing.set_module_label(module_id, label);
        }
    }

    pub fn has_active_voices(&self) -> bool {
        self.voices.playing + self.voices.releasing > 0
    }

    pub fn get_available_input_sources(&self, input: ModuleInput) -> Vec<AvailableInputSource> {
        self.routing.get_available_input_sources(input)
    }

    pub fn get_connected_input_sources(&self, input: ModuleInput) -> Vec<ConnectedInputSource> {
        self.routing.get_connected_input_sources(input)
    }

    pub fn get_input_modulated_value(&self, input: ModuleInput) -> Option<StereoSample> {
        if self.routing.has_input(input) && self.has_active_voices() {
            self.modulated_inputs.get(&input).copied()
        } else {
            None
        }
    }

    pub fn sync(&mut self) {
        let synth = self.synth.lock();

        self.routing = synth.get_routing_state();
        self.controls = synth.get_ui_state();
    }

    pub fn update(&mut self) {
        let Some(ui_end) = self.ui_end.as_mut() else {
            return;
        };

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
    }

    pub fn add_module(&mut self, module_type: ModuleType) -> ModuleId {
        let mut synth = self.synth.lock();

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
        let mut synth = self.synth.lock();

        synth.remove_module(module_id);
        self.routing = synth.get_routing_state();
    }

    pub fn set_direct_link(&mut self, src: ModuleId, dst: ModuleInput) {
        let mut synth = self.synth.lock();

        let _ = synth.set_direct_link(src, dst);
        self.routing = synth.get_routing_state();
    }

    pub fn add_link(&mut self, src: ModuleId, dst: ModuleInput, amount: StereoSample) {
        let mut synth = self.synth.lock();

        if let Err(err) = synth.add_link(src, dst, amount) {
            println!("Failed to add link: {err}");
        }
        self.routing = synth.get_routing_state();
    }

    pub fn remove_link(&mut self, src: ModuleId, dst: ModuleInput) {
        let mut synth = self.synth.lock();

        synth.remove_link(&src, &dst);
        self.routing = synth.get_routing_state();
    }

    pub fn set_link_modulation(
        &mut self,
        src_id: ModuleId,
        dst_input: &ModuleInput,
        modulator_id: ModuleId,
    ) {
        let mut synth = self.synth.lock();

        let _ = synth.set_link_modulation(src_id, dst_input, modulator_id);
        self.routing = synth.get_routing_state();
    }

    pub fn remove_link_modulation(&mut self, src_id: ModuleId, dst_input: &ModuleInput) {
        let mut synth = self.synth.lock();

        synth.remove_link_modulation(src_id, dst_input);
        self.routing = synth.get_routing_state();
    }

    pub fn set_link_amount(&mut self, src: ModuleId, dst: ModuleInput, amount: StereoSample) {
        if self
            .ui_end
            .as_mut()
            .unwrap()
            .set_link_amount(src, dst, amount)
        {
            self.routing.update_link_amount(src, dst, amount);
        }
    }

    pub fn set_voices(&mut self, voices: usize) {
        if self.ui_end.as_mut().unwrap().set_voices(voices) {
            self.controls.voices = voices;
        }
    }

    pub fn set_legato(&mut self, legato: bool) {
        if self.ui_end.as_mut().unwrap().set_legato(legato) {
            self.controls.legato = legato;
        }
    }

    pub fn set_block_size(&mut self, block_size: usize) {
        if self.ui_end.as_mut().unwrap().set_block_size(block_size) {
            self.controls.block_size = block_size;
        }
    }

    pub fn set_voice_kill_time(&mut self, voice_kill_time: Sample) {
        if self
            .ui_end
            .as_mut()
            .unwrap()
            .set_voice_kill_time(voice_kill_time)
        {
            self.controls.voice_kill_time = voice_kill_time;
        }
    }

    pub fn set_oversampling(&mut self, oversampling: bool) {
        if self.ui_end.as_mut().unwrap().set_oversampling(oversampling) {
            self.controls.oversampling = oversampling;
        }
    }

    pub fn set_stereo_spectrum(&mut self, stereo_spectrum: bool) {
        if self
            .ui_end
            .as_mut()
            .unwrap()
            .set_stereo_spectrum(stereo_spectrum)
        {
            self.controls.stereo_spectrum = stereo_spectrum;
        }
    }

    pub fn set_output_gain(&mut self, output_gain: StereoSample) {
        if self.ui_end.as_mut().unwrap().set_output_gain(output_gain) {
            self.controls.output_gain = output_gain;
        }
    }
}

impl Drop for UiBridge {
    fn drop(&mut self) {
        let mut synth_lock = self.synth.lock();

        assert!(synth_lock.ui_end.is_none());

        synth_lock.ui_end = Some(self.ui_end.take().unwrap());
    }
}
