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

impl cosmic::app::CosmicFlags for Flags {
    type SubCommand = String;
    type Args = Vec<String>;
}

pub struct AppModel {
    core: cosmic::Core,
    show_preview: bool,
    settings_window: Option<window::Id>,
    settings_error: Option<String>,
    notification_task: Option<iced::task::Handle>,
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
    PreviewRequested(window::Id),
    CloseRequested(window::Id),
    Cancel,
    Opened(window::Id),
    Closed(window::Id),
}

impl AppModel {
    fn capture() -> Task<cosmic::Action<Message>> {
        cosmic::task::future(async {
            // Identify native launches to the notification portal before using any portal.
            // Older portals may not support registration; they infer the identity themselves.
            let app_id = <Self as cosmic::Application>::APP_ID
                .parse()
                .expect("valid app ID");
            let _ = ashpd::register_host_app(app_id).await;

            cosmic::Action::App(Message::Captured(capture::capture_desktop().await))
        })
    }

    fn open_preview(&mut self) -> Task<cosmic::Action<Message>> {
        if let Some(id) = self.preview {
            return window::gain_focus(id);
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

                let preview = if self.show_preview {
                    self.open_preview()
                } else {
                    Task::none()
                };

                crate::overlay::close(self.overlay)
                    .chain(preview)
                    .chain(self.copy())
            }

            Err(error) => {
                self.status = error;
                Task::none()
            }
        }
    }

    fn restart_capture(&mut self) -> Task<cosmic::Action<Message>> {
        self.notification_task = None;
        self.source = None;
        self.result = None;
        self.handle = None;
        self.dragging = false;
        self.busy = true;
        self.status.clear();
        self.status_detail = None;
        self.overlay = window::Id::unique();
        Task::future(async {
            if let Ok(portal) = ashpd::desktop::notification::NotificationProxy::new().await {
                let _ = portal.remove_notification("capture").await;
            }
        })
        .then(|()| Self::capture())
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

    const APP_ID: &'static str = "io.github.tg.PopShot";

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
                notification_task: None,
                settings_error,
                overlay: window::Id::unique(),
                preview: None,
                closing_preview: None,
                source: None,
                result: None,
                handle: None,
                selecting: false,
                dragging: false,
                busy: true,
                status: String::new(),
                status_detail: None,
            },
            Self::capture(),
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
        })
    }

    fn dbus_activation(
        &mut self,
        _: cosmic::dbus_activation::Message,
    ) -> Task<cosmic::Action<Message>> {
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

    fn update(&mut self, message: Message) -> Task<cosmic::Action<Message>> {
        match message {
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
            Message::PreviewRequested(id) if id == self.overlay && !self.selecting => {
                return self.open_preview();
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
                if self.preview.is_none() && !self.show_preview {
                    match result {
                        Ok(_) => {
                            if let Some(image) = self.result.clone() {
                                self.busy = true;
                                let (task, handle) =
                                    capture::notify(image, self.overlay).abortable();
                                self.notification_task = Some(handle.abort_on_drop());
                                return task.map(cosmic::Action::App);
                            }
                        }
                        Err(error) => {
                            self.status =
                                "Couldn’t copy the screenshot. Try again or save it.".into();
                            self.status_detail = Some(error);
                            return self.open_preview();
                        }
                    }
                    return Task::none();
                }
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

            Message::Cancel | Message::CloseRequested(_) => return iced::exit(),

            Message::Opened(id) if self.settings_window == Some(id) => {
                return self.set_window_title("Popshot — Settings".into(), id);
            }
            Message::Opened(id) => {
                return self.set_window_title("Popshot — Snipping Tool".into(), id);
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

        Task::none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmic::Application;

    #[test]
    fn settings_window_and_notification_preview_lifecycle() {
        let image = CapturedImage {
            width: 1,
            height: 1,
            rgba: vec![255; 4].into(),
            png: Vec::new().into(),
        };
        let mut app = AppModel {
            core: cosmic::Core::default(),
            show_preview: false,
            settings_window: None,
            settings_error: None,
            notification_task: None,
            overlay: window::Id::unique(),
            preview: None,
            closing_preview: None,
            source: Some(image),
            result: None,
            handle: None,
            selecting: true,
            dragging: false,
            busy: false,
            status: String::new(),
            status_detail: None,
        };
        let overlay = app.overlay;
        let _ = app.update(Message::Settings);
        let settings = app.settings_window.unwrap();
        assert_ne!(settings, overlay);
        assert!(app.preview.is_none());
        let _ = app.update(Message::Settings);
        assert_eq!(app.settings_window, Some(settings));
        let _ = app.update(Message::Mode(CaptureMode::Fullscreen));
        assert!(app.source.is_some() && app.result.is_none());
        let _ = app.update(Message::CloseRequested(settings));
        let _ = app.update(Message::Closed(settings));
        assert!(app.settings_window.is_none() && app.selecting);
        assert_ne!(app.overlay, overlay);
        assert!(app.source.is_some());

        let _ = app.update(Message::Mode(CaptureMode::Fullscreen));
        assert!(app.result.is_some() && app.preview.is_none());
        let _ = app.update(Message::Notified(Ok(())));
        assert!(!app.busy && app.result.is_some());
        let _ = app.update(Message::PreviewRequested(overlay));
        assert!(app.preview.is_none()); // Ignore a click for an earlier capture.
        let _ = app.update(Message::PreviewRequested(app.overlay));
        let preview = app.preview.unwrap();
        let _ = app.update(Message::PreviewRequested(app.overlay));
        assert_eq!(app.preview, Some(preview));
        let _ = app.update(Message::Settings);
        let settings = app.settings_window.unwrap();
        let _ = app.update(Message::Back);
        let _ = app.update(Message::Closed(settings));
        assert_eq!(app.preview, Some(preview));
        assert!(app.result.is_some() && app.settings_window.is_none());

        // A normal launch after a notification starts another capture.
        app.preview = None;
        let _ = app.restart_capture();
        assert!(app.busy && app.result.is_none() && app.notification_task.is_none());
    }
}
