//! Continuous (or discrete, with a step size) floating point parameters.

use atomic_float::AtomicF32;
use std::fmt::{Debug, Display};
use std::sync::Arc;
use std::sync::atomic::Ordering;

use super::internals::ParamPtr;
use super::range::FloatRange;
use super::{InternalParamMut, Param, ParamFlags, ParamInfo};

/// A floating point parameter. The range converts between plain and normalized values for the host.
/// The plugin receives value changes as note events.
pub struct FloatParam {
    /// Last normalized value stored for host queries.
    normalized_value: AtomicF32,
    /// The field's default plain, unnormalized value.
    default: f32,
    /// The distribution of the parameter's values.
    range: FloatRange,
    /// The distance between discrete steps in this parameter. Mostly useful for quantizing GUI
    /// input. If this is set and if [`value_to_string`][Self::with_value_to_string] is not set, then
    /// this is also used when formatting the parameter. This must be a positive, nonzero number.
    step_size: Option<f32>,
    /// Metadata and conversion callbacks that are not used by the DSP hot path.
    info: Box<ParamInfo<f32>>,
    /// If this parameter has been marked as polyphonically modulatable, then this will be a unique
    /// integer identifying the parameter. Because this value is determined by the plugin itself,
    /// the plugin can easily map
    /// [`NoteEvent::PolyModulation`][crate::prelude::NoteEvent::PolyModulation] events to the
    /// correct parameter by pattern matching on a constant.
    poly_modulation_id: Option<u32>,
}

impl Display for FloatParam {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = self.modulated_plain_value();
        match (&self.info.value_to_string, &self.step_size) {
            (Some(func), _) => write!(f, "{}{}", func(value), self.info.unit),
            (None, Some(step_size)) => {
                let num_digits = decimals_from_step_size(*step_size);
                write!(f, "{:.num_digits$}{}", value, self.info.unit)
            }
            _ => write!(f, "{}{}", value, self.info.unit),
        }
    }
}

impl Debug for FloatParam {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.info.name, self)
    }
}

// `Params` can not be implemented outside of nice-plug itself because `ParamPtr` is also closed
impl super::Sealed for FloatParam {}

impl Param for FloatParam {
    type Plain = f32;

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
        None
    }

    fn normalized_value_to_string(&self, normalized: f32, include_unit: bool) -> String {
        let value = self.preview_plain(normalized);
        match (&self.info.value_to_string, &self.step_size, include_unit) {
            (Some(f), _, true) => format!("{}{}", f(value), self.info.unit),
            (Some(f), _, false) => f(value),
            (None, Some(step_size), true) => {
                let num_digits = decimals_from_step_size(*step_size);
                format!("{:.num_digits$}{}", value, self.info.unit)
            }
            (None, Some(step_size), false) => {
                let num_digits = decimals_from_step_size(*step_size);
                format!("{value:.num_digits$}")
            }
            (None, None, true) => format!("{}{}", value, self.info.unit),
            (None, None, false) => format!("{value}"),
        }
    }

    fn string_to_normalized_value(&self, string: &str) -> Option<f32> {
        let value = match &self.info.string_to_value {
            Some(f) => f(string.trim()),
            // In the CLAP wrapper the unit will be included, so make sure to handle that
            None => string.trim().trim_end_matches(self.info.unit).parse().ok(),
        }?;

        Some(self.preview_normalized(value))
    }

    #[inline]
    fn preview_normalized(&self, plain: Self::Plain) -> f32 {
        self.range.normalize(plain)
    }

    #[inline]
    fn preview_plain(&self, normalized: f32) -> Self::Plain {
        let value = self.range.unnormalize(normalized);
        match &self.step_size {
            Some(step_size) => self.range.snap_to_step(value, *step_size as Self::Plain),
            None => value,
        }
    }

    fn flags(&self) -> ParamFlags {
        self.info.flags
    }

    fn as_ptr(&self) -> ParamPtr {
        ParamPtr::FloatParam(self as *const _ as *mut _)
    }
}

impl InternalParamMut for FloatParam {
    unsafe fn _internal_set_plain_value(&self, plain: Self::Plain) -> bool {
        unsafe { self._internal_set_normalized_value(self.preview_normalized(plain)) }
    }

    unsafe fn _internal_set_normalized_value(&self, normalized: f32) -> bool {
        let normalized = self.preview_normalized(self.preview_plain(normalized));
        let old = self.normalized_value.swap(normalized, Ordering::Relaxed);
        normalized != old
    }
}

impl FloatParam {
    /// Build a new [`FloatParam`]. Use the other associated functions to modify the behavior of the
    /// parameter.
    pub fn new(name: impl Into<String>, default: f32, range: FloatRange) -> Self {
        range.assert_validity();

        Self {
            normalized_value: AtomicF32::new(range.normalize(default)),
            default,
            range,
            step_size: None,
            info: Box::new(ParamInfo::new(name)),
            poly_modulation_id: None,
        }
    }

    /// The range of valid plain values for this parameter.
    #[inline]
    pub fn range(&self) -> FloatRange {
        self.range
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

    /// Display a unit when rendering this parameter to a string. Appended after the
    /// [`value_to_string`][Self::with_value_to_string()] function if that is also set. nice-plug
    /// will not automatically add a space before the unit.
    pub fn with_unit(mut self, unit: &'static str) -> Self {
        self.info.unit = unit;
        self
    }

    /// Set the distance between steps of a [`FloatParam`]. Mostly useful for quantizing GUI input. If
    /// this is set and a [`value_to_string`][Self::with_value_to_string()] function is not set,
    /// then this is also used when formatting the parameter. This must be a positive, nonzero
    /// number.
    pub fn with_step_size(mut self, step_size: f32) -> Self {
        self.step_size = Some(step_size);
        self
    }

    /// Use a custom conversion function to convert the plain, unnormalized value to a
    /// string.
    pub fn with_value_to_string(
        mut self,
        callback: Arc<dyn Fn(f32) -> String + Send + Sync>,
    ) -> Self {
        self.info.value_to_string = Some(callback);
        self
    }

    /// Use a custom conversion function to convert from a string to a plain, unnormalized
    /// value. If the string cannot be parsed, then this should return a `None`. If this
    /// happens while the parameter is being updated then the update will be canceled.
    ///
    /// The input string may or may not contain the unit, so you will need to be able to handle
    /// that.
    pub fn with_string_to_value(
        mut self,
        callback: Arc<dyn Fn(&str) -> Option<f32> + Send + Sync>,
    ) -> Self {
        self.info.string_to_value = Some(callback);
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

/// Calculate how many decimals to round to when displaying a floating point value with a specific
/// step size. We'll perform some rounding to ignore spurious extra precision caused by the floating
/// point quantization.
fn decimals_from_step_size(step_size: f32) -> usize {
    const SCALE: f32 = 1_000_000.0; // 10.0f32.powi(f32::DIGITS as i32)
    let step_size = (step_size * SCALE).round() / SCALE;

    let mut num_digits = 0;
    for decimals in 0..f32::DIGITS as i32 {
        if step_size * 10.0f32.powi(decimals) >= 1.0 {
            num_digits = decimals;
            break;
        }
    }

    num_digits as usize
}
