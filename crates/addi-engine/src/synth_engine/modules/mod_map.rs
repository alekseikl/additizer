use std::array;

use wide::f32x4;

mod config;
mod link;
pub mod stub;

#[cfg(test)]
mod tests;

pub use config::{MAX_SMOOTH, ModMapConfig};
pub use link::{ModMapAudioEnd, ModMapLinks, ModMapUiEnd, UiEvent};

use addi_dsp::{
    filters::ctrl_filter::{CtrlFilter, CtrlFilterCoeffs},
    segments::Segments,
    utils::sanitize,
};

use crate::synth_engine::{
    Input, ModuleId, SmoothedSampleParams, Smp, StereoSample, WideSmp,
    buffer::{Buffer, VoicesLayout, new_voices_layout, zero_buffer},
    fast_math::{CATMULL_TAPS, catmull_rom},
    map_in_place,
    routing::{
        ControlRouterType, DataType, InputMeta, InputSlots, MixedSlots, ProcessContext,
        RouterFactory, SamplesOutput, VoiceTarget,
    },
    smooth::SmoothedSample,
    synth_module::SynthModule,
    voices::{NUM_CHANNELS, RIGHT_CHANNEL},
    zip_map_in_place,
};

/// Closed-range transfer table. Sample `i` is the curve at `i / (TABLE_SIZE - 1)`.
const TABLE_SIZE: usize = 2048;
const TABLE_PAD_LEFT: usize = 1;
const TABLE_PAD_RIGHT: usize = 2;
const TABLE_BUFFER_SIZE: usize = TABLE_SIZE + TABLE_PAD_LEFT + TABLE_PAD_RIGHT;

const _: () = assert!(TABLE_PAD_LEFT + TABLE_PAD_RIGHT == CATMULL_TAPS - 1);
const _: () = assert!(TABLE_BUFFER_SIZE == TABLE_SIZE - 1 + CATMULL_TAPS);

struct Params {
    bipolar: bool,
    smooth: Smp,
    segments: Segments,
}

impl Params {
    fn from_config(c: &ModMapConfig) -> Self {
        Self {
            bipolar: c.bipolar,
            smooth: sanitize(c.smooth, 0.0, MAX_SMOOTH),
            segments: c.segments.clone(),
        }
    }
}

struct ChannelParams {
    amount: SmoothedSample,
}

impl ChannelParams {
    fn from_config(c: &ModMapConfig, channel_idx: usize) -> Self {
        Self {
            amount: sanitize(c.amount[channel_idx], 0.0, 1.0).into(),
        }
    }

    fn advance_smoothers(&mut self, smooth_params: &SmoothedSampleParams, samples: usize) {
        self.amount.advance(smooth_params, samples);
    }
}

struct Inputs {
    control: Option<usize>,
    amount: MixedSlots,
}

impl Default for Inputs {
    fn default() -> Self {
        Self {
            control: None,
            amount: MixedSlots::new(Input::Amount),
        }
    }
}

impl Inputs {
    fn from_slots(inputs: &[InputSlots]) -> Self {
        let mut result = Self::default();

        for input in inputs {
            match input {
                InputSlots::Direct { input_type, slot } => {
                    if matches!(input_type, Input::Control) {
                        result.control = Some(*slot);
                    }
                }
                InputSlots::Mixed(input) => {
                    if matches!(input.input_type, Input::Amount) {
                        result.amount = input.clone();
                    }
                }
                InputSlots::Spectral { .. } => (),
            }
        }

        result
    }

    fn update_amount(&mut self, input_type: Input, src_slot: usize, amount: StereoSample) {
        if matches!(input_type, Input::Amount) {
            self.amount.update_amount(src_slot, amount);
        }
    }
}

struct Buffers {
    amount: Buffer,
}

impl Buffers {
    fn new(buffer_size: usize) -> Self {
        Self {
            amount: zero_buffer(buffer_size),
        }
    }
}

/// Catmull–Rom read of a closed table. End pads repeat the endpoint samples.
#[inline(always)]
fn read_table(table: &[Smp], x: Smp) -> Smp {
    let max = (TABLE_SIZE - 1) as Smp;
    let pos = (x.clamp(0.0, 1.0) * max).min(max);
    let idx = pos as usize;
    let t = pos - idx as Smp;
    let s = &table[idx..idx + CATMULL_TAPS];
    let samples = f32x4::new([s[0], s[1], s[2], s[3]]);
    let [a, b, c, d] = (catmull_rom(t) * samples).to_array();

    (a + c) + (b + d)
}

pub struct ModMap<L: ModMapLinks = stub::Links> {
    id: ModuleId,
    params: Params,
    remap: bool,
    channel_params: [ChannelParams; NUM_CHANNELS],
    table: [Smp; TABLE_BUFFER_SIZE],
    buffers: Buffers,
    smoothers: VoicesLayout<CtrlFilter>,
    coeffs: CtrlFilterCoeffs,
    probe: StereoSample,
    audio_end: L::AudioEnd,
    ui_end: Option<L::UiEnd>,
    inputs: Inputs,
    output_slot: usize,
}

impl<L: ModMapLinks> ModMap<L> {
    pub fn take_ui_end(&mut self) -> Option<L::UiEnd> {
        self.ui_end.take()
    }

    pub fn new(id: ModuleId, buffer_size: usize) -> Self {
        Self::from_config(
            &ModMapConfig {
                id,
                ..ModMapConfig::default()
            },
            buffer_size,
        )
    }

    pub fn from_config(config: &ModMapConfig, buffer_size: usize) -> Self {
        let mut config = config.clone();
        config.sanitize();

        let (audio_end, ui_end) = L::create_link_pair();

        let mut module = Self {
            id: config.id,
            params: Params::from_config(&config),
            remap: false,
            channel_params: array::from_fn(|channel_idx| {
                ChannelParams::from_config(&config, channel_idx)
            }),
            table: [0.0; TABLE_BUFFER_SIZE],
            buffers: Buffers::new(buffer_size),
            smoothers: new_voices_layout(),
            coeffs: CtrlFilterCoeffs {
                g: 0.0,
                min_time: 0.0,
            },
            probe: StereoSample::ZERO,
            audio_end,
            ui_end,
            inputs: Inputs::default(),
            output_slot: usize::MAX,
        };

        module.bake_table();
        module
    }

    pub fn get_config(&self) -> ModMapConfig {
        ModMapConfig {
            id: self.id,
            amount: get_smoothed_param!(self, amount),
            bipolar: self.params.bipolar,
            smooth: self.params.smooth,
            segments: self.params.segments.clone(),
        }
    }

    set_mono_param!(set_bipolar, bipolar, bool);
    set_mono_param!(set_smooth, smooth, Smp, sanitize(smooth, 0.0, MAX_SMOOTH));
    set_smoothed_param!(set_amount, amount, sanitize(*amount, 0.0, 1.0));

    pub fn set_segments(&mut self, mut segments: Segments) {
        segments.sanitize();
        self.params.segments = segments;
        self.bake_table();
    }

    fn bake_table(&mut self) {
        if self.params.segments.is_unity() {
            self.remap = false;
            return;
        }

        self.remap = true;

        let start = TABLE_PAD_LEFT;
        let end = TABLE_PAD_LEFT + TABLE_SIZE;

        self.params
            .segments
            .render_closed(&mut self.table[start..end]);

        let first = self.table[start];
        let last = self.table[end - 1];

        self.table[0] = first;
        self.table[end] = last;
        self.table[end + 1] = last;
    }

    fn process_voice(
        &mut self,
        target: &VoiceTarget,
        outputs: &mut VoicesLayout<SamplesOutput>,
        rf: &mut RouterFactory<ControlRouterType, L::EngineEnd>,
    ) {
        let (mut router, mut voice_output) = rf.for_voice(target, outputs);
        let output = voice_output.output();

        output.copy_from_slice(router.direct(self.inputs.control));

        if router.need_update_ui() {
            self.probe[target.channel_idx] = output.last().copied().unwrap_or_default();

            if target.channel_idx == RIGHT_CHANNEL {
                self.audio_end.update_probe(self.probe);
            }
        }

        if self.remap {
            for sample in output.iter_mut() {
                *sample = read_table(&self.table, *sample);
            }
        }

        let amount = &self.channel_params[target.channel_idx].amount;

        if let Some(amount) = router.param_stationary(&self.inputs.amount, amount) {
            let amount = amount.clamp(0.0, 1.0);

            if amount < 1.0 {
                let amount = WideSmp::splat(amount);

                map_in_place(output, |sample| sample * amount);
            }
        } else {
            router.param(&self.inputs.amount, amount, &mut self.buffers.amount);

            let n = output.len();

            zip_map_in_place(output, &self.buffers.amount[..n], |sample, amount| {
                sample * amount.clamp(WideSmp::ZERO, WideSmp::ONE)
            });
        }

        if self.params.bipolar {
            let two = WideSmp::splat(2.0);
            let bias = WideSmp::splat(-1.0);

            map_in_place(output, |sample| sample.mul_add(two, bias));
        }

        if self.params.smooth >= self.coeffs.min_time {
            let g = self.coeffs.g;
            let smoother = self.smoothers.at_mut(target.channel_idx, target.voice_idx);

            if router.triggered() {
                smoother.reset(output.first().copied().unwrap_or_default());
            }

            for sample in output.iter_mut() {
                *sample = smoother.tick(g, *sample);
            }
        }
    }

    pub(crate) fn process(&mut self, ctx: &mut ProcessContext<L::EngineEnd>) {
        ctx.control(self.id, self.output_slot)
            .for_module(|rf| {
                self.coeffs = CtrlFilter::coeffs(self.params.smooth, rf.params().sample_rate);
            })
            .for_voices(|rf, target, outputs| {
                self.process_voice(target, outputs, rf);
            })
            .advance(|rf| {
                for channel in &mut self.channel_params {
                    channel.advance_smoothers(&rf.params().smooth, rf.params().samples);
                }
            });
    }
}

impl<L: ModMapLinks> SynthModule for ModMap<L> {
    fn id(&self) -> ModuleId {
        self.id
    }

    fn inputs(&self) -> &'static [InputMeta] {
        static INPUTS: &[InputMeta] = &[
            InputMeta::direct_control(Input::Control),
            InputMeta::control(Input::Amount),
        ];

        INPUTS
    }

    fn output_type(&self) -> DataType {
        DataType::Control
    }

    fn output_slot(&self) -> usize {
        self.output_slot
    }

    fn set_output_slot(&mut self, slot: usize) {
        self.output_slot = slot;
    }

    fn set_input_slots(&mut self, inputs: &[InputSlots]) {
        self.inputs = Inputs::from_slots(inputs);
    }

    fn update_input_amount(&mut self, input_type: Input, src_slot: usize, amount: StereoSample) {
        self.inputs.update_amount(input_type, src_slot, amount);
    }

    fn process_ui_events(&mut self) {
        while let Some(event) = self.audio_end.pop_event() {
            match event {
                UiEvent::InputParam { input, value } => {
                    if matches!(input, Input::Amount) {
                        self.set_amount(value);
                    }
                }
                UiEvent::SegmentsUpdated => {
                    let segments = self.audio_end.read_segments().clone();
                    self.set_segments(segments);
                }
                UiEvent::Bipolar(value) => self.set_bipolar(value),
                UiEvent::Smooth(value) => self.set_smooth(value),
            }
        }
    }
}
