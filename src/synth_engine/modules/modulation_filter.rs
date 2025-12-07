use biquad::{Biquad, Coefficients, DirectForm1, Q_BUTTERWORTH_F32, ToHertz};
use serde::{Deserialize, Serialize};
use std::any::Any;

use crate::synth_engine::{
    Input, ModuleId, ModuleInput, ModuleType, Sample, SynthModule,
    buffer::{Buffer, ZEROES_BUFFER, zero_buffer},
    routing::{DataType, MAX_VOICES, NUM_CHANNELS, Router},
    synth_module::{InputInfo, ModuleConfigBox, NoteOnParams, ProcessParams},
};

#[derive(Clone, Serialize, Deserialize)]
pub struct ModulationFilterConfig {
    label: Option<String>,
    cutoff_frequency: Sample,
}

impl Default for ModulationFilterConfig {
    fn default() -> Self {
        Self {
            label: None,
            cutoff_frequency: 1_000.0,
        }
    }
}

pub struct ModulationFilterUI {
    pub label: String,
    pub cutoff_frequency: Sample,
}

struct Voice {
    filter: DirectForm1<Sample>,
    current_cutoff: Sample,
    output: Buffer,
}

impl Default for Voice {
    fn default() -> Self {
        let coeffs = Coefficients::<Sample>::from_params(
            biquad::Type::LowPass,
            1000.hz(),
            10.hz(),
            Q_BUTTERWORTH_F32,
        )
        .unwrap();

        Self {
            filter: DirectForm1::new(coeffs),
            current_cutoff: -1.0,
            output: zero_buffer(),
        }
    }
}

#[derive(Default)]
struct Channel {
    voices: [Voice; MAX_VOICES],
}

pub struct ModulationFilter {
    id: ModuleId,
    label: String,
    config: ModuleConfigBox<ModulationFilterConfig>,
    cutoff_frequency: Sample,
    channels: [Channel; NUM_CHANNELS],
    input_buffer: Buffer,
}

impl ModulationFilter {
    pub fn new(id: ModuleId, config: ModuleConfigBox<ModulationFilterConfig>) -> Self {
        let mut filter = Self {
            id,
            label: format!("Modulation Filter {id}"),
            config,
            cutoff_frequency: 0.0,
            channels: Default::default(),
            input_buffer: zero_buffer(),
        };

        {
            let cfg = filter.config.lock();

            if let Some(label) = cfg.label.as_ref() {
                filter.label = label.clone();
            }

            filter.cutoff_frequency = cfg.cutoff_frequency;
        }

        filter
    }

    gen_downcast_methods!();

    pub fn get_ui(&self) -> ModulationFilterUI {
        ModulationFilterUI {
            label: self.label.clone(),
            cutoff_frequency: self.cutoff_frequency,
        }
    }

    pub fn set_cutoff_frequency(&mut self, cutoff: Sample) {
        self.cutoff_frequency = cutoff.clamp(50.0, 2_500.0);
        self.config.lock().cutoff_frequency = self.cutoff_frequency;
    }

    #[allow(clippy::too_many_arguments)]
    fn process_channel_voice(
        id: ModuleId,
        cutoff_frequency: Sample,
        channel: &mut Channel,
        input_buffer: &mut Buffer,
        params: &ProcessParams,
        voice_idx: usize,
        channel_idx: usize,
        router: &dyn Router,
    ) {
        let voice = &mut channel.voices[voice_idx];

        let input = router
            .get_input(
                ModuleInput::new(Input::Audio, id),
                params.samples,
                voice_idx,
                channel_idx,
                input_buffer,
            )
            .unwrap_or(&ZEROES_BUFFER);

        if voice.current_cutoff != cutoff_frequency {
            let coeffs = Coefficients::<Sample>::from_params(
                biquad::Type::LowPass,
                params.sample_rate.hz(),
                (cutoff_frequency * 4.0).hz(),
                Q_BUTTERWORTH_F32,
            )
            .unwrap();

            voice.filter.replace_coefficients(coeffs);
            voice.current_cutoff = cutoff_frequency;
        }

        for (output, input) in voice.output.iter_mut().take(params.samples).zip(input) {
            *output = voice.filter.run(*input);
        }
    }
}

impl SynthModule for ModulationFilter {
    fn id(&self) -> ModuleId {
        self.id
    }

    fn label(&self) -> String {
        self.label.clone()
    }

    fn set_label(&mut self, label: String) {
        self.label = label.clone();
        self.config.lock().label = Some(label);
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::ModulationFilter
    }

    fn inputs(&self) -> &'static [InputInfo] {
        static INPUTS: &[InputInfo] = &[InputInfo::buffer(Input::Audio)];

        INPUTS
    }

    fn outputs(&self) -> &'static [DataType] {
        &[DataType::Buffer]
    }

    fn note_on(&mut self, params: &NoteOnParams) {
        if params.reset {
            for channel in &mut self.channels {
                channel.voices[params.voice_idx].filter.reset_state();
            }
        }
    }

    fn process(&mut self, params: &ProcessParams, router: &dyn Router) {
        for (channel_idx, channel) in self.channels.iter_mut().enumerate() {
            for voice_idx in params.active_voices {
                Self::process_channel_voice(
                    self.id,
                    self.cutoff_frequency,
                    channel,
                    &mut self.input_buffer,
                    params,
                    *voice_idx,
                    channel_idx,
                    router,
                );
            }
        }
    }

    fn get_buffer_output(&self, voice_idx: usize, channel_idx: usize) -> &Buffer {
        &self.channels[channel_idx].voices[voice_idx].output
    }
}
