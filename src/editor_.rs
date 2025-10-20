pub mod gain_slider;

use gain_slider::{GainSlider, GainSliderModifiers};
use nih_plug::prelude::Editor;
use parking_lot::Mutex;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use vizia_plug::{ViziaState, ViziaTheming, create_vizia_editor};

use crate::AdditizerParams;
use crate::synth_engine::SynthEngine;

pub const NOTO_SANS: &str = "Noto Sans";

#[derive(Lens)]
struct Data {
    params: Arc<AdditizerParams>,
    synth_engine: Arc<Mutex<SynthEngine>>,
    harmonics: Vec<f32>,
}

enum EditorEvent {
    Harmonic(f32, usize),
    TailHarmonic(f32),
}

// Makes sense to also define this here, makes it a bit easier to keep track of
pub(crate) fn default_state() -> Arc<ViziaState> {
    ViziaState::new(|| (824, 400))
}

impl Model for Data {
    fn event(&mut self, _cx: &mut EventContext, event: &mut Event) {
        event.map(|editor_event, _meta| match editor_event {
            EditorEvent::Harmonic(value, idx) => {
                self.harmonics[*idx] = *value;
                *self.params.harmonics.lock() = self.harmonics.clone();
                self.synth_engine.lock().update_harmonics(
                    &self.harmonics,
                    self.params.tail_harmonics.load(Ordering::Relaxed),
                );
            }
            EditorEvent::TailHarmonic(value) => {
                self.params.tail_harmonics.store(*value, Ordering::Relaxed);
                self.synth_engine
                    .lock()
                    .update_harmonics(&self.harmonics, *value);
            }
        });
    }
}

pub(crate) fn create(
    params: Arc<AdditizerParams>,
    editor_state: Arc<ViziaState>,
    synth_engine: Arc<Mutex<SynthEngine>>,
) -> Option<Box<dyn Editor>> {
    create_vizia_editor(editor_state, ViziaTheming::None, move |cx, _| {
        cx.add_stylesheet(include_style!("src/style.css"))
            .expect("Failed to load styles.");

        let harmonics_count: usize = params.harmonics.lock().len();

        Data {
            params: Arc::clone(&params),
            synth_engine: Arc::clone(&synth_engine),
            harmonics: params.harmonics.lock().clone(),
        }
        .build(cx);

        HStack::new(cx, |cx| {
            for i in 0..harmonics_count {
                GainSlider::new(cx, i as i32 + 1, Data::harmonics.map(move |list| list[i]))
                    .on_change(move |ex, value| ex.emit(EditorEvent::Harmonic(value, i)));
            }

            GainSlider::new(
                cx,
                99,
                Data::params
                    .map(move |p| p.tail_harmonics.load(std::sync::atomic::Ordering::Relaxed)),
            )
            .class("tail-harmonics")
            .on_change(move |ex, value| ex.emit(EditorEvent::TailHarmonic(value)));
        })
        .class("harmonics-container");
    })
}
