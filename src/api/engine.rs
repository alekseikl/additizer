use axum::{Json, Router, extract::State, routing::get};
use serde::Serialize;

use crate::{
    api::SynthRef,
    synth_engine::{ModuleId, ModuleType, SynthEngineUiData},
};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ModuleListItem {
    id: ModuleId,
    module_type: ModuleType,
    label: String,
}

async fn modules(State(synth): State<SynthRef>) -> Json<Vec<ModuleListItem>> {
    Json(
        synth
            .lock()
            .get_modules()
            .iter()
            .map(|module| ModuleListItem {
                id: module.id(),
                module_type: module.module_type(),
                label: module.label(),
            })
            .collect(),
    )
}

async fn engine_params(State(synth): State<SynthRef>) -> Json<SynthEngineUiData> {
    Json(synth.lock().get_ui())
}

pub fn engine_router() -> Router<SynthRef> {
    Router::new()
        .route("/", get(engine_params))
        .route("/modules", get(modules))
}
