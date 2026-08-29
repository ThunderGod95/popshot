use std::borrow::Cow;

use cosmic::{
    Element,
    iced::{
        Color, Length, Point, Rectangle, Size,
        clipboard::{
            dnd::{self, DndAction, DndDestinationRectangle, DndEvent, OfferEvent, SourceEvent},
            mime::{AllowedMimeTypes, AsMimeTypes},
        },
        core::{Clipboard, Layout, Shell, clipboard::DndSource, layout::Node, widget::Tree},
        mouse,
        widget::{Stack, canvas},
        window,
    },
    widget::{self, Widget},
};

use crate::geometry::{LogicalPoint, LogicalRect};

const MIME: &str = "application/x-popshot-selection";

#[derive(Debug, Clone, Copy)]
pub enum SelectionEvent {
    Started(LogicalPoint),
    Moved(LogicalPoint),
    Finished,
    Cancelled,
}

struct DragData;

impl From<(Vec<u8>, String)> for DragData {
    fn from(_: (Vec<u8>, String)) -> Self {
        Self
    }
}

impl AllowedMimeTypes for DragData {
    fn allowed() -> Cow<'static, [String]> {
        Cow::Owned(vec![MIME.to_string()])
    }
}

impl AsMimeTypes for DragData {
    fn available(&self) -> Cow<'static, [String]> {
        Cow::Owned(vec![MIME.to_string()])
    }

    fn as_bytes(&self, _mime_type: &str) -> Option<Cow<'static, [u8]>> {
        Some(Cow::Borrowed(b"selection"))
    }
}

struct SelectionInput {
    window_id: window::Id,
    dnd_id: u128,
    output_rect: LogicalRect,
    widget_id: widget::Id,
}

impl SelectionInput {
    fn new(window_id: window::Id, dnd_id: u128, output_rect: LogicalRect) -> Self {
        Self {
            window_id,
            dnd_id,
            output_rect,
            widget_id: widget::Id::unique(),
        }
    }

    fn global_point(&self, x: f32, y: f32) -> LogicalPoint {
        LogicalPoint {
            x: self.output_rect.x + x,
            y: self.output_rect.y + y,
        }
    }
}

impl Widget<SelectionEvent, cosmic::Theme, cosmic::Renderer> for SelectionInput {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        _renderer: &cosmic::Renderer,
        limits: &cosmic::iced::core::layout::Limits,
    ) -> Node {
        Node::new(limits.width(Length::Fill).height(Length::Fill).resolve(
            Length::Fill,
            Length::Fill,
            Size::ZERO,
        ))
    }

    fn tag(&self) -> cosmic::iced::core::widget::tree::Tag {
        struct State;

        cosmic::iced::core::widget::tree::Tag::of::<State>()
    }

    fn update(
        &mut self,
        _tree: &mut Tree,
        event: &cosmic::iced::Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &cosmic::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, SelectionEvent>,
        _viewport: &Rectangle,
    ) {
        match event {
            cosmic::iced::Event::Dnd(DndEvent::Offer(id, event)) if *id == Some(self.dnd_id) => {
                match event {
                    OfferEvent::Enter { x, y, .. } | OfferEvent::Motion { x, y } => {
                        let point = Point::new(*x as f32, *y as f32);

                        if !mouse::Cursor::Available(point).is_over(layout.bounds()) {
                            return;
                        }

                        shell.publish(SelectionEvent::Moved(
                            self.global_point(*x as f32, *y as f32),
                        ));

                        shell.capture_event();
                    }

                    OfferEvent::Drop => {
                        shell.publish(SelectionEvent::Finished);

                        shell.capture_event();
                    }

                    _ => {}
                }
            }

            cosmic::iced::Event::Dnd(DndEvent::Source(event)) => match event {
                SourceEvent::Cancelled => {
                    shell.publish(SelectionEvent::Cancelled);
                }

                SourceEvent::Dropped | SourceEvent::Finished => {
                    shell.publish(SelectionEvent::Finished);
                }

                _ => {}
            },

            cosmic::iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let Some(position) = cursor.position_in(layout.bounds()) else {
                    return;
                };

                clipboard.start_dnd(
                    false,
                    Some(DndSource::Surface(self.window_id)),
                    None,
                    Box::new(DragData),
                    DndAction::Copy,
                );

                shell.publish(SelectionEvent::Started(
                    self.global_point(position.x, position.y),
                ));

                shell.capture_event();
            }

            cosmic::iced::Event::Mouse(_) => {
                if cursor.is_over(layout.bounds()) {
                    shell.capture_event();
                }
            }

            _ => {}
        }
    }

    fn draw(
        &self,
        _tree: &Tree,
        _renderer: &mut cosmic::Renderer,
        _theme: &cosmic::Theme,
        _style: &cosmic::iced::core::renderer::Style,
        _layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
    }

    fn mouse_interaction(
        &self,
        _tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &cosmic::Renderer,
    ) -> mouse::Interaction {
        if cursor.is_over(layout.bounds()) {
            mouse::Interaction::Crosshair
        } else {
            mouse::Interaction::default()
        }
    }

    fn drag_destinations(
        &self,
        _tree: &Tree,
        layout: Layout<'_>,
        _renderer: &cosmic::Renderer,
        rectangles: &mut cosmic::iced::core::clipboard::DndDestinationRectangles,
    ) {
        let bounds = layout.bounds();

        rectangles.push(DndDestinationRectangle {
            id: self.dnd_id,
            rectangle: dnd::Rectangle {
                x: bounds.x as f64,
                y: bounds.y as f64,
                width: bounds.width as f64,
                height: bounds.height as f64,
            },
            mime_types: vec![Cow::Borrowed(MIME)],
            actions: DndAction::Copy,
            preferred: DndAction::Copy,
        });
    }

    fn set_id(&mut self, id: widget::Id) {
        self.widget_id = id;
    }
}

impl<'a> From<SelectionInput> for Element<'a, SelectionEvent> {
    fn from(widget: SelectionInput) -> Self {
        Element::new(widget)
    }
}

struct SelectionVisual {
    output_rect: LogicalRect,
    selection: Option<LogicalRect>,
}

impl canvas::Program<SelectionEvent, cosmic::Theme, cosmic::Renderer> for SelectionVisual {
    type State = ();

    fn update(
        &self,
        _state: &mut Self::State,
        _event: &canvas::Event,
        _bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Option<canvas::Action<SelectionEvent>> {
        None
    }

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &cosmic::Renderer,
        _theme: &cosmic::Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry<cosmic::Renderer>> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());

        let mut dim = Color::BLACK;
        dim.a = 0.48;

        let Some(selection) = self.selection else {
            fill_rect(
                &mut frame,
                Rectangle::new(Point::ORIGIN, bounds.size()),
                dim,
            );

            return vec![frame.into_geometry()];
        };

        let Some(intersection) = selection.intersection(self.output_rect) else {
            fill_rect(
                &mut frame,
                Rectangle::new(Point::ORIGIN, bounds.size()),
                dim,
            );

            return vec![frame.into_geometry()];
        };

        let local = LogicalRect {
            x: intersection.x - self.output_rect.x,
            y: intersection.y - self.output_rect.y,
            width: intersection.width,
            height: intersection.height,
        };

        draw_dimmed_outside(&mut frame, bounds.size(), local, dim);

        if selection.is_valid() {
            draw_selection_edges(&mut frame, self.output_rect, selection);

            let top_left = LogicalPoint {
                x: selection.x,
                y: selection.y,
            };

            if self.output_rect.contains(top_left) {
                draw_dimensions(&mut frame, bounds.size(), self.output_rect, selection);
            }
        }

        vec![frame.into_geometry()]
    }
}

fn draw_dimmed_outside(
    frame: &mut canvas::Frame,
    canvas_size: Size,
    selection: LogicalRect,
    color: Color,
) {
    let right = selection.right();
    let bottom = selection.bottom();

    fill_rect(
        frame,
        Rectangle::new(
            Point::ORIGIN,
            Size::new(canvas_size.width, selection.y.max(0.0)),
        ),
        color,
    );

    fill_rect(
        frame,
        Rectangle::new(
            Point::new(0.0, bottom),
            Size::new(canvas_size.width, (canvas_size.height - bottom).max(0.0)),
        ),
        color,
    );

    fill_rect(
        frame,
        Rectangle::new(
            Point::new(0.0, selection.y),
            Size::new(selection.x.max(0.0), selection.height),
        ),
        color,
    );

    fill_rect(
        frame,
        Rectangle::new(
            Point::new(right, selection.y),
            Size::new((canvas_size.width - right).max(0.0), selection.height),
        ),
        color,
    );
}

fn draw_selection_edges(frame: &mut canvas::Frame, output: LogicalRect, selection: LogicalRect) {
    let thickness = 2.0;

    let horizontal_left = selection.x.max(output.x);

    let horizontal_right = selection.right().min(output.right());

    if horizontal_right > horizontal_left {
        if selection.y >= output.y && selection.y <= output.bottom() {
            fill_rect(
                frame,
                Rectangle::new(
                    Point::new(
                        horizontal_left - output.x,
                        selection.y - output.y - thickness / 2.0,
                    ),
                    Size::new(horizontal_right - horizontal_left, thickness),
                ),
                Color::WHITE,
            );
        }

        if selection.bottom() >= output.y && selection.bottom() <= output.bottom() {
            fill_rect(
                frame,
                Rectangle::new(
                    Point::new(
                        horizontal_left - output.x,
                        selection.bottom() - output.y - thickness / 2.0,
                    ),
                    Size::new(horizontal_right - horizontal_left, thickness),
                ),
                Color::WHITE,
            );
        }
    }

    let vertical_top = selection.y.max(output.y);

    let vertical_bottom = selection.bottom().min(output.bottom());

    if vertical_bottom > vertical_top {
        if selection.x >= output.x && selection.x <= output.right() {
            fill_rect(
                frame,
                Rectangle::new(
                    Point::new(
                        selection.x - output.x - thickness / 2.0,
                        vertical_top - output.y,
                    ),
                    Size::new(thickness, vertical_bottom - vertical_top),
                ),
                Color::WHITE,
            );
        }

        if selection.right() >= output.x && selection.right() <= output.right() {
            fill_rect(
                frame,
                Rectangle::new(
                    Point::new(
                        selection.right() - output.x - thickness / 2.0,
                        vertical_top - output.y,
                    ),
                    Size::new(thickness, vertical_bottom - vertical_top),
                ),
                Color::WHITE,
            );
        }
    }
}

fn draw_dimensions(
    frame: &mut canvas::Frame,
    canvas_size: Size,
    output: LogicalRect,
    selection: LogicalRect,
) {
    let text = format!(
        "{} × {}",
        selection.width.round() as u32,
        selection.height.round() as u32,
    );

    let label_width = 96.0;
    let label_height = 26.0;

    let local_x = selection.x - output.x;

    let local_y = selection.y - output.y;

    let label_x = local_x.clamp(0.0, (canvas_size.width - label_width).max(0.0));

    let preferred_y = if local_y >= label_height + 8.0 {
        local_y - label_height - 6.0
    } else {
        local_y + selection.height + 6.0
    };

    let label_y = preferred_y.clamp(0.0, (canvas_size.height - label_height).max(0.0));

    let label_rect = Rectangle::new(
        Point::new(label_x, label_y),
        Size::new(label_width, label_height),
    );

    let mut background = Color::BLACK;
    background.a = 0.75;

    fill_rect(frame, label_rect, background);

    frame.fill_text(canvas::Text {
        content: text,
        position: Point::new(label_x + 8.0, label_y + 5.0),
        color: Color::WHITE,
        size: 14.0.into(),
        ..Default::default()
    });
}

fn fill_rect(frame: &mut canvas::Frame, rectangle: Rectangle, color: Color) {
    if rectangle.width <= 0.0 || rectangle.height <= 0.0 {
        return;
    }

    let path = canvas::Path::rectangle(rectangle.position(), rectangle.size());

    frame.fill(&path, color);
}

pub fn view(
    window_id: window::Id,
    dnd_id: u128,
    output_rect: LogicalRect,
    selection: Option<LogicalRect>,
) -> Element<'static, SelectionEvent> {
    let visual = canvas::Canvas::new(SelectionVisual {
        output_rect,
        selection,
    })
    .width(Length::Fill)
    .height(Length::Fill);

    let input: Element<'static, SelectionEvent> =
        SelectionInput::new(window_id, dnd_id, output_rect).into();

    Stack::with_children([visual.into(), input])
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
