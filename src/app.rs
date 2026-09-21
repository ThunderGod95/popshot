use crate::{
    capture::{self, CapturedImage},
    output_selection::{CaptureMode, Selection},
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
    copy_on_capture: bool,
    auto_save: bool,
    save_location: String,
    show_notification: bool,
    return_to_editor: bool,
    capture_mode: Option<CaptureMode>,
    settings_open: bool,
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
    Select(Selection),
    Drag(bool),
    Copy,
    NewSnip,
    Save,
    Done(Result<String, String>),
    Settings,
    Back,
    ShowPreview(bool),
    CopyOnCapture(bool),
    AutoSave(bool),
    ShowNotification(bool),
    ChooseSaveLocation,
    SaveLocation(Result<Option<String>, String>),
    Notified(Result<(), String>),
    Activated(crate::activation::Request),
    PreviewLoaded(Result<CapturedImage, String>),
    Prepared(Result<(Option<String>, String), String>),
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

        let (id, action) = cosmic::surface::action::app_window::<Self>(
            |_| Default::default(),
            |_| window::Settings {
                size: iced::Size::new(1040.0, 720.0),
                min_size: Some(iced::Size::new(560.0, 360.0)),
                transparent: true,
                exit_on_close_request: false,
                platform_specific: window::settings::PlatformSpecific {
                    application_id: crate::activation::APP_ID.into(),
                    ..Default::default()
                },
                ..Default::default()
            },
            None,
        );

        self.preview = Some(id);

        cosmic::surface::surface_task(action)
    }

    fn focus_preview(&mut self, id: window::Id) -> Task<cosmic::Action<Message>> {
        if let Some(token) = self.activation_token.take() {
            return iced::platform_specific::shell::commands::activation::activate(id, token);
        }
        window::gain_focus(id)
    }

    fn finish(&mut self, region: Option<Selection>) -> Task<cosmic::Action<Message>> {
        if !self.selecting {
            return Task::none();
        }

        let Some(source) = self.source.as_ref() else {
            return Task::none();
        };

        let result = match region {
            Some(region) => capture::crop(source, &region),
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
                self.status = "Finishing capture…".into();
                self.busy = true;

                let png = self.result.as_ref().unwrap().png.clone();

                let close = if self.capture_mode == Some(CaptureMode::Fullscreen) {
                    Task::none()
                } else {
                    crate::overlay::close(self.overlay)
                };

                let copy = self.copy_on_capture;
                let folder = self.auto_save.then(|| self.save_location.clone());
                let notify = self.show_notification;

                close.chain(cosmic::task::future(async move {
                    let mut errors = Vec::new();
                    let mut status = Vec::new();

                    if copy {
                        match crate::clipboard::copy_png(&png).await {
                            Ok(()) => status.push("Copied to clipboard".to_string()),
                            Err(error) => errors.push(error),
                        }
                    }

                    if let Some(folder) = folder {
                        match capture::auto_save(&png, std::path::Path::new(&folder)).await {
                            Ok(path) => status.push(format!("Saved to {}", path.display())),
                            Err(error) => errors.push(error),
                        }
                    }

                    let mut id = None;

                    if notify {
                        match tokio::task::spawn_blocking(move || crate::cache::store(&png))
                            .await
                            .map_err(|e| e.to_string())
                            .and_then(|result| result)
                        {
                            Ok(capture_id) => id = Some(capture_id),
                            Err(error) => errors.push(error),
                        }
                    }

                    let result = if errors.is_empty() {
                        Ok((
                            id,
                            if status.is_empty() {
                                "Screenshot captured".into()
                            } else {
                                status.join(". ")
                            },
                        ))
                    } else {
                        Err(status
                            .into_iter()
                            .chain(errors)
                            .collect::<Vec<_>>()
                            .join(". "))
                    };

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

    fn activate(
        &mut self,
        return_to_editor: bool,
        mode: Option<CaptureMode>,
    ) -> Task<cosmic::Action<Message>> {
        if self.selecting || self.busy || self.settings_open || self.closing_preview.is_some() {
            return Task::none();
        }

        self.return_to_editor = return_to_editor;
        self.capture_mode = mode;

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

        if self.busy || self.settings_open || self.closing_preview.is_some() {
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
            .unwrap_or(false);
        let copy_on_capture = config
            .as_ref()
            .ok()
            .and_then(|c| c.get("copy_on_capture").ok())
            .unwrap_or(true);
        let auto_save = config
            .as_ref()
            .ok()
            .and_then(|c| c.get("auto_save").ok())
            .unwrap_or(false);
        let show_notification = config
            .as_ref()
            .ok()
            .and_then(|c| c.get("show_notification").ok())
            .unwrap_or(true);
        let save_location = config
            .as_ref()
            .ok()
            .and_then(|c| c.get("save_location").ok())
            .unwrap_or_else(capture::default_save_location);
        let settings_error = config
            .err()
            .map(|error| format!("Couldn’t load settings: {error}"));

        (
            Self {
                core,
                show_preview,
                copy_on_capture,
                auto_save,
                save_location,
                show_notification,
                return_to_editor: false,
                capture_mode: None,
                settings_open: false,
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
        if id == self.overlay
            && self.selecting
            && let Some(handle) = &self.handle
        {
            return crate::ui::selection(
                handle,
                self.dragging,
                &self.status,
                self.capture_mode.unwrap_or(CaptureMode::Rectangle),
            );
        }

        let preview = crate::ui::preview(
            self.handle.as_ref(),
            self.result
                .as_ref()
                .map(|image| (image.width, image.height)),
            self.busy,
            &self.status,
            self.status_detail.as_deref(),
        );

        if self.settings_open {
            crate::ui::settings(
                preview,
                self.show_preview,
                self.copy_on_capture,
                self.auto_save,
                &self.save_location,
                self.show_notification,
                self.settings_error.as_deref(),
            )
        } else {
            preview
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            crate::activation::subscription(),
            iced::event::listen_with(|event, status, id| match event {
                Event::Window(window::Event::Opened { .. }) => Some(Message::Opened(id)),

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
                    "l" if !modifiers.control() => Some(Message::Mode(CaptureMode::Freehand)),
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
                        || self.settings_open
                        || self.closing_preview.is_some()
                    {
                        self.pending_preview = Some(request);
                        return Task::none();
                    }

                    return self.load_preview(request);
                }

                self.activation_token = request.token;

                return self.activate(false, request.mode);
            }

            Message::NewSnip if self.preview.is_some() => return self.activate(true, None),

            Message::Prepared(Ok((id, status))) => {
                self.busy = false;
                self.status = status;
                self.status_detail = None;

                // Keep an otherwise discarded screenshot available for manual saving.
                let editor = if self.pending_preview.is_none()
                    && (self.show_preview
                        || self.return_to_editor
                        || (!self.copy_on_capture && !self.auto_save && !self.show_notification))
                {
                    self.open_preview()
                } else {
                    Task::none()
                };

                if let Some(id) = id {
                    self.busy = true;

                    let image = self.result.clone().expect("capture prepared");
                    let status = self.status.clone();

                    return Task::batch([
                        editor,
                        cosmic::task::future(async move {
                            cosmic::Action::App(Message::Notified(
                                capture::notify(&image, &id, &status).await,
                            ))
                        }),
                    ]);
                }

                if let Some(request) = self.pending_preview.take() {
                    return self.load_preview(request);
                }

                return if self.preview.is_some() {
                    editor
                } else {
                    iced::exit()
                };
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

            Message::Settings if self.preview.is_some() => {
                self.settings_open = true;
            }

            Message::Back => {
                self.settings_open = false;
            }

            Message::CloseRequested(id) if self.settings_open && self.preview == Some(id) => {
                return self.update(Message::Back);
            }

            Message::ShowPreview(value)
            | Message::CopyOnCapture(value)
            | Message::AutoSave(value)
            | Message::ShowNotification(value) => {
                let (key, setting) = match message {
                    Message::ShowPreview(_) => ("show_preview", &mut self.show_preview),
                    Message::CopyOnCapture(_) => ("copy_on_capture", &mut self.copy_on_capture),
                    Message::AutoSave(_) => ("auto_save", &mut self.auto_save),
                    _ => ("show_notification", &mut self.show_notification),
                };
                match Config::new(Self::APP_ID, 1).and_then(|config| config.set(key, value)) {
                    Ok(()) => {
                        *setting = value;
                        self.settings_error = None;
                    }
                    Err(error) => {
                        self.settings_error = Some(format!("Couldn’t save settings: {error}"))
                    }
                }
            }

            Message::ChooseSaveLocation if !self.busy => {
                self.busy = true;
                return cosmic::task::future(async {
                    cosmic::Action::App(Message::SaveLocation(
                        capture::choose_save_location().await,
                    ))
                });
            }

            Message::SaveLocation(result) => {
                self.busy = false;
                match result {
                    Ok(Some(path)) => match Config::new(Self::APP_ID, 1)
                        .and_then(|config| config.set("save_location", path.clone()))
                    {
                        Ok(()) => {
                            self.save_location = path;
                            self.settings_error = None;
                        }
                        Err(error) => {
                            self.settings_error = Some(format!("Couldn’t save settings: {error}"))
                        }
                    },
                    Ok(None) => {}
                    Err(error) => self.settings_error = Some(error),
                }
            }

            Message::Notified(Ok(())) => {
                self.busy = false;
                self.status_detail = None;

                if let Some(request) = self.pending_preview.take() {
                    return self.load_preview(request);
                }

                return if self.preview.is_some() {
                    Task::none()
                } else {
                    iced::exit()
                };
            }

            Message::Notified(Err(error)) => {
                self.busy = false;
                self.status.push_str(". Couldn’t show a notification.");
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

                if self.capture_mode == Some(CaptureMode::Fullscreen) {
                    return self.finish(None);
                }

                return crate::overlay::open(self.overlay);
            }

            Message::Captured(Err(error)) => {
                self.busy = false;
                self.status = "Couldn’t take a screenshot. Please try again.".into();
                self.status_detail = Some(error);

                return self.open_preview();
            }

            Message::Mode(CaptureMode::Fullscreen) if !self.dragging => return self.finish(None),

            Message::Mode(mode) if self.selecting && !self.dragging => {
                self.capture_mode = Some(mode);
                self.status.clear();
            }

            Message::Select(region) => return self.finish(Some(region)),

            Message::Drag(dragging) => self.dragging = dragging,

            Message::Copy => return self.copy(),

            Message::Save
                if !self.busy && !self.settings_open && self.closing_preview.is_none() =>
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

            Message::Opened(id) if self.preview == Some(id) => {
                return self
                    .set_window_title("PopShot".into(), id)
                    .chain(self.focus_preview(id));
            }

            Message::Closed(id) if self.closing_preview == Some(id) => {
                self.closing_preview = None;

                // Request the next screenshot only after the old preview is destroyed.
                return self.restart_capture();
            }

            _ => {}
        }

        if !self.busy
            && !self.selecting
            && !self.settings_open
            && self.closing_preview.is_none()
            && let Some(request) = self.pending_preview.take()
        {
            return self.load_preview(request);
        }

        Task::none()
    }
}
