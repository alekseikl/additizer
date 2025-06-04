pub mod gain_slider;

use gain_slider::{GainSlider, GainSliderModifiers};
use nih_plug::prelude::Editor;
use std::sync::Arc;
use vizia_plug::vizia::prelude::*;
use vizia_plug::widgets::*;
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
    SubharmonicChanged(f32, usize),
    HarmonicChanged(f32, usize),
}

// Makes sense to also define this here, makes it a bit easier to keep track of
pub(crate) fn default_state() -> Arc<ViziaState> {
    ViziaState::new(|| (800, 400))
}

impl Model for Data {
    fn event(&mut self, _cx: &mut EventContext, event: &mut Event) {
        event.map(|editor_event, _meta| match editor_event {
            EditorEvent::SubharmonicChanged(value, idx) => {
                self.subharmonics[*idx] = *value;
            }
            EditorEvent::HarmonicChanged(value, idx) => {
                self.harmonics[*idx] = *value;
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
            params: params.clone(),
            gain: 1.0,
            subharmonics: vec![1.0; subharmonics_count],
            harmonics: vec![1.0; harmonics_count],
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
                .on_change(move |ex, value| ex.emit(EditorEvent::SubharmonicChanged(value, i)));
            }

            for i in 0..harmonics_count {
                GainSlider::new(cx, i as i32 + 1, Data::harmonics.map(move |list| list[i]))
                    .on_change(move |ex, value| ex.emit(EditorEvent::HarmonicChanged(value, i)));
            }
        })
        .class("harmonics-container");
    })
}
