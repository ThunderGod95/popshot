use crate::{
    capture::{self, CapturedImage},
    output_selection::{CaptureMode, OutputSelection},
};
use cosmic::{
    iced::{
        self, ContentFit, Event, Length, Subscription,
        keyboard::{self, Key, key::Named},
        widget::Stack,
        window,
    },
    prelude::*,
    widget::{self, image::Handle},
};

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
            size: iced::Size::new(960.0, 640.0),
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
        
        let png = image.png.clone();
        
        cosmic::task::future(async move {
            cosmic::Action::App(Message::Done(
                crate::clipboard::copy_png(&png)
                    .await
                    .map(|()| "Screenshot copied to clipboard".into()),
            ))
        })
    }
}

impl cosmic::Application for AppModel {
    type Executor = cosmic::executor::Default;
    type Flags = ();
    type Message = Message;
    
    const APP_ID: &'static str = "io.github.tg.PopShot";
    
    fn core(&self) -> &cosmic::Core {
        &self.core
    }
    
    fn core_mut(&mut self) -> &mut cosmic::Core {
        &mut self.core
    }
    
    fn init(core: cosmic::Core, _: ()) -> (Self, Task<cosmic::Action<Message>>) {
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
        if id == self.overlay && self.selecting {
            let screenshot = widget::image(self.handle.clone().unwrap())
                .width(Length::Fill)
                .height(Length::Fill)
                .content_fit(ContentFit::Fill);
            
            let selector = Element::new(OutputSelection {
                on_select: Message::Select,
                on_drag: Message::Drag,
            });

            let mut layers = vec![screenshot.into(), selector];

            if !self.dragging {
                let mut modes = widget::row([]).spacing(6);
                for mode in CaptureMode::ALL {
                    let button = if mode == CaptureMode::Rectangle {
                        widget::button::suggested(mode.label())
                    } else {
                        widget::button::standard(mode.label())
                    };
                    modes = modes.push(
                        button.on_press_maybe(mode.available().then_some(Message::Mode(mode))),
                    );
                }
                modes =
                    modes.push(widget::button::standard("Close · Esc").on_press(Message::Cancel));
                let toolbar = widget::container(widget::column([]).spacing(8).push(modes).push(
                    widget::text(if self.status.is_empty() {
                        "Drag to snip a rectangle · R Rectangle · F Fullscreen"
                    } else {
                        &self.status
                    }),
                ))
                .padding(12)
                .class(cosmic::theme::Container::Card);
                layers.push(
                    widget::container(toolbar)
                        .width(Length::Fill)
                        .align_x(iced::alignment::Horizontal::Center)
                        .padding(16)
                        .into(),
                );
            }
            return Stack::with_children(layers)
                .width(Length::Fill)
                .height(Length::Fill)
                .into();
        }
        let buttons = widget::row([])
            .spacing(8)
            .push(widget::text::title3("Popshot"))
            .push(widget::space().width(Length::Fill))
            .push(
                widget::button::standard("Copy · Ctrl+C")
                    .on_press_maybe((self.result.is_some() && !self.busy).then_some(Message::Copy)),
            )
            .push(
                widget::button::suggested("Save as… · Ctrl+S")
                    .on_press_maybe((self.result.is_some() && !self.busy).then_some(Message::Save)),
            )
            .push(widget::button::standard("Close").on_press(Message::Cancel));
        let mut content = widget::column([]).spacing(16).push(buttons);
        if let Some(image) = &self.result {
            content = content.push(widget::text(format!(
                "{} × {} pixels",
                image.width, image.height
            )));
        }
        if let Some(handle) = &self.handle {
            content = content.push(
                widget::image(handle.clone())
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .content_fit(ContentFit::Contain),
            );
        }
        widget::container(content.push(widget::text(&self.status)))
            .padding(20)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
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
                self.status = format!("Capture failed: {error}");
                return self.open_preview();
            }
            Message::Mode(CaptureMode::Fullscreen) if !self.dragging => return self.finish(None),
            Message::Select(region) => return self.finish(Some(region)),
            Message::Drag(dragging) => self.dragging = dragging,
            Message::Copy => return self.copy(),
            Message::Save if !self.busy => {
                if let Some(image) = &self.result {
                    self.busy = true;
                    let png = image.png.clone();
                    return cosmic::task::future(async move {
                        cosmic::Action::App(Message::Done(capture::save(&png).await))
                    });
                }
            }
            Message::Done(result) => {
                self.busy = false;
                self.status = result
                    .unwrap_or_else(|error| format!("{error}. You can retry or save the image."));
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
