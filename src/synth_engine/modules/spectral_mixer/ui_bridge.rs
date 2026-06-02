use std::sync::Arc;

use parking_lot::Mutex;

use crate::synth_engine::{Input, MixType, ModuleId, StereoSample, SynthEngine, VolumeType};

use super::{InputParams, SpectralMixer};
use super::link::UiEnd;

#[derive(Clone)]
pub struct ControlsState {
    pub num_inputs: u8,
    pub input_params: [InputParams; super::MAX_INPUTS as usize],
    pub input_levels: [StereoSample; super::MAX_INPUTS as usize],
    pub input_gains: [StereoSample; super::MAX_INPUTS as usize],
    pub output_volume_type: VolumeType,
    pub output_level: StereoSample,
    pub output_gain: StereoSample,
}

pub struct UiBridge {
    synth: Arc<Mutex<SynthEngine>>,
    module_id: ModuleId,
    ui_end: Option<UiEnd>,
    controls: ControlsState,
}

impl UiBridge {
    pub fn create(module_id: ModuleId, synth: Arc<Mutex<SynthEngine>>) -> Option<Self> {
        let mut synth_lock = synth.lock();
        let mixer = synth_lock.get_typed_module_mut::<SpectralMixer>(module_id)?;
        let ui_end = mixer.take_ui_end()?;
        let controls = mixer.get_controls_state();

        drop(synth_lock);

        Some(Self {
            synth,
            module_id,
            ui_end: Some(ui_end),
            controls,
        })
    }

    pub fn module_id(&self) -> ModuleId {
        self.module_id
    }

    pub fn controls(&self) -> &ControlsState {
        &self.controls
    }

    pub fn set_param(&mut self, input: Input, value: StereoSample) {
        if !self.ui_end.as_mut().unwrap().set_param(input, value) {
            return;
        }

        match input {
            Input::Gain => self.controls.output_gain = value,
            Input::Level => self.controls.output_level = value,
            Input::GainMix(idx) => self.controls.input_gains[idx as usize] = value,
            Input::LevelMix(idx) => self.controls.input_levels[idx as usize] = value,
            _ => (),
        }
    }

    pub fn set_num_inputs(&mut self, num_inputs: u8) {
        if self.ui_end.as_mut().unwrap().set_num_inputs(num_inputs) {
            self.controls.num_inputs = num_inputs;
        }
    }

    pub fn set_mix_type(&mut self, input_idx: u8, mix_type: MixType) {
        if self.ui_end.as_mut().unwrap().set_mix_type(input_idx, mix_type) {
            self.controls.input_params[input_idx as usize].mix_type = mix_type;
        }
    }

    pub fn set_volume_type(&mut self, input_idx: u8, volume_type: VolumeType) {
        if self
            .ui_end
            .as_mut()
            .unwrap()
            .set_volume_type(input_idx, volume_type)
        {
            self.controls.input_params[input_idx as usize].volume_type = volume_type;
        }
    }

    pub fn set_output_volume_type(&mut self, volume_type: VolumeType) {
        if self
            .ui_end
            .as_mut()
            .unwrap()
            .set_output_volume_type(volume_type)
        {
            self.controls.output_volume_type = volume_type;
        }
    }
}

impl Drop for UiBridge {
    fn drop(&mut self) {
        let mut synth_lock = self.synth.lock();

        if let Some(mixer) = synth_lock.get_typed_module_mut::<SpectralMixer>(self.module_id) {
            mixer.return_ui_end(self.ui_end.take().unwrap());
        }
    }
}
