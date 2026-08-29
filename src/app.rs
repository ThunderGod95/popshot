use std::{collections::HashMap, time::Duration};

use crate::{
    capture::{self, CapturedImage},
    display::DisplayState,
    geometry::{LogicalPoint, LogicalRect, Selection},
    selection::SelectionEvent,
};

use cosmic::{
    iced::{
        ContentFit, Event, Length, Subscription,
        core::event::wayland::OutputEvent,
        event,
        keyboard::{Event as KeyEvent, Key, key::Named},
        widget::Stack,
        window,
    },
    prelude::*,
    widget::{self, image::Handle},
};

use wayland_client::protocol::wl_output::WlOutput;

pub struct AppModel {
    core: cosmic::Core,

    state: AppState,

    screenshot: Option<CapturedImage>,

    overlay_images: HashMap<window::Id, Handle>,

    display: DisplayState,

    selection_start: Option<LogicalPoint>,
    selection_rect: Option<LogicalRect>,

    geometry_generation: u64,
}

#[derive(Debug, Clone)]
enum AppState {
    Capturing,
    Selecting,
    Processing,
}

#[derive(Debug, Clone)]
pub enum Message {
    CaptureFinished(Result<CapturedImage, String>),

    OutputChanged(OutputEvent, WlOutput),

    GeometrySettled(u64),

    Selection(SelectionEvent),

    SelectionProcessed(Result<(), String>),

    Cancel,
}

impl AppModel {
    fn schedule_geometry_check(&mut self) -> Task<cosmic::Action<Message>> {
        self.geometry_generation = self.geometry_generation.wrapping_add(1);

        let generation = self.geometry_generation;

        cosmic::task::future(async move {
            tokio::time::sleep(Duration::from_millis(75)).await;

            cosmic::Action::App(Message::GeometrySettled(generation))
        })
    }

    fn try_open_overlays(&mut self) -> Task<cosmic::Action<Message>> {
        if !matches!(self.state, AppState::Capturing) {
            return Task::none();
        }

        if self.screenshot.is_none() || self.display.len() == 0 {
            return Task::none();
        }

        let images = match self.build_overlay_images() {
            Ok(images) => images,

            Err(error) => {
                eprintln!(
                    "popshot: invalid display geometry: \
                         {error}"
                );

                return cosmic::iced::exit();
            }
        };

        self.overlay_images = images;
        self.state = AppState::Selecting;

        let tasks = self
            .display
            .outputs()
            .iter()
            .map(|output| crate::overlay::open(output.overlay_id, output.output.clone()))
            .collect::<Vec<_>>();

        Task::batch(tasks)
    }

    fn build_overlay_images(&self) -> Result<HashMap<window::Id, Handle>, String> {
        let screenshot = self
            .screenshot
            .as_ref()
            .ok_or_else(|| "screenshot is unavailable".to_string())?;

        let mut result = HashMap::new();

        /*
         * Single-output screenshots may retain physical capture
         * resolution, so keep the whole source image and let iced
         * uniformly scale it onto the logical output surface.
         */
        if let Some(output) = self.display.single_output() {
            let Some((scale_x, scale_y)) = output
                .geometry
                .image_scale(screenshot.width, screenshot.height)
            else {
                return Err(format!(
                    "single-output screenshot {}x{} \
                     does not match logical output {}x{}",
                    screenshot.width,
                    screenshot.height,
                    output.geometry.size.0,
                    output.geometry.size.1,
                ));
            };

            eprintln!(
                "popshot: single output: \
                 logical={}x{}, screenshot={}x{}, \
                 scale={scale_x:.4}x{scale_y:.4}",
                output.geometry.size.0, output.geometry.size.1, screenshot.width, screenshot.height,
            );

            result.insert(
                output.overlay_id,
                Handle::from_rgba(
                    screenshot.width,
                    screenshot.height,
                    screenshot.rgba.to_vec(),
                ),
            );

            return Ok(result);
        }

        /*
         * For multiple outputs COSMIC's portal constructs the returned
         * PNG directly in logical desktop coordinates.
         */
        let bounds = self
            .display
            .bounds()
            .ok_or_else(|| "desktop has no bounds".to_string())?;

        let width = bounds
            .width()
            .ok_or_else(|| "invalid desktop width".to_string())?;

        let height = bounds
            .height()
            .ok_or_else(|| "invalid desktop height".to_string())?;

        if screenshot.width != width || screenshot.height != height {
            return Err(format!(
                "multi-output screenshot is {}x{}, \
                 but logical desktop is {width}x{height}",
                screenshot.width, screenshot.height,
            ));
        }

        eprintln!(
            "popshot: {} outputs: \
             logical desktop={}x{}, screenshot={}x{}",
            self.display.len(),
            width,
            height,
            screenshot.width,
            screenshot.height,
        );

        for output in self.display.outputs() {
            let (x, y, width, height) = bounds
                .image_region(output.geometry)
                .ok_or_else(|| "output lies outside desktop bounds".to_string())?;

            let rgba = crate::image_ops::extract_rgba(screenshot, x, y, width, height)?;

            result.insert(output.overlay_id, Handle::from_rgba(width, height, rgba));
        }

        Ok(result)
    }

    fn close_overlays(&self) -> Task<cosmic::Action<Message>> {
        close_overlay_ids(self.display.overlay_ids())
    }

    fn finish_selection(&mut self) -> Task<cosmic::Action<Message>> {
        if !matches!(self.state, AppState::Selecting) {
            return Task::none();
        }

        let Some(rect) = self.selection_rect else {
            self.selection_start = None;
            return Task::none();
        };

        if !rect.is_valid() {
            self.selection_start = None;
            self.selection_rect = None;

            return Task::none();
        }

        let Some(bounds) = self
            .display
            .bounds()
            .and_then(|bounds| bounds.logical_rect())
        else {
            eprintln!("popshot: desktop bounds disappeared");

            return self.close_overlays().chain(cosmic::iced::exit());
        };

        let Some(selection) = Selection::from_logical_rect(rect, bounds) else {
            self.selection_start = None;
            self.selection_rect = None;

            return Task::none();
        };

        let Some(screenshot) = self.screenshot.take() else {
            eprintln!("popshot: captured screenshot disappeared");

            return self.close_overlays().chain(cosmic::iced::exit());
        };

        self.state = AppState::Processing;

        self.overlay_images.clear();
        self.selection_start = None;
        self.selection_rect = None;

        let processing = cosmic::task::future(async move {
            let result = async {
                let png = crate::image_ops::crop_to_png(&screenshot, selection)?;

                crate::clipboard::copy_png(&png).await?;

                Ok(())
            }
            .await;

            cosmic::Action::App(Message::SelectionProcessed(result))
        });

        self.close_overlays().chain(processing)
    }
}

fn close_overlay_ids(ids: Vec<window::Id>) -> Task<cosmic::Action<Message>> {
    let tasks = ids
        .into_iter()
        .map(crate::overlay::close)
        .collect::<Vec<_>>();

    Task::batch(tasks)
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

    fn init(
        core: cosmic::Core,
        _flags: Self::Flags,
    ) -> (Self, Task<cosmic::Action<Self::Message>>) {
        let app = Self {
            core,

            state: AppState::Capturing,

            screenshot: None,

            overlay_images: HashMap::new(),

            display: DisplayState::default(),

            selection_start: None,
            selection_rect: None,

            geometry_generation: 0,
        };

        let task = cosmic::task::future(async {
            cosmic::Action::App(Message::CaptureFinished(capture::capture_desktop().await))
        });

        (app, task)
    }

    fn view(&self) -> Element<'_, Self::Message> {
        widget::space()
            .width(Length::Fixed(1.0))
            .height(Length::Fixed(1.0))
            .into()
    }

    fn view_window(&self, id: window::Id) -> Element<'_, Self::Message> {
        let Some(output) = self.display.output_for_overlay(id) else {
            return widget::space()
                .width(Length::Fixed(1.0))
                .height(Length::Fixed(1.0))
                .into();
        };

        let Some(handle) = self.overlay_images.get(&id) else {
            return widget::space()
                .width(Length::Fill)
                .height(Length::Fill)
                .into();
        };

        let screenshot = widget::image(handle.clone())
            .width(Length::Fill)
            .height(Length::Fill)
            .content_fit(ContentFit::Fill);

        let selector = crate::selection::view(
            id,
            output.dnd_id,
            output.geometry.logical_rect(),
            self.selection_rect,
        )
        .map(Message::Selection);

        Stack::with_children([screenshot.into(), selector])
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        event::listen_with(|event, _status, _window| match event {
            Event::PlatformSpecific(event::PlatformSpecific::Wayland(
                event::wayland::Event::Output(output_event, output),
            )) => Some(Message::OutputChanged(output_event, output)),

            Event::Keyboard(KeyEvent::KeyPressed {
                key: Key::Named(Named::Escape),
                ..
            }) => Some(Message::Cancel),

            _ => None,
        })
    }

    fn update(&mut self, message: Self::Message) -> Task<cosmic::Action<Self::Message>> {
        match message {
            Message::CaptureFinished(result) => match result {
                Ok(screenshot) => {
                    self.screenshot = Some(screenshot);

                    self.schedule_geometry_check()
                }

                Err(error) => {
                    eprintln!(
                        "popshot: capture failed: \
                             {error}"
                    );

                    cosmic::iced::exit()
                }
            },

            Message::OutputChanged(event, output) => {
                if matches!(self.state, AppState::Selecting) {
                    /*
                     * Freeze geometry while selecting. Hotplugging or
                     * rearranging displays invalidates every coordinate
                     * relationship established for the current capture.
                     */
                    let ids = self.display.overlay_ids();

                    self.display.handle_output_event(event, output);

                    eprintln!(
                        "popshot: display configuration \
                         changed while selecting"
                    );

                    return close_overlay_ids(ids).chain(cosmic::iced::exit());
                }

                self.display.handle_output_event(event, output);

                if matches!(self.state, AppState::Capturing) {
                    self.schedule_geometry_check()
                } else {
                    Task::none()
                }
            }

            Message::GeometrySettled(generation) => {
                if generation != self.geometry_generation {
                    return Task::none();
                }

                self.try_open_overlays()
            }

            Message::Selection(event) => {
                if !matches!(self.state, AppState::Selecting) {
                    return Task::none();
                }

                match event {
                    SelectionEvent::Started(point) => {
                        self.selection_start = Some(point);

                        self.selection_rect = Some(LogicalRect::from_points(point, point));

                        Task::none()
                    }

                    SelectionEvent::Moved(point) => {
                        let Some(start) = self.selection_start else {
                            return Task::none();
                        };

                        self.selection_rect = Some(LogicalRect::from_points(start, point));

                        Task::none()
                    }

                    SelectionEvent::Finished => self.finish_selection(),

                    SelectionEvent::Cancelled => {
                        self.selection_start = None;

                        self.selection_rect = None;

                        Task::none()
                    }
                }
            }

            Message::Cancel => {
                if !matches!(self.state, AppState::Selecting) {
                    return Task::none();
                }

                self.screenshot = None;
                self.overlay_images.clear();

                self.close_overlays().chain(cosmic::iced::exit())
            }

            Message::SelectionProcessed(result) => {
                if let Err(error) = result {
                    eprintln!(
                        "popshot: failed to process \
                         selection: {error}"
                    );
                }

                cosmic::iced::exit()
            }
        }
    }
}
