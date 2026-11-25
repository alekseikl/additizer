use std::collections::BTreeMap;
use std::sync::atomic::Ordering;

use atomic_float::AtomicF32;
use nice_plug_core::params::range::FloatRange;
use nice_plug_core::params::{FloatParam, PluginState};
use parking_lot::Mutex;

use addi_engine::external_param::NUM_EXT_PARAMS;
use addi_engine::{EngineConfig, Smp, SynthEngine};

use crate::{
    default_scheme::build_default_preset,
    links::PluginLinks,
    preset::{Preset, PresetInfo},
    ui_bridge::UiBridge,
};

/// The preset field key written by the previous parameter object. Kept so old presets still load.
const PRESET_FIELD: &str = "plugin-preset";

pub struct AddiState {
    /// Host sample rate. Zero until [`activate`][Self::activate]; a non-zero value means the
    /// plugin is activated.
    host_sample_rate: AtomicF32,
    /// Preset last published or last restored from the host. The open editor's bridge is newer;
    /// [`serialize_fields`][PluginState::serialize_fields] reads that first.
    ///
    /// In place before [`activate`][Self::activate]: the default patch at construction, or the
    /// host preset if state was restored first.
    preset: Mutex<Box<Preset>>,
    /// Newly published engine. The audio thread takes it when the mutex is free.
    engine: Mutex<Option<Box<SynthEngine<PluginLinks>>>>,
    /// Current UI bridge. The editor locks it while drawing. A replacement bridge reports
    /// [`UiBridge::is_new`] until the editor calls [`UiBridge::reset_new`].
    ui_bridge: Mutex<Option<Box<UiBridge>>>,
}

impl AddiState {
    pub fn new() -> Self {
        Self {
            host_sample_rate: AtomicF32::new(0.0),
            preset: Mutex::new(Box::new(build_default_preset())),
            engine: Mutex::new(None),
            ui_bridge: Mutex::new(None),
        }
    }

    /// Slot the editor locks while drawing.
    pub fn ui_bridge(&self) -> &Mutex<Option<Box<UiBridge>>> {
        &self.ui_bridge
    }

    /// Takes the published engine when the slot is free, leaving `None`.
    ///
    /// Returns `None` when the slot is empty or currently locked.
    pub fn try_take_engine(&self) -> Option<Box<SynthEngine<PluginLinks>>> {
        self.engine.try_lock()?.take()
    }

    /// Build the engine for `sample_rate` from the preset already stored here.
    ///
    /// The returned engine is for the audio thread. A replacement published later stays in
    /// [`try_take_engine`][Self::try_take_engine].
    pub fn activate(&self, sample_rate: Smp) -> Option<Box<SynthEngine<PluginLinks>>> {
        self.host_sample_rate.store(sample_rate, Ordering::Release);
        self.engine.lock().take();

        let (engine, bridge) = {
            let preset = self.preset.lock();
            let mut engine = self.build_engine(&preset.engine, preset.oversampling)?;
            let bridge = UiBridge::create(&mut engine, preset.ui.clone(), preset.oversampling)?;
            (engine, bridge)
        };
        self.ui_bridge.lock().replace(Box::new(bridge));
        Some(Box::new(engine))
    }

    /// Copy the open editor's preset into `preset`, then drop the engine and the UI bridge.
    ///
    /// The host reactivates later without sending the preset again. `host_sample_rate` goes back
    /// to zero.
    pub fn deactivate(&self) {
        let live = self.ui_bridge.lock().as_mut().map(|bridge| bridge.preset());
        if let Some(preset) = live {
            *self.preset.lock() = preset;
        }
        self.host_sample_rate.store(0.0, Ordering::Release);
        self.engine.lock().take();
        self.ui_bridge.lock().take();
    }

    pub fn load_preset(&self, preset: &Preset) -> bool {
        let Some(mut engine) = self.build_engine(&preset.engine, preset.oversampling) else {
            return false;
        };
        let Some(bridge) = UiBridge::create(&mut engine, preset.ui.clone(), preset.oversampling)
        else {
            return false;
        };

        **self.preset.lock() = Preset {
            info: PresetInfo::default(),
            engine: preset.engine.clone(),
            oversampling: preset.oversampling,
            ui: preset.ui.clone(),
        };
        *self.engine.lock() = Some(Box::new(engine));
        self.ui_bridge.lock().replace(Box::new(bridge));
        true
    }

    fn build_engine(
        &self,
        config: &EngineConfig,
        oversampling: bool,
    ) -> Option<SynthEngine<PluginLinks>> {
        SynthEngine::<PluginLinks>::try_new(
            config,
            self.host_sample_rate.load(Ordering::Acquire),
            oversampling,
        )
    }

    fn host_params() -> Vec<FloatParam> {
        (0..NUM_EXT_PARAMS)
            .map(|index| {
                FloatParam::new(
                    format!("Param {}", index + 1),
                    0.0,
                    FloatRange::Linear { min: 0.0, max: 1.0 },
                )
                .with_poly_modulation_id(index as u32)
            })
            .collect()
    }
}

impl PluginState for AddiState {
    fn params(&self) -> Vec<FloatParam> {
        Self::host_params()
    }

    fn serialize_fields(&self) -> BTreeMap<String, String> {
        let mut serialized = BTreeMap::new();
        let mut bridge = self.ui_bridge.lock();
        let preset_json = if let Some(bridge) = bridge.as_mut() {
            serde_json::to_string(&bridge.preset())
        } else {
            drop(bridge);
            serde_json::to_string(self.preset.lock().as_ref())
        };
        match preset_json {
            Ok(data) => {
                serialized.insert(String::from(PRESET_FIELD), data);
            }
            Err(err) => {
                tracing::error!("Could not serialize '{PRESET_FIELD}': {err}");
            }
        }
        serialized
    }

    fn deserialize_fields(&self, serialized: &BTreeMap<String, String>) {
        let Some(data) = serialized.get(PRESET_FIELD) else {
            return;
        };
        let preset = match serde_json::from_str::<Preset>(data) {
            Ok(preset) => preset,
            Err(err) => {
                tracing::error!("Could not deserialize '{PRESET_FIELD}': {err}");
                return;
            }
        };

        if self.host_sample_rate.load(Ordering::Acquire) != 0.0 {
            self.load_preset(&preset);
        } else {
            **self.preset.lock() = preset;
        }
    }
}
