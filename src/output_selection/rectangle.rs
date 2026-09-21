use cosmic::{
    iced::{
        self, Color, Length, Point, Rectangle, Size,
        core::{
            Layout, Renderer, Shell,
            layout::Node,
            renderer::Quad,
            widget::{Tree, tree},
        },
        mouse,
    },
    widget::Widget,
};

#[derive(Default)]
struct Drag {
    start: Option<Point>,
    end: Point,
}

impl Drag {
    fn rectangle(&self) -> Option<Rectangle> {
        let start = self.start?;

        Some(Rectangle {
            x: start.x.min(self.end.x),
            y: start.y.min(self.end.y),
            width: (start.x - self.end.x).abs(),
            height: (start.y - self.end.y).abs(),
        })
    }
}

pub struct RectangleSelection<Message> {
    pub on_select: fn([f32; 4]) -> Message,
    pub on_drag: fn(bool) -> Message,
}

impl<Message: Clone + 'static> Widget<Message, cosmic::Theme, cosmic::Renderer>
    for RectangleSelection<Message>
{
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }

    fn state(&self) -> tree::State {
        tree::State::new(Drag::default())
    }

    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<Drag>()
    }

    fn layout(&mut self, _: &mut Tree, _: &cosmic::Renderer, limits: &iced::Limits) -> Node {
        Node::new(limits.resolve(Length::Fill, Length::Fill, Size::ZERO))
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut cosmic::Renderer,
        _: &cosmic::Theme,
        _: &iced::core::renderer::Style,
        layout: Layout<'_>,
        _: mouse::Cursor,
        _: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let selected = tree.state.downcast_ref::<Drag>().rectangle();

        let mut fill = |bounds: Rectangle| {
            renderer.fill_quad(
                Quad {
                    bounds,
                    ..Default::default()
                },
                Color::from_rgba(0.0, 0.0, 0.0, 0.45),
            )
        };

        if let Some(r) = selected {
            fill(Rectangle {
                height: r.y,
                ..bounds
            });
            fill(Rectangle {
                y: bounds.y + r.y + r.height,
                height: bounds.height - r.y - r.height,
                ..bounds
            });
            fill(Rectangle {
                y: bounds.y + r.y,
                width: r.x,
                height: r.height,
                ..bounds
            });
            fill(Rectangle {
                x: bounds.x + r.x + r.width,
                y: bounds.y + r.y,
                width: bounds.width - r.x - r.width,
                height: r.height,
            });
            renderer.fill_quad(
                Quad {
                    bounds: Rectangle {
                        x: bounds.x + r.x,
                        y: bounds.y + r.y,
                        ..r
                    },
                    border: iced::Border {
                        width: 2.0,
                        color: Color::WHITE,
                        ..Default::default()
                    },
                    ..Default::default()
                },
                Color::TRANSPARENT,
            );
        } else {
            fill(bounds);
        }
    }

    fn mouse_interaction(
        &self,
        _: &Tree,
        _: Layout<'_>,
        _: mouse::Cursor,
        _: &Rectangle,
        _: &cosmic::Renderer,
    ) -> mouse::Interaction {
        mouse::Interaction::Crosshair
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &iced::Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _: &cosmic::Renderer,
        _: &mut dyn iced::core::Clipboard,
        shell: &mut Shell<'_, Message>,
        _: &Rectangle,
    ) {
        let drag = tree.state.downcast_mut::<Drag>();
        let bounds = layout.bounds();
        let point = cursor.position().map(|p| {
            Point::new(
                (p.x - bounds.x).clamp(0.0, bounds.width),
                (p.y - bounds.y).clamp(0.0, bounds.height),
            )
        });

        match event {
            iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
                if cursor.is_over(bounds) =>
            {
                drag.start = point;
                drag.end = point.unwrap();
                shell.publish((self.on_drag)(true));
            }

            iced::Event::Mouse(mouse::Event::CursorMoved { .. }) if drag.start.is_some() => {
                if let Some(point) = point {
                    drag.end = point;
                }
            }

            iced::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
                if drag.start.is_some() =>
            {
                if let Some(point) = point {
                    drag.end = point;
                }

                if let Some(r) = drag
                    .rectangle()
                    .filter(|r| r.width >= 2.0 && r.height >= 2.0)
                {
                    shell.publish((self.on_select)([
                        r.x / bounds.width,
                        r.y / bounds.height,
                        (r.x + r.width) / bounds.width,
                        (r.y + r.height) / bounds.height,
                    ]));
                }

                drag.start = None;
                shell.publish((self.on_drag)(false));
            }
            _ => return,
        }

        shell.request_redraw();
        shell.capture_event();
    }
}
