pub mod additizer_ui;
pub mod slider_control;

use crate::AdditizerParams;
use additizer_ui::{AdditizerUI, Message as AdditizerUIMessage};
use crossbeam::{atomic::AtomicCell, channel};
use iced_baseview::{
    Application, Element, IcedBaseviewSettings, Settings, Task, Theme, WindowSubs,
    baseview::{self, WindowOpenOptions, WindowScalePolicy},
    futures::{
        Subscription,
        futures::{self},
    },
};
use nih_plug::{
    editor::{Editor, ParentWindowHandle},
    params::persist::PersistentField,
    prelude::GuiContext,
};
use raw_window_handle::{HasRawWindowHandle, RawWindowHandle};
use serde::{Deserialize, Serialize};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub struct CreateUIParams {
    pub params: Arc<AdditizerParams>,
    pub editor_state: Arc<IcedState>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IcedState {
    /// The window's size in logical pixels before applying `scale_factor`.
    #[serde(with = "nih_plug::params::persist::serialize_atomic_cell")]
    size: AtomicCell<(u32, u32)>,
    /// Whether the editor's window is currently open.
    #[serde(skip)]
    open: AtomicBool,
}

impl PersistentField<'_, IcedState> for Arc<IcedState> {
    fn set(&self, new_value: IcedState) {
        self.size.store(new_value.size.load());
    }

    fn map<F, R>(&self, f: F) -> R
    where
        F: Fn(&IcedState) -> R,
    {
        f(self)
    }
}

impl IcedState {
    pub fn from_size(width: u32, height: u32) -> Arc<IcedState> {
        Arc::new(IcedState {
            size: AtomicCell::new((width, height)),
            open: AtomicBool::new(false),
        })
    }

    pub fn size(&self) -> (u32, u32) {
        self.size.load()
    }

    pub fn is_open(&self) -> bool {
        self.open.load(Ordering::Acquire)
    }
}

struct ParameterUpdate;

#[derive(Debug, Clone, Copy)]
enum Message {
    UIMessage(AdditizerUIMessage),
}

struct AdditizerWrapper {
    additizer_ui: AdditizerUI,
    parameter_updates_receiver: Arc<channel::Receiver<ParameterUpdate>>,
}

impl Application for AdditizerWrapper {
    type Message = Message;
    type Flags = (
        Arc<dyn GuiContext>,
        Arc<channel::Receiver<ParameterUpdate>>,
        Arc<CreateUIParams>,
    );
    type Theme = Theme;
    type Executor = iced_baseview::executor::Default;

    fn new(flags: Self::Flags) -> (Self, Task<Message>) {
        let (additizer_ui, task) = AdditizerUI::new();

        (
            Self {
                additizer_ui,
                parameter_updates_receiver: flags.1,
            },
            task.map(Message::UIMessage),
        )
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::UIMessage(ui_message) => {
                self.additizer_ui.update(ui_message).map(Message::UIMessage)
            }
        }
    }

    fn subscription(
        &self,
        _window_subs: &mut WindowSubs<Self::Message>,
    ) -> Subscription<Self::Message> {
        let mut param_update_subscription = Subscription::none();

        if self.parameter_updates_receiver.try_recv().is_ok() {
            param_update_subscription = Subscription::run_with_id(
                "parameter updates",
                futures::stream::once(async {
                    Message::UIMessage(AdditizerUIMessage::ParameterUpdate)
                }),
            );
        }

        Subscription::batch([
            param_update_subscription,
            self.additizer_ui.subscription().map(Message::UIMessage),
        ])
    }

    fn view(&self) -> Element<Message> {
        self.additizer_ui.view().map(Message::UIMessage)
    }

    fn theme(&self) -> Theme {
        self.additizer_ui.theme()
    }
}

struct IcedEditorHandle<Message: 'static + Send> {
    window: iced_baseview::window::WindowHandle<Message>,
    iced_state: Arc<IcedState>,
}

unsafe impl<Message: Send> Send for IcedEditorHandle<Message> {}

impl<Message: Send> Drop for IcedEditorHandle<Message> {
    fn drop(&mut self) {
        self.iced_state.open.store(false, Ordering::Release);
        self.window.close_window();
    }
}

pub struct NihEditorWrapper {
    create_ui_params: Arc<CreateUIParams>,
    parameter_updates_sender: channel::Sender<ParameterUpdate>,
    parameter_updates_receiver: Arc<channel::Receiver<ParameterUpdate>>,
    scaling_factor: AtomicCell<Option<f32>>,
}

impl NihEditorWrapper {
    pub fn new(create_ui_params: CreateUIParams) -> Self {
        let (parameter_updates_sender, parameter_updates_receiver) = channel::bounded(1);

        Self {
            create_ui_params: Arc::new(create_ui_params),
            parameter_updates_sender,
            parameter_updates_receiver: Arc::new(parameter_updates_receiver),

            #[cfg(target_os = "macos")]
            scaling_factor: AtomicCell::new(None),
            #[cfg(not(target_os = "macos"))]
            scaling_factor: AtomicCell::new(Some(1.0)),
        }
    }
}

struct ParentWindowHandleAdapter(nih_plug::editor::ParentWindowHandle);

unsafe impl HasRawWindowHandle for ParentWindowHandleAdapter {
    fn raw_window_handle(&self) -> RawWindowHandle {
        match self.0 {
            ParentWindowHandle::X11Window(window) => {
                let mut handle = raw_window_handle::XcbWindowHandle::empty();
                handle.window = window;
                RawWindowHandle::Xcb(handle)
            }
            ParentWindowHandle::AppKitNsView(ns_view) => {
                let mut handle = raw_window_handle::AppKitWindowHandle::empty();
                handle.ns_view = ns_view;
                RawWindowHandle::AppKit(handle)
            }
            ParentWindowHandle::Win32Hwnd(hwnd) => {
                let mut handle = raw_window_handle::Win32WindowHandle::empty();
                handle.hwnd = hwnd;
                RawWindowHandle::Win32(handle)
            }
        }
    }
}

impl Editor for NihEditorWrapper {
    fn spawn(
        &self,
        parent: ParentWindowHandle,
        context: Arc<dyn GuiContext>,
    ) -> Box<dyn std::any::Any + Send> {
        let (unscaled_width, unscaled_height) = self.create_ui_params.editor_state.size();
        let scaling_factor = self.scaling_factor.load();

        let settings = Settings {
            window: WindowOpenOptions {
                title: String::from("Additizer"),
                size: baseview::Size::new(unscaled_width as f64, unscaled_height as f64),
                scale: scaling_factor
                    .map(|factor| WindowScalePolicy::ScaleFactor(factor as f64))
                    .unwrap_or(WindowScalePolicy::SystemScaleFactor),
            },
            iced_baseview: IcedBaseviewSettings {
                always_redraw: false,
                ignore_non_modifier_keys: false,
            },
            ..Default::default()
        };

        let window = iced_baseview::open_parented::<AdditizerWrapper, ParentWindowHandleAdapter>(
            &ParentWindowHandleAdapter(parent),
            (
                context,
                self.parameter_updates_receiver.clone(),
                self.create_ui_params.clone(),
            ),
            settings,
        );

        Box::new(IcedEditorHandle {
            window,
            iced_state: self.create_ui_params.editor_state.clone(),
        })
    }

    fn size(&self) -> (u32, u32) {
        self.create_ui_params.editor_state.size()
    }

    fn set_scale_factor(&self, factor: f32) -> bool {
        if self.create_ui_params.editor_state.is_open() {
            return false;
        }

        self.scaling_factor.store(Some(factor));
        true
    }

    fn param_value_changed(&self, _id: &str, _normalized_value: f32) {
        let _ = self.parameter_updates_sender.try_send(ParameterUpdate);
    }

    fn param_modulation_changed(&self, _id: &str, _modulation_offset: f32) {
        let _ = self.parameter_updates_sender.try_send(ParameterUpdate);
    }

    fn param_values_changed(&self) {
        let _ = self.parameter_updates_sender.try_send(ParameterUpdate);
    }
}
