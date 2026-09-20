use crate::{
    capture::{self, CapturedImage},
    output_selection::CaptureMode,
};
use cosmic::{
    cosmic_config::{Config, ConfigGet, ConfigSet},
    iced::{
        self, Event, Subscription,
        keyboard::{self, Key, key::Named},
        window,
    },
    prelude::*,
    widget::{self, image::Handle},
};

pub struct Flags;

pub struct AppModel {
    core: cosmic::Core,
    show_preview: bool,
    settings_window: Option<window::Id>,
    settings_error: Option<String>,
    pending_preview: Option<crate::activation::Request>,
    activation_token: Option<String>,
    overlay: window::Id,
    preview: Option<window::Id>,
    closing_preview: Option<window::Id>,
    source: Option<CapturedImage>,
    result: Option<CapturedImage>,
    handle: Option<Handle>,
    selecting: bool,
    dragging: bool,
    busy: bool,
    status: String,
    status_detail: Option<String>,
}

#[derive(Debug, Clone)]
pub enum Message {
    Captured(Result<CapturedImage, String>),
    Mode(CaptureMode),
    Select([f32; 4]),
    Drag(bool),
    Copy,
    Save,
    Done(Result<String, String>),
    Settings,
    Back,
    ShowPreview(bool),
    Notified(Result<(), String>),
    Activated(crate::activation::Request),
    PreviewLoaded(Result<CapturedImage, String>),
    Prepared(Result<String, String>),
    CloseRequested(window::Id),
    Cancel,
    Opened(window::Id),
    Closed(window::Id),
}

impl AppModel {
    fn capture() -> Task<cosmic::Action<Message>> {
        cosmic::task::future(async {
            cosmic::Action::App(Message::Captured(capture::capture_desktop().await))
        })
    }

    fn open_preview(&mut self) -> Task<cosmic::Action<Message>> {
        if let Some(id) = self.preview {
            return self.focus_preview(id);
        }

        let (id, task) = window::open(window::Settings {
            size: iced::Size::new(1040.0, 720.0),
            min_size: Some(iced::Size::new(560.0, 360.0)),
            exit_on_close_request: false,
            ..Default::default()
        });

        self.preview = Some(id);

        task.map(|id| cosmic::Action::App(Message::Opened(id)))
    }

    fn focus_preview(&mut self, id: window::Id) -> Task<cosmic::Action<Message>> {
        if let Some(token) = self.activation_token.take() {
            return iced::platform_specific::shell::commands::activation::activate(id, token);
        }
        window::gain_focus(id)
    }

    fn finish(&mut self, region: Option<[f32; 4]>) -> Task<cosmic::Action<Message>> {
        if !self.selecting || self.settings_window.is_some() {
            return Task::none();
        }

        let Some(source) = self.source.as_ref() else {
            return Task::none();
        };

        let result = match region {
            Some(region) => capture::crop(source, region),
            None => Ok(source.clone()),
        };

        match result {
            Ok(image) => {
                self.handle = Some(Handle::from_rgba(
                    image.width,
                    image.height,
                    image.rgba.to_vec(),
                ));
                self.result = Some(image);
                self.source = None;
                self.selecting = false;
                self.status = "Copying screenshot…".into();

                self.busy = true;

                let png = self.result.as_ref().unwrap().png.clone();

                crate::overlay::close(self.overlay).chain(cosmic::task::future(async move {
                    let result = async {
                        crate::clipboard::copy_png(&png).await?;
                        tokio::task::spawn_blocking(move || crate::cache::store(&png))
                            .await
                            .map_err(|e| e.to_string())?
                    }
                    .await;

                    cosmic::Action::App(Message::Prepared(result))
                }))
            }

            Err(error) => {
                self.status = error;
                Task::none()
            }
        }
    }

    fn restart_capture(&mut self) -> Task<cosmic::Action<Message>> {
        self.source = None;
        self.result = None;
        self.handle = None;
        self.dragging = false;
        self.busy = true;
        self.status.clear();
        self.status_detail = None;
        self.overlay = window::Id::unique();
        Self::capture()
    }

    fn load_preview(
        &mut self,
        request: crate::activation::Request,
    ) -> Task<cosmic::Action<Message>> {
        self.activation_token = request.token;

        let id = request.capture_id.expect("preview request");

        self.busy = true;
        self.source = None;
        self.result = None;
        self.handle = None;

        cosmic::task::future(async move {
            let result = tokio::task::spawn_blocking(move || crate::cache::load(&id))
                .await
                .map_err(|e| e.to_string())
                .and_then(|result| result);

            cosmic::Action::App(Message::PreviewLoaded(result))
        })
    }

    fn activate(&mut self) -> Task<cosmic::Action<Message>> {
        if self.selecting
            || self.busy
            || self.settings_window.is_some()
            || self.closing_preview.is_some()
        {
            return Task::none();
        }

        // Taking the ID also ignores repeat invocations while closing/capturing.
        let Some(id) = self.preview.take() else {
            return self.restart_capture();
        };
        self.closing_preview = Some(id);

        window::close(id)
    }

    fn copy(&mut self) -> Task<cosmic::Action<Message>> {
        let Some(image) = &self.result else {
            return Task::none();
        };

        if self.busy || self.settings_window.is_some() || self.closing_preview.is_some() {
            return Task::none();
        }

        self.busy = true;
        self.status = "Copying image…".into();
        self.status_detail = None;

        let png = image.png.clone();

        cosmic::task::future(async move {
            cosmic::Action::App(Message::Done(
                crate::clipboard::copy_png(&png)
                    .await
                    .map(|()| "Copied to clipboard".into()),
            ))
        })
    }
}

impl cosmic::Application for AppModel {
    type Executor = cosmic::executor::Default;
    type Flags = Flags;
    type Message = Message;

    const APP_ID: &'static str = crate::activation::APP_ID;

    fn core(&self) -> &cosmic::Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut cosmic::Core {
        &mut self.core
    }

    fn init(core: cosmic::Core, _: Flags) -> (Self, Task<cosmic::Action<Message>>) {
        let config = Config::new(Self::APP_ID, 1);
        let show_preview = config
            .as_ref()
            .ok()
            .and_then(|config| config.get("show_preview").ok())
            .unwrap_or(true);
        let settings_error = config
            .err()
            .map(|error| format!("Couldn’t load settings: {error}"));

        (
            Self {
                core,
                show_preview,
                settings_window: None,
                pending_preview: None,
                activation_token: None,
                settings_error,
                overlay: window::Id::unique(),
                preview: None,
                closing_preview: None,
                source: None,
                result: None,
                handle: None,
                selecting: false,
                dragging: false,
                busy: false,
                status: String::new(),
                status_detail: None,
            },
            Task::none(),
        )
    }

    fn view(&self) -> Element<'_, Message> {
        widget::space().into()
    }

    fn view_window(&self, id: window::Id) -> Element<'_, Message> {
        if self.settings_window == Some(id) {
            return crate::ui::settings(self.show_preview, self.settings_error.as_deref());
        }

        if id == self.overlay
            && self.selecting
            && let Some(handle) = &self.handle
        {
            return crate::ui::selection(handle, self.dragging, &self.status);
        }

        crate::ui::preview(
            self.handle.as_ref(),
            self.result
                .as_ref()
                .map(|image| (image.width, image.height)),
            self.busy,
            &self.status,
            self.status_detail.as_deref(),
        )
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            crate::activation::subscription(),
            iced::event::listen_with(|event, status, id| match event {
                Event::Window(window::Event::Closed) => Some(Message::Closed(id)),

                Event::Window(window::Event::CloseRequested) => Some(Message::CloseRequested(id)),

                Event::Keyboard(keyboard::Event::KeyPressed {
                    key: Key::Named(Named::Escape),
                    ..
                }) => Some(Message::CloseRequested(id)),

                Event::Keyboard(keyboard::Event::KeyPressed {
                    key: Key::Character(key),
                    modifiers,
                    ..
                }) if status == iced::event::Status::Ignored => match key.to_lowercase().as_str() {
                    "c" if modifiers.control() => Some(Message::Copy),
                    "s" if modifiers.control() => Some(Message::Save),
                    "r" if !modifiers.control() => Some(Message::Mode(CaptureMode::Rectangle)),
                    "f" if !modifiers.control() => Some(Message::Mode(CaptureMode::Fullscreen)),
                    _ => None,
                },

                _ => None,
            }),
        ])
    }

    fn update(&mut self, message: Message) -> Task<cosmic::Action<Message>> {
        match message {
            Message::Activated(request) => {
                if request.capture_id.is_some() {
                    if self.busy
                        || self.selecting
                        || self.settings_window.is_some()
                        || self.closing_preview.is_some()
                    {
                        self.pending_preview = Some(request);
                        return Task::none();
                    }

                    return self.load_preview(request);
                }

                self.activation_token = request.token;

                return self.activate();
            }

            Message::Prepared(Ok(id)) => {
                self.busy = false;
                self.status = "Copied to clipboard".into();

                if self.show_preview && self.pending_preview.is_none() {
                    return self.open_preview();
                }

                self.busy = true;

                let image = self.result.clone().expect("capture prepared");

                return cosmic::task::future(async move {
                    cosmic::Action::App(Message::Notified(capture::notify(&image, &id).await))
                });
            }

            Message::Prepared(Err(error)) => {
                self.busy = false;
                self.status = "Couldn’t finish capturing. Copy or save your screenshot.".into();
                self.status_detail = Some(error);

                return self.open_preview();
            }

            Message::PreviewLoaded(result) => {
                self.busy = false;

                if let Some(request) = self.pending_preview.take() {
                    return self.load_preview(request);
                }

                match result {
                    Ok(image) => {
                        self.handle = Some(Handle::from_rgba(
                            image.width,
                            image.height,
                            image.rgba.to_vec(),
                        ));
                        self.result = Some(image);
                        self.status = "Screenshot ready".into();
                        self.status_detail = None;
                    }

                    Err(error) => {
                        self.status = "Cached screenshot is no longer available.".into();
                        self.status_detail = Some(error);
                    }
                }

                return self.open_preview();
            }

            Message::Settings => {
                if let Some(id) = self.settings_window {
                    return window::gain_focus(id);
                }

                let (id, task) = window::open(window::Settings {
                    size: iced::Size::new(640.0, 360.0),
                    min_size: Some(iced::Size::new(420.0, 280.0)),
                    exit_on_close_request: false,
                    ..Default::default()
                });

                self.settings_window = Some(id);

                let open = task.map(|id| cosmic::Action::App(Message::Opened(id)));

                return if self.selecting {
                    crate::overlay::close(self.overlay).chain(open)
                } else {
                    open
                };
            }
            Message::Back => {
                if let Some(id) = self.settings_window {
                    return window::close(id);
                }
            }
            Message::CloseRequested(id) if self.settings_window == Some(id) => {
                return window::close(id);
            }

            Message::ShowPreview(value) => {
                match Config::new(Self::APP_ID, 1)
                    .and_then(|config| config.set("show_preview", value))
                {
                    Ok(()) => {
                        self.show_preview = value;
                        self.settings_error = None;
                    }
                    Err(error) => {
                        self.settings_error = Some(format!("Couldn’t save settings: {error}"))
                    }
                }
            }

            Message::Notified(Ok(())) => {
                self.busy = false;
                self.status = "Copied to clipboard".into();
                self.status_detail = None;
                if let Some(request) = self.pending_preview.take() {
                    return self.load_preview(request);
                }
                return iced::exit();
            }

            Message::Notified(Err(error)) => {
                self.busy = false;
                self.status = "Copied to clipboard, but couldn’t show a notification.".into();
                self.status_detail = Some(error);

                return self.open_preview();
            }

            Message::Captured(Ok(image)) => {
                self.busy = false;
                self.handle = Some(Handle::from_rgba(
                    image.width,
                    image.height,
                    image.rgba.to_vec(),
                ));
                self.source = Some(image);
                self.selecting = true;

                return crate::overlay::open(self.overlay);
            }

            Message::Captured(Err(error)) => {
                self.busy = false;
                self.status = "Couldn’t take a screenshot. Please try again.".into();
                self.status_detail = Some(error);
                return self.open_preview();
            }

            Message::Mode(CaptureMode::Fullscreen) if !self.dragging => return self.finish(None),

            Message::Select(region) => return self.finish(Some(region)),

            Message::Drag(dragging) => self.dragging = dragging,

            Message::Copy => return self.copy(),

            Message::Save
                if !self.busy
                    && self.settings_window.is_none()
                    && self.closing_preview.is_none() =>
            {
                if let Some(image) = &self.result {
                    self.busy = true;
                    self.status = "Saving image…".into();
                    self.status_detail = None;

                    let png = image.png.clone();

                    return cosmic::task::future(async move {
                        cosmic::Action::App(Message::Done(capture::save(&png).await))
                    });
                }
            }

            Message::Done(result) => {
                self.busy = false;
                match result {
                    Ok(status) => {
                        self.status = if status.starts_with("Saved to ") {
                            "Screenshot saved".into()
                        } else {
                            status
                        };
                        self.status_detail = None;
                    }
                    Err(error) => {
                        self.status = "Couldn’t finish. Try again; your screenshot is safe.".into();
                        self.status_detail = Some(error);
                    }
                }
            }

            // Finish writes before exiting, including a save already in progress.
            Message::Cancel | Message::CloseRequested(_) if self.busy => return Task::none(),
            Message::Cancel | Message::CloseRequested(_) => {
                if let Some(request) = self.pending_preview.take() {
                    self.selecting = false;
                    return crate::overlay::close(self.overlay).chain(self.load_preview(request));
                }
                return iced::exit();
            }

            Message::Opened(id) if self.settings_window == Some(id) => {
                return self.set_window_title("Popshot — Settings".into(), id);
            }

            Message::Opened(id) => {
                return self
                    .set_window_title("Popshot — Snipping Tool".into(), id)
                    .chain(self.focus_preview(id));
            }

            Message::Closed(id) if self.closing_preview == Some(id) => {
                // Request the next screenshot only after the old preview is destroyed.
                self.closing_preview = None;
                return self.restart_capture();
            }

            Message::Closed(id) if self.settings_window == Some(id) => {
                self.settings_window = None;
                if self.selecting {
                    self.overlay = window::Id::unique();
                    return crate::overlay::open(self.overlay);
                }
            }

            _ => {}
        }

        if !self.busy
            && !self.selecting
            && self.settings_window.is_none()
            && let Some(request) = self.pending_preview.take()
        {
            return self.load_preview(request);
        }

        Task::none()
    }
}
