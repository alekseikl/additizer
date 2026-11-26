use atomic_refcell::AtomicRefCell;
use crossbeam::atomic::AtomicCell;
use nice_plug_core::audio_setup::{AudioIOLayout, BufferConfig, ProcessMode};
use nice_plug_core::context::gui::GuiContext;
use nice_plug_core::context::process::Transport;
use nice_plug_core::editor::dpi::Size;
use nice_plug_core::editor::{Editor, SpawnedEditor};
use nice_plug_core::midi::{MidiConfig, PluginNoteEvent};
use crossbeam::queue::ArrayQueue;
use nice_plug_core::params::PluginState;
use nice_plug_core::plugin::TrackInfo;
use nice_plug_core::plugin::{Plugin, ProcessStatus, SavedState, TaskExecutor};
use parking_lot::Mutex;
use parking_lot::RwLock;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use try_lock::TryLock;
use vst3::ComWrapper;
use vst3::Steinberg::Vst::{IComponentHandler, IComponentHandlerTrait, IEventList, RestartFlags_};

/// Re-read titles, flags, and values. The parameter count and ids stay the same.
const PARAM_RESCAN_FLAGS: i32 =
    RestartFlags_::kParamValuesChanged | RestartFlags_::kParamTitlesChanged;
use vst3::Steinberg::kResultOk;
use vst3::{ComPtr, ComRef};

use super::context::{WrapperActivateContext, WrapperProcessContext};
use super::note_expressions::NoteExpressionController;
use super::param_units::ParamUnits;
use super::util::{VST3_MIDI_PARAMS_END, VST3_MIDI_PARAMS_START};
use crate::event_loop::{EventLoop, MainThreadExecutor, OsEventLoop};
use crate::util::permit_alloc;
use crate::wrapper::state;
use crate::wrapper::util::buffer_management::BufferManager;
use crate::wrapper::vst3::Vst3Plugin;
use crate::wrapper::vst3::context::WrapperGuiContext;
use crate::wrapper::vst3::view::WrapperView;

/// The actual wrapper bits. We need this as an `Arc<T>` so we can safely use our event loop API.
/// Since we can't combine that with VST3's interior reference counting this just has to be moved to
/// its own struct.
pub(crate) struct WrapperInner<P: Vst3Plugin> {
    /// The wrapped plugin instance.
    pub plugin: TryLock<P>,
    /// The plugin's background task executor closure.
    pub task_executor: Mutex<TaskExecutor<P>>,
    /// Plugin state fetched once during initialization. Persistent fields are saved and restored
    /// through this object without locking `plugin`. Host parameter queries go through this object.
    /// The parameter id is its index.
    pub plugin_state: Arc<dyn PluginState>,
    /// Parameter value changes that arrived while the plugin was not processing. The next process
    /// call delivers them as note events. The id is the parameter index.
    pub pending_automation: ArrayQueue<(u32, f32)>,
    /// The plugin's editor, if it has one. This object does not do anything on its own, but we need
    /// to instantiate this in advance so we don't need to lock the entire [`Plugin`] object when
    /// creating an editor. Wrapped in an `AtomicRefCell` because it needs to be initialized late.
    #[allow(clippy::type_complexity)]
    pub editor: AtomicRefCell<Option<Arc<Mutex<P::Editor>>>>,
    #[allow(clippy::type_complexity)]
    // My personal record for craziest Rust type!!!!
    pub editor_window:
        Arc<AtomicRefCell<Option<fragile::Fragile<SpawnedEditor<<P::Editor as Editor>::Handle>>>>>,

    /// The host's [`IComponentHandler`] instance, if passed through
    /// [`IEditController::set_component_handler`].
    pub component_handler: AtomicRefCell<Option<ComPtr<IComponentHandler>>>,

    /// Our own [`IPlugView`] instance.
    pub plug_view: RwLock<Option<ComWrapper<WrapperView<P>>>>,

    /// Whether the editor is currently open. This is changed by the attached and removed functions on IPlugViewTrait
    pub is_editor_open: AtomicBool,

    /// A realtime-safe task queue so the plugin can schedule tasks that need to be run later on the
    /// GUI thread. This field should not be used directly for posting tasks. This should be done
    /// through [`Self::schedule_gui()`] instead. That method posts the task to the host's
    /// `IRunLoop` instead of it's available.
    ///
    /// This AtomicRefCell+Option is only needed because it has to be initialized late. There is no
    /// reason to mutably borrow the event loop, so reads will never be contested.
    ///
    /// TODO: Is there a better type for Send+Sync late initialization?
    pub event_loop: AtomicRefCell<Option<OsEventLoop<Task<P>, Self>>>,

    /// Whether the plugin is currently processing audio. In other words, the last state
    /// `IAudioProcessor::setActive()` has been called with.
    pub is_processing: AtomicBool,
    /// The current audio IO layout. Modified through `IAudioProcessor::setBusArrangements()` after
    /// matching the proposed bus arrangement to one of the supported ones. The plugin's first audio
    /// IO layout is chosen as the default. Because of the way VST3 works it's not possible to
    /// change the number of busses from that default, only the channel counts can change.
    pub current_audio_io_layout: AtomicCell<AudioIOLayout>,
    /// The current buffer configuration, containing the sample rate and the maximum block size.
    /// Will be set in `IAudioProcessor::setupProcessing()`.
    pub current_buffer_config: AtomicCell<Option<BufferConfig>>,
    /// The current audio processing mode. Set in `IAudioProcessor::setup_processing()`.
    pub current_process_mode: AtomicCell<ProcessMode>,
    /// The most recently reported track information. Hosts may send partial updates (e.g.
    /// Ableton), so this is used to merge successive [`IInfoListener::setChannelContextInfos()`]
    /// calls.
    pub current_track_info: AtomicRefCell<TrackInfo>,

    /// The last process status returned by the plugin. This is used for tail handling.
    pub last_process_status: AtomicCell<ProcessStatus>,
    /// The current latency in samples, as set by the plugin through the [`ActivateContext`] and the
    /// [`ProcessContext`].
    pub current_latency: AtomicU32,
    /// A data structure that helps manage and create buffers for all of the plugin's inputs and
    /// outputs based on channel pointers provided by the host.
    pub buffer_manager: AtomicRefCell<BufferManager>,
    /// The incoming note events for the plugin, if `P::MIDI_INPUT` is set.
    ///
    /// NOTE: Because with VST3 MIDI CC messages are sent as parameter changes and VST3 does not
    ///       interleave parameter changes and note events, this queue has to be sorted when
    ///       creating the process context
    pub input_note_events: AtomicRefCell<VecDeque<PluginNoteEvent<P>>>,
    /// VST3 has several useful predefined note expressions, but for some reason they are the only
    /// note event type that don't have MIDI note ID and channel fields. So we need to keep track of
    /// the most recent VST3 note IDs we've seen, and then map those back to MIDI note IDs and
    /// channels as needed.
    pub note_expression_controller: AtomicRefCell<NoteExpressionController>,
    /// Note events for this process call, including MIDI CC messages and `MonoAutomation` events
    /// that VST3 delivers as parameter changes. Parameter values themselves are applied
    /// immediately. The queues arrive separately, so they are merged here and sorted by sample
    /// offset before the plugin processes the whole host buffer in one call.
    pub process_events: AtomicRefCell<Vec<ProcessEvent<P>>>,

    pub param_units: ParamUnits,
}

/// Tasks that can be sent from the plugin to be executed on the main thread in a non-blocking
/// realtime-safe way (either a random thread or `IRunLoop` on Linux, the OS' message loop on
/// Windows and macOS).
#[allow(clippy::enum_variant_names)]
pub enum Task<P: Plugin> {
    /// Execute one of the plugin's background tasks.
    PluginTask(P::BackgroundTask),
    /// Inform the plugin that one or more parameter values have changed.
    StateChanged,
    /// Trigger a restart with the given restart flags. This is a bit set of the flags from
    /// [`vst3::Steinberg::Vst::RestartFlags`].
    TriggerRestart(i32),
    // Request the editor to be resized according to its current size. Right now there is no way to
    // handle "denied resize" requests yet.
    RequestResize {
        size: Size,
        scale_factor: f64,
    },
    CallMainThread,
}

/// VST3 delivers MIDI CC messages as parameter changes, separate from the note-event list. Both are
/// collected here and sorted by sample offset before the plugin processes the host buffer.
#[derive(Debug, PartialEq)]
pub enum ProcessEvent<P: Plugin> {
    /// A note event, or a MIDI CC / channel pressure / pitch bend message translated from a
    /// parameter change. Timings are sample offsets within the host buffer.
    NoteEvent(PluginNoteEvent<P>),
}

impl<P: Plugin> ProcessEvent<P> {
    #[inline]
    pub(crate) fn timing(&self) -> u32 {
        let ProcessEvent::NoteEvent(event) = self;
        event.timing()
    }
}

impl<P: Vst3Plugin> WrapperInner<P> {
    #[allow(unused_unsafe)]
    pub fn new() -> Arc<Self> {
        let mut plugin = P::default();
        let task_executor = Mutex::new(plugin.task_executor());

        let plugin_state = plugin.state();
        let param_count = plugin_state.param_count();
        let param_units = ParamUnits::from_param_groups((0..param_count).map(|index| (index, "")))
            .expect("Inconsistent parameter groups");

        if cfg!(debug_assertions) && P::MIDI_INPUT >= MidiConfig::MidiCCs {
            for index in 0..param_count {
                if (VST3_MIDI_PARAMS_START..VST3_MIDI_PARAMS_END).contains(&index) {
                    crate::nice_debug_assert_failure!(
                        "Parameter index {} collides with an automatically generated MIDI CC \
                         parameter",
                        index
                    );
                }
            }
        }

        let input_note_events = if P::MIDI_INPUT == MidiConfig::None {
            AtomicRefCell::new(VecDeque::new())
        } else {
            AtomicRefCell::new(VecDeque::with_capacity(P::INPUT_EVENT_CAPACITY))
        };

        let wrapper = Arc::new(Self {
            plugin: TryLock::new(plugin),
            task_executor,
            plugin_state,
            pending_automation: ArrayQueue::new(2048),
            // Initialized later as it needs a reference to the wrapper for the async executor
            editor: AtomicRefCell::new(None),
            editor_window: Arc::new(AtomicRefCell::new(None)),

            component_handler: AtomicRefCell::new(None),

            plug_view: RwLock::new(None),

            is_editor_open: AtomicBool::new(false),

            event_loop: AtomicRefCell::new(None),

            is_processing: AtomicBool::new(false),
            // Some hosts, like the current version of Bitwig and Ardour at the time of writing,
            // will try using the plugin's default not yet initialized bus arrangement. Because of
            // that, we'll always initialize this configuration even before the host requests a
            // channel layout.
            current_audio_io_layout: AtomicCell::new(
                P::AUDIO_IO_LAYOUTS.first().copied().unwrap_or_default(),
            ),
            current_buffer_config: AtomicCell::new(None),
            current_process_mode: AtomicCell::new(ProcessMode::Realtime),
            current_track_info: AtomicRefCell::new(TrackInfo::default()),
            last_process_status: AtomicCell::new(ProcessStatus::Normal),
            current_latency: AtomicU32::new(0),
            // This is initialized just before calling `Plugin::activate()` so that during the
            // process call buffers can be initialized without any allocations
            buffer_manager: AtomicRefCell::new(BufferManager::for_audio_io_layout(
                0,
                AudioIOLayout::default(),
            )),
            input_note_events,
            note_expression_controller: AtomicRefCell::new(NoteExpressionController::default()),
            process_events: AtomicRefCell::new(Vec::with_capacity(P::INPUT_EVENT_CAPACITY)),

            param_units,
        });

        // FIXME: Right now this is safe, but if we are going to have a singleton main thread queue
        //        serving multiple plugin instances, Arc can't be used because its reference count
        //        is separate from the internal COM-style reference count.
        *wrapper.event_loop.borrow_mut() =
            Some(OsEventLoop::new_and_spawn(Arc::downgrade(&wrapper)));

        // The editor also needs to be initialized later so the Async executor can work.

        {
            *wrapper.editor.borrow_mut() = wrapper
                .plugin
                .try_lock()
                .unwrap()
                .editor(nice_plug_core::context::gui::AsyncExecutor::new(
                    Arc::new({
                        let wrapper = Arc::downgrade(&wrapper);
                        move |task| {
                            let wrapper = match wrapper.upgrade() {
                                Some(wrapper) => wrapper,
                                None => return,
                            };

                            let task_posted = wrapper.schedule_background(Task::PluginTask(task));
                            crate::nice_debug_assert!(
                                task_posted,
                                "The task queue is full, dropping task..."
                            );
                        }
                    }),
                    Arc::new({
                        let wrapper = Arc::downgrade(&wrapper);
                        move |task| {
                            let wrapper = match wrapper.upgrade() {
                                Some(wrapper) => wrapper,
                                None => return,
                            };

                            let task_posted = wrapper.schedule_gui(Task::PluginTask(task));
                            crate::nice_debug_assert!(
                                task_posted,
                                "The task queue is full, dropping task..."
                            );
                        }
                    }),
                ))
                .map(|editor| Arc::new(Mutex::new(editor)));
        }

        wrapper
    }

    pub fn make_gui_context(self: Arc<Self>) -> GuiContext {
        GuiContext::new(Arc::new(WrapperGuiContext {
            inner: Arc::downgrade(&self),
            #[cfg(debug_assertions)]
            param_gesture_checker: Default::default(),
        }))
    }

    /// # Note
    ///
    /// The lock on the plugin must be dropped before this object is dropped to avoid deadlocks
    /// caused by reentrant function calls.
    pub fn make_activate_context(&self) -> WrapperActivateContext<'_, P> {
        WrapperActivateContext {
            inner: self,
            pending_requests: Default::default(),
        }
    }

    pub fn make_process_context<'a>(
        &'a self,
        transport: Transport,
        host_out_events: Option<ComRef<'a, IEventList>>,
        total_buffer_len: usize,
        current_sample_idx: usize,
    ) -> WrapperProcessContext<'a, P> {
        WrapperProcessContext {
            inner: self,
            input_events_guard: self.input_note_events.borrow_mut(),
            transport,
            host_out_events,
            total_buffer_len: total_buffer_len as u32,
            current_sample_idx: current_sample_idx as u32,
        }
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

    /// Either posts the task to the task queue using [`EventLoop::schedule_gui()`] so it can be
    /// delegated to the main thread, executes the task directly if this is the main thread, or runs
    /// the task on the host's `IRunLoop` if the GUI is open and it exposes one.
    ///
    /// If the task queue is full, then this will return false.
    #[must_use]
    pub fn schedule_gui(&self, task: Task<P>) -> bool {
        let event_loop = self.event_loop.borrow();
        let event_loop = event_loop.as_ref().unwrap();
        if event_loop.is_main_thread() {
            self.execute(task, true);
            true
        } else {
            // If the editor is open, and the host exposes the `IRunLoop` interface, then we'll run
            // the task on the host's GUI thread using that interface. Otherwise we'll use the
            // regular event loop. If the editor gets dropped while there's still outstanding work
            // left in the run loop task queue, then those tasks will be posted to the regular event
            // loop so no work is lost.
            if self.is_editor_open.load(Ordering::Relaxed) {
                match self
                    .plug_view
                    .read()
                    .clone()
                    .unwrap()
                    .do_maybe_in_run_loop(task)
                {
                    Ok(()) => true,
                    Err(task) => event_loop.schedule_gui(task),
                }
            } else {
                event_loop.schedule_gui(task)
            }
        }
    }

    pub fn has_param(&self, index: u32) -> bool {
        index < self.plugin_state.param_count()
    }

    pub fn queue_pending_automation(&self, index: u32, normalized_value: f32) -> bool {
        self.pending_automation
            .push((index, normalized_value))
            .is_ok()
    }

    /// Get the plugin's state object, may be called by the plugin's GUI as part of its own preset
    /// management. The wrapper doesn't use these functions and serializes and deserializes directly
    /// the JSON in the relevant plugin API methods instead.
    pub fn get_state_object(&self) -> SavedState {
        state::serialize_object::<P>(self.plugin_state.clone())
    }

    pub fn set_latency_samples(&self, samples: u32) {
        // Only trigger a restart if it's actually needed
        let old_latency = self.current_latency.swap(samples, Ordering::SeqCst);
        if old_latency != samples {
            self.request_restart();
        }
    }

    pub fn request_param_rescan(&self) {
        let task_posted = self.schedule_gui(Task::TriggerRestart(PARAM_RESCAN_FLAGS));
        crate::nice_debug_assert!(task_posted, "The task queue is full, dropping task...");
    }

    pub fn request_restart(&self) {
        let task_posted = self.schedule_gui(Task::TriggerRestart(RestartFlags_::kLatencyChanged));
        crate::nice_debug_assert!(task_posted, "The task queue is full, dropping task...");
    }

    /// Immediately set the plugin state. Returns `false` if the deserialization failed. The plugin
    /// state is set from a couple places, so this function aims to deduplicate that. Includes
    /// `permit_alloc()`s around the deserialization.
    ///
    /// Implicitly emits `Task::ParameterValuesChanged`.
    ///
    /// # Notes
    ///
    /// `self.plugin` must _not_ be locked while calling this function or it will deadlock.
    pub fn set_state_inner(&self, state: &mut SavedState) -> bool {
        // FIXME: This is obviously not realtime-safe, but loading presets without doing this
        //        could lead to inconsistencies. `state::deserialize_object()` normally never
        //        allocates, but if the plugin has persistent non-parameter data then its
        //        `deserialize_fields()` implementation may still allocate.
        let success =
            permit_alloc(|| state::deserialize_object::<P>(state, self.plugin_state.clone()));
        if !success {
            crate::nice_debug_assert_failure!(
                "Deserializing plugin state from a state object failed"
            );
            return false;
        }

        {
            // Reinitialize the plugin after loading state so it can respond to the new parameter values
            let task_posted = self.schedule_gui(Task::StateChanged);
            crate::nice_debug_assert!(task_posted, "The task queue is full, dropping task...");
            let task_posted = self.schedule_gui(Task::TriggerRestart(PARAM_RESCAN_FLAGS));
            crate::nice_debug_assert!(task_posted, "The task queue is full, dropping task...");
        }

        success
    }
}

impl<P: Vst3Plugin> MainThreadExecutor<Task<P>> for WrapperInner<P> {
    fn execute(&self, task: Task<P>, is_gui_thread: bool) {
        // This function is always called from the main thread
        match task {
            Task::PluginTask(task) => (self.task_executor.lock())(task),
            Task::StateChanged => {
                use nice_plug_core::editor::EditorHandle;

                if let Some(editor_window) = self.editor_window.borrow().as_ref() {
                    editor_window.get().handle.state_changed();
                }
            }
            Task::TriggerRestart(flags) => match &*self.component_handler.borrow() {
                Some(handler) => unsafe {
                    crate::nice_debug_assert!(is_gui_thread);
                    let result = handler.restartComponent(flags);
                    crate::nice_debug_assert_eq!(
                        result,
                        kResultOk,
                        "Failed the restart request call with flags '{:?}'",
                        flags
                    );
                },
                None => crate::nice_debug_assert_failure!("Component handler not yet set"),
            },
            Task::RequestResize { size, scale_factor } => {
                if self.is_editor_open.load(Ordering::SeqCst) {
                    unsafe {
                        crate::nice_debug_assert!(is_gui_thread);
                        let _success = WrapperView::request_resize(
                            &self.plug_view.read().clone().unwrap(),
                            size,
                            scale_factor,
                        );
                    }
                } else {
                    crate::nice_debug_assert_failure!("Can't resize a closed editor");
                }
            }
            Task::CallMainThread => {
                use nice_plug_core::editor::EditorHandle;

                if let Some(editor_window) = self.editor_window.borrow().as_ref() {
                    let editor_window = editor_window.get();
                    editor_window
                        .handle
                        .host_main_thread_callback(&editor_window.window)
                }
            }
        }
    }
}
