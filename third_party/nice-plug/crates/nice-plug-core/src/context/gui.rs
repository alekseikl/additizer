//! A context passed to a plugin's editor.

use std::sync::Arc;

use crate::plugin::{Plugin, SavedState};

use super::PluginApi;

/// Callbacks the plugin can make when the user interacts with its GUI such as updating parameter
/// values. This is passed to the plugin during [`Editor::spawn()`][crate::editor::Editor::spawn()].
/// All of these functions assume they're being called from the main GUI thread.
#[derive(Clone)]
pub struct GuiContext {
    inner: Arc<dyn GuiContextInner>,
}

impl GuiContext {
    pub fn new(inner: Arc<dyn GuiContextInner>) -> Self {
        Self { inner }
    }

    /// Get the current plugin API. This may be useful to display in the plugin's GUI as part of an
    /// about screen.
    pub fn plugin_api(&self) -> PluginApi {
        self.inner.plugin_api()
    }

    pub fn param_setter<'a>(&'a self) -> ParamSetter<'a> {
        ParamSetter {
            raw_context: &*self.inner,
        }
    }

    /// Inform the host a parameter will be automated. `index` is the parameter id.
    ///
    /// # Safety
    ///
    /// The implementing function still needs to check if `index` is in range.
    pub unsafe fn raw_begin_set_parameter(&self, index: u32) {
        unsafe {
            self.inner.raw_begin_set_parameter(index);
        }
    }

    /// Inform the host a parameter is being automated with an already normalized value. `index` is
    /// the parameter id.
    ///
    /// # Safety
    ///
    /// The implementing function still needs to check if `index` is in range.
    pub unsafe fn raw_set_parameter_normalized(&self, index: u32, normalized: f32) {
        unsafe {
            self.inner.raw_set_parameter_normalized(index, normalized);
        }
    }

    /// Inform the host a parameter has been automated. `index` is the parameter id.
    ///
    /// # Safety
    ///
    /// The implementing function still needs to check if `index` is in range.
    pub unsafe fn raw_end_set_parameter(&self, index: u32) {
        unsafe {
            self.inner.raw_end_set_parameter(index);
        }
    }

    /// Serialize the plugin's current state to a serde-serializable object. Useful for implementing
    /// preset handling within a plugin's GUI.
    pub fn get_state(&self) -> SavedState {
        self.inner.get_state()
    }

    /// Request the plugin to be restarted.
    pub fn request_restart(&self) {
        self.inner.request_restart();
    }

    /// Ask the host to re-read every parameter. The parameter count and ids stay the same.
    /// Names, flags, text, and values are queried again.
    pub fn request_param_rescan(&self) {
        self.inner.request_param_rescan();
    }
}

/// Callbacks the plugin can make when the user interacts with its GUI such as updating parameter
/// values. This is passed to the plugin during [`Editor::spawn()`][crate::editor::Editor::spawn()].
/// All of these functions assume they're being called from the main GUI thread.
//
// # Safety
//
// The implementing wrapper can assume that everything is being called from the main thread. Since
// nice-plug doesn't own the GUI event loop, this invariant cannot be part of the interface.
pub trait GuiContextInner: Send + Sync + 'static {
    /// Get the current plugin API. This may be useful to display in the plugin's GUI as part of an
    /// about screen.
    fn plugin_api(&self) -> PluginApi;

    /// Inform the host a parameter will be automated. `index` is the parameter id.
    ///
    /// # Safety
    ///
    /// The implementing function still needs to check if `index` is in range.
    unsafe fn raw_begin_set_parameter(&self, index: u32);

    /// Inform the host a parameter is being automated with an already normalized value.
    ///
    /// # Safety
    ///
    /// The implementing function still needs to check if `index` is in range.
    unsafe fn raw_set_parameter_normalized(&self, index: u32, normalized: f32);

    /// Inform the host a parameter has been automated.
    ///
    /// # Safety
    ///
    /// The implementing function still needs to check if `index` is in range.
    unsafe fn raw_end_set_parameter(&self, index: u32);

    /// Serialize the plugin's current state to a serde-serializable object. Useful for implementing
    /// preset handling within a plugin's GUI.
    fn get_state(&self) -> SavedState;

    /// Request the plugin to be restarted.
    fn request_restart(&self);

    /// Ask the host to re-read every parameter. The parameter count and ids stay the same.
    fn request_param_rescan(&self);
}

/// An way to run background tasks from the plugin's GUI, equivalent to the
/// [`ProcessContext::execute_background()`][crate::context::process::ProcessContext::execute_background()]
/// and [`ProcessContext::execute_gui()`][crate::context::process::ProcessContext::execute_gui()]
/// functions. This is passed directly to [`Plugin::editor()`] so the plugin can move it into its
/// editor and use it later.
///
/// # Note
///
/// This is only intended to be used from the GUI. Use the methods on
/// [`ActivateContext`][crate::context::activate::ActivateContext] and
/// [`ProcessContext`][crate::context::process::ProcessContext] to run tasks during the `activate()`
/// and `process()` functions.
//
// NOTE: This is separate from `GuiContext` because adding a type parameter there would clutter up a
//       lot of structs, and may even be incompatible with the way certain GUI libraries work.
pub struct AsyncExecutor<P: Plugin> {
    pub(crate) execute_background: Arc<dyn Fn(P::BackgroundTask) + Send + Sync>,
    pub(crate) execute_gui: Arc<dyn Fn(P::BackgroundTask) + Send + Sync>,
}

impl<P: Plugin> AsyncExecutor<P> {
    pub fn new(
        execute_background: Arc<dyn Fn(P::BackgroundTask) + Send + Sync>,
        execute_gui: Arc<dyn Fn(P::BackgroundTask) + Send + Sync>,
    ) -> Self {
        Self {
            execute_background,
            execute_gui,
        }
    }
}

// Can't derive this since Rust then requires `P` to also be `Clone`able
impl<P: Plugin> Clone for AsyncExecutor<P> {
    fn clone(&self) -> Self {
        Self {
            execute_background: self.execute_background.clone(),
            execute_gui: self.execute_gui.clone(),
        }
    }
}

/// A convenience helper for setting parameter values from the GUI thread. Changes are sent to the
/// host. The parameter id is its index. The host delivers the value back as a note event.
pub struct ParamSetter<'a> {
    pub raw_context: &'a dyn GuiContextInner,
}

impl<P: Plugin> AsyncExecutor<P> {
    /// Execute a task on a background thread using `[Plugin::task_executor]`. This allows you to
    /// defer expensive tasks for later without blocking either the process function or the GUI
    /// thread. As long as creating the `task` is realtime-safe, this operation is too.
    ///
    /// # Note
    ///
    /// Scheduling the same task multiple times will cause those duplicate tasks to pile up. Try to
    /// either prevent this from happening, or check whether the task still needs to be completed in
    /// your task executor.
    pub fn execute_background(&self, task: P::BackgroundTask) {
        (self.execute_background)(task);
    }

    /// Execute a task on a background thread using `[Plugin::task_executor]`.
    ///
    /// # Note
    ///
    /// Scheduling the same task multiple times will cause those duplicate tasks to pile up. Try to
    /// either prevent this from happening, or check whether the task still needs to be completed in
    /// your task executor.
    pub fn execute_gui(&self, task: P::BackgroundTask) {
        (self.execute_gui)(task);
    }
}

impl<'a> ParamSetter<'a> {
    pub fn new(context: &'a dyn GuiContextInner) -> Self {
        Self {
            raw_context: context,
        }
    }

    /// Inform the host that you will start automating a parameter. Call this before
    /// [`set_parameter_normalized()`][Self::set_parameter_normalized()] for that index.
    pub fn begin_set_parameter(&self, index: u32) {
        unsafe { self.raw_context.raw_begin_set_parameter(index) };
    }

    /// Set a parameter to an already normalized value. Call
    /// [`begin_set_parameter()`][Self::begin_set_parameter()] before and
    /// [`end_set_parameter()`][Self::end_set_parameter()] after this so the host can record
    /// automation. This can be called multiple times in a row before
    /// [`end_set_parameter()`][Self::end_set_parameter()], for instance while a slider moves.
    ///
    /// Call this from the GUI thread.
    pub fn set_parameter_normalized(&self, index: u32, normalized: f32) {
        unsafe {
            self.raw_context
                .raw_set_parameter_normalized(index, normalized)
        };
    }

    /// Inform the host that you are done automating a parameter.
    pub fn end_set_parameter(&self, index: u32) {
        unsafe { self.raw_context.raw_end_set_parameter(index) };
    }
}
