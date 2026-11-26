use atomic_refcell::AtomicRefCell;
use crossbeam::queue::ArrayQueue;
use nice_plug_core::audio_setup::{AudioIOLayout, BufferConfig, ProcessMode};
use nice_plug_core::context::gui::GuiContext;
use nice_plug_core::context::process::Transport;
use nice_plug_core::editor::{Editor, EditorHandle, SpawnedEditor};
use nice_plug_core::midi::PluginNoteEvent;
use nice_plug_core::params::PluginState;
use nice_plug_core::plugin::{Plugin, ProcessStatus, SavedState, TaskExecutor};
use parking_lot::Mutex;
use std::error::Error;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::thread;

use super::backend::Backend;
use super::config::WrapperConfig;
use super::context::{WrapperActivateContext, WrapperProcessContext};
use crate::event_loop::{EventLoop, MainThreadExecutor, OsEventLoop};
use crate::wrapper::standalone::context::WrapperGuiContext;
use crate::wrapper::state;
use crate::wrapper::util::process_wrapper;

/// How many parameter changes we can store in our unprocessed parameter change queue. Storing more
/// than this many parameters at a time will cause changes to get lost.
const EVENT_QUEUE_CAPACITY: usize = 2048;

pub struct Wrapper<P: Plugin, B: Backend<P>> {
    backend: AtomicRefCell<B>,

    /// The wrapped plugin instance.
    plugin: Mutex<P>,
    /// The plugin's background task executor closure. Tasks scheduled by the plugin will be
    /// executed on the GUI or background thread using this function.
    pub task_executor: Mutex<TaskExecutor<P>>,
    /// Plugin state fetched once during initialization. Persistent fields are saved and restored
    /// through this object without locking `plugin`.
    plugin_state: Arc<dyn PluginState>,
    /// The plugin's editor, if it has one. This object does not do anything on its own, but we need
    /// to instantiate this in advance so we don't need to lock the entire [`Plugin`] object when
    /// creating an editor. Wrapped in an `AtomicRefCell` because it needs to be initialized late.
    #[allow(clippy::type_complexity)]
    pub editor: AtomicRefCell<Option<Arc<Mutex<P::Editor>>>>,
    #[allow(clippy::type_complexity)]
    pub editor_handle: Mutex<Option<<P::Editor as Editor>::Handle>>,
    close_requested: Arc<AtomicBool>,

    /// A realtime-safe task queue so the plugin can schedule tasks that need to be run later on the
    /// GUI thread. See the same field in the VST3 wrapper for more information on why this looks
    /// the way it does.
    event_loop: AtomicRefCell<Option<OsEventLoop<Task<P>, Self>>>,

    /// This is used to grab the DPI scaling config. Not used on macOS.
    #[allow(unused)]
    config: WrapperConfig,

    /// The bus and buffer configurations are static for the standalone target.
    audio_io_layout: AudioIOLayout,
    buffer_config: BufferConfig,

    /// Parameter changes from the GUI that the audio thread has not yet delivered as note events.
    /// The id is the parameter index.
    unprocessed_param_changes: ArrayQueue<(u32, f32)>,
    /// The current latency in samples, as set by the plugin through the [`ActivateContext`] and the
    /// [`ProcessContext`]. This value may not be used depending on the audio backend, but it's
    /// still kept track of to avoid firing debug assertions multiple times for the same latency
    /// value.
    current_latency: AtomicU32,
}

/// Tasks that can be sent from the plugin to be executed on the main thread in a non-blocking
/// realtime-safe way (either a random thread or `IRunLoop` on Linux, the OS' message loop on
/// Windows and macOS).
pub enum Task<P: Plugin> {
    /// Execute one of the plugin's background tasks.
    PluginTask(P::BackgroundTask),
}

/// Errors that may arise while initializing the wrapped plugins.
#[derive(Debug, thiserror::Error)]
pub enum WrapperError {
    /// The plugin returned `false` during activation.
    #[error("The plugin returned false during activation")]
    ActivationFailed,
    #[error("{0}")]
    WindowError(#[from] Box<dyn Error>),
}

impl<P: Plugin, B: Backend<P>> MainThreadExecutor<Task<P>> for Wrapper<P, B> {
    fn execute(&self, task: Task<P>, _is_gui_thread: bool) {
        let Task::PluginTask(task) = task;
        (self.task_executor.lock())(task);
    }
}

impl<P: Plugin, B: Backend<P>> Wrapper<P, B> {
    /// Instantiate a new instance of the standalone wrapper. Returns an error if the plugin does
    /// not accept the IO configuration from the wrapper config.
    pub fn new(backend: B, config: WrapperConfig) -> Result<Arc<Self>, WrapperError> {
        // The backend has already queried this, so this will never cause the program to exit
        // TODO: Do the validation and parsing in the argument parser so this value can be stored on
        //       the config itself. Right now clap doesn't support this.
        let audio_io_layout = config.audio_io_layout_or_exit::<P>();

        let mut plugin = P::default();
        let task_executor = Mutex::new(plugin.task_executor());
        let plugin_state = plugin.state();

        let wrapper = Arc::new(Wrapper {
            backend: AtomicRefCell::new(backend),

            plugin: Mutex::new(plugin),
            task_executor,
            plugin_state,
            // Initialized later as it needs a reference to the wrapper for the async executor
            editor: AtomicRefCell::new(None),
            editor_handle: Mutex::new(None),
            close_requested: Arc::new(AtomicBool::new(false)),

            // Also initialized later as it also needs a reference to the wrapper
            event_loop: AtomicRefCell::new(None),

            audio_io_layout,
            buffer_config: BufferConfig {
                sample_rate: config.sample_rate,
                min_buffer_size: None,
                max_buffer_size: config.period_size,
                // TODO: Detect JACK freewheeling and report it here
                process_mode: ProcessMode::Realtime,
            },
            config,

            unprocessed_param_changes: ArrayQueue::new(EVENT_QUEUE_CAPACITY),
            current_latency: AtomicU32::new(0),
        });

        *wrapper.event_loop.borrow_mut() =
            Some(OsEventLoop::new_and_spawn(Arc::downgrade(&wrapper)));

        // The editor needs to be initialized later so the Async executor can work.
        {
            *wrapper.editor.borrow_mut() = wrapper
                .plugin
                .lock()
                .editor(nice_plug_core::context::gui::AsyncExecutor::new(
                    Arc::new({
                        let wrapper = wrapper.clone();

                        move |task| {
                            let task_posted = wrapper.schedule_background(Task::PluginTask(task));
                            crate::nice_debug_assert!(
                                task_posted,
                                "The task queue is full, dropping task..."
                            );
                        }
                    }),
                    Arc::new({
                        let wrapper = wrapper.clone();

                        move |task| {
                            let task_posted = wrapper.schedule_gui(Task::PluginTask(task));
                            crate::nice_debug_assert!(
                                task_posted,
                                "The task queue is full, dropping task..."
                            );
                        }
                    }),
                ))
                .map(|editor| {
                    // Safety: This is only called in standalone mode, and only if the user has
                    // enabled the `unsafe_standalone_dpi_fix` feature.
                    #[cfg(feature = "unsafe_standalone_dpi_fix")]
                    unsafe {
                        editor.assume_standalone_in_process();
                    }

                    Arc::new(Mutex::new(editor))
                });
        }

        {
            let mut plugin = wrapper.plugin.lock();
            if !plugin.activate(
                &wrapper.audio_io_layout,
                &wrapper.buffer_config,
                &mut wrapper.make_activate_context(),
            ) {
                return Err(WrapperError::ActivationFailed);
            }
            process_wrapper(|| plugin.reset());
        }

        Ok(wrapper)
    }

    /// Open the editor, start processing audio, and block this thread until the editor is closed.
    /// If the plugin does not have an editor, then this will block until SIGINT is received.
    ///
    /// Will return an error if the plugin threw an error during audio processing or if the editor
    /// could not be opened.
    pub fn run(self: Arc<Self>) -> Result<(), WrapperError> {
        let close_gui_requested = Arc::clone(&self.close_requested);
        if let Err(e) = ctrlc::set_handler(move || {
            close_gui_requested.store(true, Ordering::Relaxed);
        }) {
            crate::nice_error!("Error setting close signal callback {}", e);
        }

        // We'll spawn a separate thread to handle IO and to process audio. This audio thread should
        // terminate together with this function.
        let terminate_audio_thread = Arc::new(AtomicBool::new(false));
        let close_gui_requested = Arc::clone(&self.close_requested);
        let audio_thread = {
            let this = self.clone();
            let terminate_audio_thread = terminate_audio_thread.clone();
            thread::spawn(move || {
                this.run_audio_thread(terminate_audio_thread, close_gui_requested)
            })
        };

        #[allow(unused_mut)]
        let mut gui_spawned = false;

        let res = match self.editor.borrow().clone() {
            Some(editor) => {
                let context = self.clone().make_gui_context();

                match editor.lock().spawn(None, false, None, context, None) {
                    Ok(editor_window) => {
                        let SpawnedEditor {
                            handle: instance,
                            window,
                        } = editor_window;

                        gui_spawned = true;
                        *self.editor_handle.lock() = Some(instance);

                        <P::Editor as Editor>::Handle::run_until_closed(window)
                            .map_err(|e| WrapperError::WindowError(e.into()))
                    }
                    Err(e) => Err(WrapperError::WindowError(e)),
                }
            }
            None => Ok(()),
        };

        if !gui_spawned {
            crate::nice_log!("No GUI available for {}, blocking indefinitely...", P::NAME);

            loop {
                if self.close_requested.swap(false, Ordering::Relaxed) {
                    break;
                }

                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        }

        terminate_audio_thread.store(true, Ordering::SeqCst);

        let _ = audio_thread.join();

        // Some plugins may use this to clean up resources. Should not be needed for the standalone
        // application, but it seems like a good idea to stay consistent.
        self.plugin.lock().deactivate();

        res
    }

    pub(crate) fn has_param(&self, index: u32) -> bool {
        index < self.plugin_state.param_count()
    }

    /// Queue a parameter change. The audio thread delivers it as a note event.
    pub fn set_parameter(&self, index: u32, normalized: f32) -> bool {
        if !self.has_param(index) {
            return false;
        }

        let push_successful = self
            .unprocessed_param_changes
            .push((index, normalized))
            .is_ok();
        crate::nice_debug_assert!(push_successful, "The parameter change queue was full");

        push_successful
    }

    pub(crate) fn pop_param_change(&self) -> Option<(u32, f32)> {
        self.unprocessed_param_changes.pop()
    }

    /// Get the plugin's state object, may be called by the plugin's GUI as part of its own preset
    /// management. The wrapper doesn't use these functions and serializes and deserializes directly
    /// the JSON in the relevant plugin API methods instead.
    pub fn get_state_object(&self) -> SavedState {
        state::serialize_object::<P>(self.plugin_state.clone())
    }

    /// Posts the task to the background task queue using [`EventLoop::schedule_background()`] so it
    /// can be run in the background without blocking either the GUI or the audio thread.
    ///
    /// If the task queue is full, then this will return false.
    #[must_use]
    pub fn schedule_background(&self, task: Task<P>) -> bool {
        let event_loop = self.event_loop.borrow();
        let event_loop = event_loop.as_ref().unwrap();
        event_loop.schedule_background(task)
    }

    /// Posts the task to the task queue using [`EventLoop::schedule_gui()`] so it can be delegated
    /// to the main thread. The task is run directly if this is the GUI thread.
    ///
    /// If the task queue is full, then this will return false.
    #[must_use]
    pub fn schedule_gui(&self, task: Task<P>) -> bool {
        let event_loop = self.event_loop.borrow();
        let event_loop = event_loop.as_ref().unwrap();
        event_loop.schedule_gui(task)
    }

    pub fn set_latency_samples(&self, samples: u32) {
        // This should only change the value if it's actually needed
        let old_latency = self.current_latency.swap(samples, Ordering::SeqCst);
        if old_latency != samples {
            // None of the backends actually support this at the moment
            crate::nice_debug_assert_failure!(
                "Standalones currently don't support latency reporting"
            );
        }
    }

    /// The audio thread. This should be called from another thread, and it will run until
    /// `should_terminate` is `true`.
    fn run_audio_thread(
        self: Arc<Self>,
        should_terminate: Arc<AtomicBool>,
        close_gui_requested: Arc<AtomicBool>,
    ) {
        self.clone().backend.borrow_mut().run(
            move |buffer, aux, transport, input_events, output_events| {
                // TODO: This process wrapper should actually be in the backends (since the backends
                //       should also not allocate in their audio callbacks), but that's a bit more
                //       error prone
                process_wrapper(|| {
                    if should_terminate.load(Ordering::SeqCst) {
                        return false;
                    }

                    {
                        let mut plugin = self.plugin.lock();
                        if let ProcessStatus::Error(err) = plugin.process(
                            buffer,
                            aux,
                            &mut self.make_process_context(transport, input_events, output_events),
                        ) {
                            crate::nice_error!("The plugin returned an error while processing:");
                            crate::nice_error!("{}", err);

                            close_gui_requested.store(true, Ordering::Relaxed);

                            return false;
                        }
                    }

                    true
                })
            },
        );
    }

    fn make_gui_context(self: Arc<Self>) -> GuiContext {
        GuiContext::new(Arc::new(WrapperGuiContext {
            wrapper: Arc::downgrade(&self),
            #[cfg(debug_assertions)]
            param_gesture_checker: Default::default(),
        }))
    }

    fn make_activate_context(&self) -> WrapperActivateContext<'_, P, B> {
        WrapperActivateContext { wrapper: self }
    }

    fn make_process_context<'a>(
        &'a self,
        transport: Transport,
        input_events: &'a [PluginNoteEvent<P>],
        output_events: &'a mut Vec<PluginNoteEvent<P>>,
    ) -> WrapperProcessContext<'a, P, B> {
        WrapperProcessContext {
            wrapper: self,
            input_events,
            input_events_idx: 0,
            output_events,
            transport,
        }
    }

}
