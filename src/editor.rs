pub mod gain_slider;

use gain_slider::{GainSlider, GainSliderModifiers};
use nih_plug::prelude::Editor;
use std::sync::Arc;
use vizia_plug::vizia::prelude::*;
use vizia_plug::{ViziaState, ViziaTheming, create_vizia_editor};

use crate::AdditizerParams;

pub const NOTO_SANS: &str = "Noto Sans";

#[derive(Lens)]
struct Data {
    params: Arc<AdditizerParams>,
    gain: f32,
    subharmonics: Vec<f32>,
    harmonics: Vec<f32>,
}

enum EditorEvent {
    Subharmonic(f32, usize),
    Harmonic(f32, usize),
    TailHarmonic(f32),
}

// Makes sense to also define this here, makes it a bit easier to keep track of
pub(crate) fn default_state() -> Arc<ViziaState> {
    ViziaState::new(|| (800, 400))
}

impl Model for Data {
    fn event(&mut self, _cx: &mut EventContext, event: &mut Event) {
        event.map(|editor_event, _meta| match editor_event {
            EditorEvent::Subharmonic(value, idx) => {
                self.subharmonics[*idx] = *value;
                *self.params.subharmonics.lock().unwrap() = self.subharmonics.clone();
            }
            EditorEvent::Harmonic(value, idx) => {
                self.harmonics[*idx] = *value;
                *self.params.harmonics.lock().unwrap() = self.harmonics.clone();
            }
            EditorEvent::TailHarmonic(value) => {
                self.params
                    .tail_harmonics
                    .store(*value, std::sync::atomic::Ordering::Relaxed);
            }
        });
    }
}

pub(crate) fn create(
    params: Arc<AdditizerParams>,
    editor_state: Arc<ViziaState>,
) -> Option<Box<dyn Editor>> {
    create_vizia_editor(editor_state, ViziaTheming::None, move |cx, _| {
        cx.add_stylesheet(include_style!("src/style.css"))
            .expect("Failed to load styles.");

        let subharmonics_count: usize = 3;
        let harmonics_count: usize = 30;

        Data {
            params: Arc::clone(&params),
            gain: 1.0,
            subharmonics: params.subharmonics.lock().unwrap().clone(),
            harmonics: params.harmonics.lock().unwrap().clone(),
        }
        .build(cx);

        HStack::new(cx, |cx| {
            for i in 0..subharmonics_count {
                GainSlider::new(
                    cx,
                    i as i32 - 3,
                    Data::subharmonics.map(move |list| list[i]),
                )
                .class("subharmonic")
                .on_change(move |ex, value| ex.emit(EditorEvent::Subharmonic(value, i)));
            }

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
