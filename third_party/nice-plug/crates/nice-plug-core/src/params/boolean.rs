//! Simple boolean parameters.

use atomic_float::AtomicF32;
use std::fmt::{Debug, Display};
use std::sync::Arc;
use std::sync::atomic::Ordering;

use super::internals::ParamPtr;
use super::{InternalParamMut, Param, ParamFlags, ParamInfo};

/// A simple boolean parameter. The plugin receives value changes as note events.
pub struct BoolParam {
    /// Last normalized value stored for host queries. `1.0` is true.
    normalized_value: AtomicF32,
    /// The field's default value.
    default: bool,
    /// Metadata and conversion callbacks that are not used by the DSP hot path.
    info: Box<ParamInfo<bool>>,
    /// If this parameter has been marked as polyphonically modulatable, then this will be a unique
    /// integer identifying the parameter. Because this value is determined by the plugin itself,
    /// the plugin can easily map
    /// [`NoteEvent::PolyModulation`][crate::prelude::NoteEvent::PolyModulation] events to the
    /// correct parameter by pattern matching on a constant.
    poly_modulation_id: Option<u32>,
}

impl Display for BoolParam {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = self.modulated_plain_value();
        match (value, &self.info.value_to_string) {
            (v, Some(func)) => write!(f, "{}", func(v)),
            (true, None) => write!(f, "On"),
            (false, None) => write!(f, "Off"),
        }
    }
}

impl Debug for BoolParam {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.info.name, self)
    }
}

// `Params` can not be implemented outside of nice-plug itself because `ParamPtr` is also closed
impl super::Sealed for BoolParam {}

impl Param for BoolParam {
    type Plain = bool;

    fn name(&self) -> &str {
        &self.info.name
    }

    fn unit(&self) -> &'static str {
        self.info.unit
    }

    fn poly_modulation_id(&self) -> Option<u32> {
        self.poly_modulation_id
    }

    #[inline]
    fn modulated_plain_value(&self) -> Self::Plain {
        self.preview_plain(self.modulated_normalized_value())
    }

    #[inline]
    fn modulated_normalized_value(&self) -> f32 {
        self.normalized_value.load(Ordering::Relaxed)
    }

    #[inline]
    fn unmodulated_plain_value(&self) -> Self::Plain {
        self.modulated_plain_value()
    }

    #[inline]
    fn unmodulated_normalized_value(&self) -> f32 {
        self.modulated_normalized_value()
    }

    #[inline]
    fn default_plain_value(&self) -> Self::Plain {
        self.default
    }

    fn step_count(&self) -> Option<usize> {
        Some(1)
    }

    fn normalized_value_to_string(&self, normalized: f32, _include_unit: bool) -> String {
        let value = self.preview_plain(normalized);
        match (value, &self.info.value_to_string) {
            (v, Some(f)) => f(v),
            (true, None) => String::from("On"),
            (false, None) => String::from("Off"),
        }
    }

    fn string_to_normalized_value(&self, string: &str) -> Option<f32> {
        let string = string.trim();
        let value = match &self.info.string_to_value {
            Some(f) => f(string),
            None => Some(string.eq_ignore_ascii_case("true") || string.eq_ignore_ascii_case("on")),
        }?;

        Some(self.preview_normalized(value))
    }

    #[inline]
    fn preview_normalized(&self, plain: Self::Plain) -> f32 {
        if plain { 1.0 } else { 0.0 }
    }

    #[inline]
    fn preview_plain(&self, normalized: f32) -> Self::Plain {
        normalized > 0.5
    }

    fn flags(&self) -> ParamFlags {
        self.info.flags
    }

    fn as_ptr(&self) -> ParamPtr {
        ParamPtr::BoolParam(self as *const BoolParam as *mut BoolParam)
    }
}

impl InternalParamMut for BoolParam {
    unsafe fn _internal_set_plain_value(&self, plain: Self::Plain) -> bool {
        unsafe { self._internal_set_normalized_value(self.preview_normalized(plain)) }
    }

    unsafe fn _internal_set_normalized_value(&self, normalized: f32) -> bool {
        let normalized = self.preview_normalized(self.preview_plain(normalized));
        let old = self.normalized_value.swap(normalized, Ordering::Relaxed);
        normalized != old
    }
}

impl BoolParam {
    /// Build a new [`BoolParam`]. Use the other associated functions to modify the behavior of the
    /// parameter.
    pub fn new(name: impl Into<String>, default: bool) -> Self {
        Self {
            normalized_value: AtomicF32::new(if default { 1.0 } else { 0.0 }),
            default,
            info: Box::new(ParamInfo::new(name)),
            poly_modulation_id: None,
        }
    }

    /// Enable polyphonic modulation for this parameter. The ID is used to uniquely identify this
    /// parameter in [`NoteEvent::PolyModulation`][crate::midi::NoteEvent::PolyModulation]
    /// events, and must thus be unique between _all_ polyphonically modulatable parameters. See the
    /// event's documentation on how to use polyphonic modulation. Also consider configuring the
    /// `ClapPlugin::CLAP_POLY_MODULATION_CONFIG` constant when enabling this.
    ///
    /// # Important
    ///
    /// After enabling polyphonic modulation, the plugin **must** start sending
    /// [`NoteEvent::VoiceTerminated`][crate::midi::NoteEvent::VoiceTerminated] events to the
    /// host when a voice has fully ended. This allows the host to reuse its modulation resources.
    pub fn with_poly_modulation_id(mut self, id: u32) -> Self {
        self.poly_modulation_id = Some(id);
        self
    }

    /// Use a custom conversion function to convert the boolean value to a string.
    pub fn with_value_to_string(
        mut self,
        callback: Arc<dyn Fn(bool) -> String + Send + Sync>,
    ) -> Self {
        self.info.value_to_string = Some(callback);
        self
    }

    /// Use a custom conversion function to convert from a string to a boolean value. If the string
    /// cannot be parsed, then this should return a `None`. If this happens while the parameter is
    /// being updated then the update will be canceled.
    pub fn with_string_to_value(
        mut self,
        callback: Arc<dyn Fn(&str) -> Option<bool> + Send + Sync>,
    ) -> Self {
        self.info.string_to_value = Some(callback);
        self
    }

    /// Mark this parameter as a bypass parameter. Plugin hosts can integrate this parameter into
    /// their UI. Only a single [`BoolParam`] can be a bypass parameter, and nice-plug will add one
    /// if you don't create one yourself. You will need to implement this yourself if your plugin
    /// introduces latency.
    pub fn make_bypass(mut self) -> Self {
        self.info.flags.insert(ParamFlags::BYPASS);
        self
    }

    /// Mark the parameter as non-automatable. This means that the parameter cannot be changed from
    /// an automation lane. The parameter can however still be manually changed by the user from
    /// either the plugin's own GUI or from the host's generic UI.
    pub fn non_automatable(mut self) -> Self {
        self.info.flags.insert(ParamFlags::NON_AUTOMATABLE);
        self
    }

    /// Hide the parameter in the host's generic UI for this plugin. This also implies
    /// `NON_AUTOMATABLE`. Setting this does not prevent you from changing the parameter in the
    /// plugin's editor GUI.
    pub fn hide(mut self) -> Self {
        self.info.flags.insert(ParamFlags::HIDDEN);
        self
    }
}
