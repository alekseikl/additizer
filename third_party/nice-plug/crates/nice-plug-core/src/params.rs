//! Host parameters and persistent plugin fields.
//!
//! A plugin returns a [`PluginState`] from [`Plugin::state()`][crate::plugin::Plugin::state()].
//! The wrapper calls the parameter methods when the host asks. It does not keep a parameter
//! object. The parameter's index is its id. The count does not change for the life of the instance.
//!
//! Persistent data is saved and restored through [`PluginState::serialize_fields`] and
//! [`PluginState::deserialize_fields`].

use std::collections::BTreeMap;

// The proc-macro for deriving `PluginState` persistence.
pub use nice_plug_derive::Params;

pub mod enums;
pub mod persist;
pub mod range;
pub mod smoothing;

bitflags::bitflags! {
    /// Flags for controlling a parameter's behavior.
    #[repr(transparent)]
    #[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ParamFlags: u32 {
        /// When applied to a boolean parameter, this links it to the host's bypass control. Only
        /// one parameter can be marked as bypass.
        const BYPASS = 1 << 0;
        /// The parameter cannot be changed from an automation lane. The parameter can still be
        /// changed from the plugin's own GUI or from the host's generic UI.
        const NON_AUTOMATABLE = 1 << 1;
        /// Hides the parameter in the host's generic UI. This also implies [`NON_AUTOMATABLE`](Self::NON_AUTOMATABLE).
        const HIDDEN = 1 << 2;
    }
}

/// Persistent plugin data and the host parameters the wrapper queries by index.
///
/// Value changes from the host are delivered as note events. These methods answer host queries.
/// They are not a source of values for DSP, and the audio thread does not call them.
pub trait PluginState: 'static + Send + Sync {
    /// Number of host parameters. Constant for the plugin instance.
    fn param_count(&self) -> u32 {
        0
    }

    /// Human-readable name. `index` is the parameter id.
    fn param_name(&self, index: u32) -> String {
        format!("Param {}", index + 1)
    }

    /// Unit label appended by the host. Empty when the parameter has no unit.
    fn param_unit(&self, _index: u32) -> &'static str {
        ""
    }

    /// Behavior flags. See [`ParamFlags`].
    fn param_flags(&self, _index: u32) -> ParamFlags {
        ParamFlags::HIDDEN | ParamFlags::NON_AUTOMATABLE
    }

    /// Discrete step count. `None` for a continuous parameter.
    fn param_step_count(&self, _index: u32) -> Option<usize> {
        None
    }

    /// Normalized `[0, 1]` default.
    fn param_default_normalized(&self, _index: u32) -> f32 {
        0.0
    }

    /// Last normalized `[0, 1]` value published for the host. Modulation offsets are not included.
    fn param_normalized_value(&self, _index: u32) -> f32 {
        0.0
    }

    /// String for a normalized value. Most plugin formats already have a unit field. Set
    /// `include_unit` only when the format has nowhere else to put it.
    fn param_value_to_text(&self, index: u32, normalized: f32, include_unit: bool) -> String {
        let plain = self.param_normalized_to_plain(index, normalized);
        if include_unit {
            format!("{plain}{}", self.param_unit(index))
        } else {
            format!("{plain}")
        }
    }

    /// Parse a host string into a normalized value. The string may include [`param_unit`](Self::param_unit).
    /// `None` cancels the update.
    fn param_text_to_normalized(&self, index: u32, text: &str) -> Option<f32> {
        let unit = self.param_unit(index);
        let plain = text.trim().trim_end_matches(unit).trim().parse().ok()?;
        Some(self.param_plain_to_normalized(index, plain))
    }

    /// Plain value for a normalized value. Continuous parameters snap to their step size here.
    fn param_normalized_to_plain(&self, _index: u32, normalized: f32) -> f32 {
        normalized.clamp(0.0, 1.0)
    }

    /// Normalized value for a plain value.
    fn param_plain_to_normalized(&self, _index: u32, plain: f32) -> f32 {
        plain.clamp(0.0, 1.0)
    }

    /// Serialize persistent fields into a map of JSON values so they can be written to the
    /// plugin's state and recalled later.
    fn serialize_fields(&self) -> BTreeMap<String, String> {
        BTreeMap::new()
    }

    /// Restore persistent fields from a map created by
    /// [`serialize_fields()`][Self::serialize_fields]. Called when the plugin's state is restored.
    #[allow(unused_variables)]
    fn deserialize_fields(&self, serialized: &BTreeMap<String, String>) {}
}
