use std::sync::Arc;

use anyhow::{Context, Result};
use nice_plug_core::{
    params::Params,
    plugin::{Plugin, PluginState},
};

/// Serialize a plugin's state to a state object. This is separate from [`serialize_json()`] to
/// allow passing the raw object directly to the plugin. Parameter values are not included; the
/// host delivers those as note events. Fields marked with `#[persist]` are saved.
pub(crate) fn serialize_object<P: Plugin>(plugin_params: Arc<dyn Params>) -> PluginState {
    PluginState {
        version: String::from(P::VERSION),
        params: Default::default(),
        fields: plugin_params.serialize_fields(),
    }
}

/// Serialize a plugin's state to a vector containing JSON data. This can (and should) be shared
/// across plugin formats. If the `zstd` feature is enabled, then the state will be compressed using
/// Zstandard.
pub(crate) fn serialize_json<P: Plugin>(plugin_params: Arc<dyn Params>) -> Result<Vec<u8>> {
    let plugin_state = serialize_object::<P>(plugin_params);
    let json = serde_json::to_vec(&plugin_state).context("Could not format as JSON")?;

    #[cfg(feature = "zstd")]
    {
        let compressed = zstd::encode_all(json.as_slice(), zstd::DEFAULT_COMPRESSION_LEVEL)
            .context("Could not compress state")?;

        let state_bytes = json.len();
        let compressed_state_bytes = compressed.len();
        let compression_ratio = compressed_state_bytes as f32 / state_bytes as f32 * 100.0;
        nice_plug_core::nice_trace!(
            "Compressed {state_bytes} bytes of state to {compressed_state_bytes} bytes \
             ({compression_ratio:.1}% compression ratio)"
        );

        Ok(compressed)
    }
    #[cfg(not(feature = "zstd"))]
    {
        Ok(json)
    }
}

/// Deserialize a plugin's state from a [`PluginState`] object. Restores fields marked with
/// `#[persist]`. Parameter values in the state are ignored.
///
/// The [`Plugin`] argument is used to call [`Plugin::filter_state()`] just before loading the
/// state.
pub(crate) fn deserialize_object<P: Plugin>(
    state: &mut PluginState,
    plugin_params: Arc<dyn Params>,
) -> bool {
    P::filter_state(state);
    plugin_params.deserialize_fields(&state.fields);
    true
}

/// Deserialize a plugin's state from a vector containing (compressed) JSON data. Doesn't load the
/// plugin state since doing so should be accompanied by calls to `Plugin::init()` and
/// `Plugin::reset()`, and this way all of that behavior can be encapsulated so it can be reused in
/// multiple places. The returned state object can be passed to [`deserialize_object()`].
pub(crate) fn deserialize_json(state: &[u8]) -> Option<PluginState> {
    #[cfg(feature = "zstd")]
    let result: Option<PluginState> = match zstd::decode_all(state) {
        Ok(decompressed) => match serde_json::from_slice(decompressed.as_slice()) {
            Ok(s) => {
                let state_bytes = decompressed.len();
                let compressed_state_bytes = state.len();
                let compression_ratio = compressed_state_bytes as f32 / state_bytes as f32 * 100.0;
                crate::nice_trace!(
                    "Inflated {compressed_state_bytes} bytes of state to {state_bytes} bytes \
                     ({compression_ratio:.1}% compression ratio)"
                );

                Some(s)
            }
            Err(err) => {
                crate::nice_debug_assert_failure!("Error while deserializing state: {}", err);
                None
            }
        },
        // Uncompressed state files can still be loaded after enabling this feature to prevent
        // breaking existing plugin instances
        Err(zstd_err) => match serde_json::from_slice(state) {
            Ok(s) => {
                crate::nice_trace!("Older uncompressed state found");
                Some(s)
            }
            Err(json_err) => {
                crate::nice_debug_assert_failure!(
                    "Error while deserializing state as either compressed or uncompressed state: \
                     {}, {}",
                    zstd_err,
                    json_err
                );
                None
            }
        },
    };

    #[cfg(not(feature = "zstd"))]
    let result: Option<PluginState> = match serde_json::from_slice(state) {
        Ok(s) => Some(s),
        Err(err) => {
            crate::nice_debug_assert_failure!("Error while deserializing state: {}", err);
            None
        }
    };

    result
}
