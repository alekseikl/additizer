//! Host parameter registration.
//!
//! [`PluginState::params`] is called once. The returned vector is stored under a [`Mutex`] for the
//! plugin instance lifetime, and each parameter's index is the hash the host uses to address it.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use nice_plug_core::params::internals::ParamPtr;
use nice_plug_core::params::{FloatParam, Param, ParamFlags, PluginState};
use parking_lot::Mutex;

/// Parameters registered with the host, plus the maps the wrappers use to address them.
pub(crate) struct RegisteredParams {
    /// Owns the parameter objects. `ParamPtr`s point into this vector.
    ///
    /// The vector must not be resized or replaced. Moving this mutex does not move the allocation.
    pub storage: Mutex<Vec<FloatParam>>,
    pub hashes: Vec<u32>,
    pub by_hash: HashMap<u32, ParamPtr>,
    pub id_by_hash: HashMap<u32, String>,
    pub group_by_hash: HashMap<u32, String>,
    pub ptr_to_hash: HashMap<ParamPtr, u32>,
    pub poly_mod_ids_by_hash: HashMap<u32, u32>,
}

impl RegisteredParams {
    pub fn new(state: &Arc<dyn PluginState>) -> Self {
        let storage = Mutex::new(state.params());
        let params = storage.lock();

        let mut hashes = Vec::with_capacity(params.len());
        let mut by_hash = HashMap::with_capacity(params.len());
        let mut id_by_hash = HashMap::with_capacity(params.len());
        let mut group_by_hash = HashMap::with_capacity(params.len());
        let mut ptr_to_hash = HashMap::with_capacity(params.len());
        let mut poly_mod_ids_by_hash = HashMap::new();
        let mut bypass_param_exists = false;

        for (index, param) in params.iter().enumerate() {
            let hash = index as u32;
            let ptr = param.as_ptr();
            hashes.push(hash);
            by_hash.insert(hash, ptr);
            id_by_hash.insert(hash, hash.to_string());
            group_by_hash.insert(hash, String::new());
            ptr_to_hash.insert(ptr, hash);

            if let Some(id) = param.poly_modulation_id() {
                poly_mod_ids_by_hash.insert(hash, id);
            }

            if cfg!(debug_assertions) {
                let is_bypass = param.flags().contains(ParamFlags::BYPASS);
                if is_bypass && bypass_param_exists {
                    crate::nice_debug_assert_failure!(
                        "Duplicate bypass parameters found, the host will only use the first one"
                    );
                }
                bypass_param_exists |= is_bypass;
            }
        }

        if cfg!(debug_assertions) {
            let poly_mod_ids: HashSet<u32> = poly_mod_ids_by_hash.values().copied().collect();
            crate::nice_debug_assert_eq!(
                poly_mod_ids_by_hash.len(),
                poly_mod_ids.len(),
                "The plugin has duplicate poly modulation IDs. Polyphonic modulation will not be \
                 routed to the correct parameter."
            );
        }

        drop(params);

        Self {
            storage,
            hashes,
            by_hash,
            id_by_hash,
            group_by_hash,
            ptr_to_hash,
            poly_mod_ids_by_hash,
        }
    }
}
