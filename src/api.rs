use std::sync::Arc;

use axum::{Router, routing::get};
use parking_lot::Mutex;
use tokio::runtime::Runtime;

use crate::{api::router::build_router, synth_engine::SynthEngine};

mod router;

pub struct ApiServer {
    runtime: Mutex<Option<Runtime>>,
    synth_engine: Arc<Mutex<SynthEngine>>,
}

impl ApiServer {
    pub fn new(synth_engine: Arc<Mutex<SynthEngine>>) -> Self {
        Self {
            runtime: Mutex::new(None),
            synth_engine,
        }
    }

    pub fn is_running(&self) -> bool {
        self.runtime.lock().is_some()
    }

    pub fn start(&self) {
        if self.runtime.lock().is_some() {
            return;
        }

        let Some(runtime) = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .ok()
        else {
            return;
        };

        let router = build_router(Arc::clone(&self.synth_engine));

        runtime.spawn(async move {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:7688").await?;

            axum::serve(listener, router).await
        });

        *self.runtime.lock() = Some(runtime);
    }

    pub fn stop(&self) {
        if let Some(runtime) = self.runtime.lock().take() {
            runtime.shutdown_background();
        }
    }
}
