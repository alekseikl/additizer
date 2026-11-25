//! Utilities for saving a [`crate::plugin::Plugin`]'s state. The actual state object is also exposed
//! to plugins through the [`GuiContext`][crate::prelude::GuiContext].

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// These state objects are also exposed directly to the plugin so it can do its own internal preset
// management

/// A plain, unnormalized value for a parameter.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParamValue {
    F32(f32),
    I32(i32),
    Bool(bool),
    /// Only used for enum parameters that have the `#[id = "..."]` attribute set.
    String(String),
}

/// A plugin's state so it can be restored at a later point. This object can be serialized and
/// deserialized using serde.
///
/// The fields are stored as `BTreeMap`s so the order in the serialized file is consistent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedState {
    /// The plugin version this state was saved with. Right now this is not used, but later versions
    /// of nice-plug may allow you to modify the plugin state object directly before it is loaded to
    /// allow migrating plugin states between breaking parameter changes.
    ///
    /// # Notes
    ///
    /// If the saved state is very old, then this field may be empty.
    #[serde(default)]
    pub version: String,

    /// Kept so older state files still deserialize. Parameter values are not saved or restored;
    /// the host delivers them as note events.
    #[serde(default)]
    pub params: BTreeMap<String, ParamValue>,
    /// Arbitrary fields persisted with the plugin.
    /// [`PluginState::serialize_fields`][crate::params::PluginState::serialize_fields] fills this
    /// map and [`PluginState::deserialize_fields`][crate::params::PluginState::deserialize_fields]
    /// restores it.
    ///
    /// The individual fields are also serialized as JSON so they can safely be restored
    /// independently of the other fields.
    pub fields: BTreeMap<String, String>,
}
