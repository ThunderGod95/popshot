use crate::{
    capture::{self, CapturedImage},
    output_selection::CaptureMode,
};
use cosmic::{
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
    overlay: window::Id,
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
    Cancel,
    Opened(window::Id),
}

impl AppModel {
    fn open_preview(&mut self) -> Task<cosmic::Action<Message>> {
        let (_, task) = window::open(window::Settings {
            size: iced::Size::new(1040.0, 720.0),
            min_size: Some(iced::Size::new(560.0, 360.0)),
            exit_on_close_request: false,
            ..Default::default()
        });

        task.map(|id| cosmic::Action::App(Message::Opened(id)))
    }

    fn finish(&mut self, region: Option<[f32; 4]>) -> Task<cosmic::Action<Message>> {
        if !self.selecting {
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
                crate::overlay::close(self.overlay)
                    .chain(self.open_preview())
                    .chain(self.copy())
            }

            Err(error) => {
                self.status = error;
                Task::none()
            }
        }
    }

    fn copy(&mut self) -> Task<cosmic::Action<Message>> {
        let Some(image) = &self.result else {
            return Task::none();
        };

        if self.busy {
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
        (
            Self {
                core,
                overlay: window::Id::unique(),
                source: None,
                result: None,
                handle: None,
                selecting: false,
                dragging: false,
                busy: false,
                status: String::new(),
                status_detail: None,
            },
            cosmic::task::future(async {
                cosmic::Action::App(Message::Captured(capture::capture_desktop().await))
            }),
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
        iced::event::listen_with(|event, status, _| match event {
            Event::Window(window::Event::CloseRequested) => Some(Message::Cancel),
            Event::Keyboard(keyboard::Event::KeyPressed {
                key: Key::Named(Named::Escape),
                ..
            }) => Some(Message::Cancel),
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

    fn update(&mut self, message: Message) -> Task<cosmic::Action<Message>> {
        match message {
            Message::Captured(Ok(image)) => {
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
                self.status = "Couldn’t take a screenshot. Please try again.".into();
                self.status_detail = Some(error);
                return self.open_preview();
            }

            Message::Mode(CaptureMode::Fullscreen) if !self.dragging => return self.finish(None),

            Message::Select(region) => return self.finish(Some(region)),

            Message::Drag(dragging) => self.dragging = dragging,

            Message::Copy => return self.copy(),

            Message::Save if !self.busy => {
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

            Message::Cancel => return iced::exit(),

            Message::Opened(id) => {
                return self.set_window_title("Popshot — Snipping Tool".into(), id);
            }

            _ => {}
        }

        Task::none()
    }
}
