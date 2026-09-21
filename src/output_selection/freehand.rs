use cosmic::{
    iced::{
        self, Color, Length, Point, Rectangle, Size,
        core::{
            Layout, Renderer, Shell,
            layout::Node,
            widget::{Tree, tree},
        },
        mouse,
    },
    widget::Widget,
};

#[derive(Default)]
struct Drag {
    points: Vec<Point>,
}

impl Drag {
    fn push(&mut self, point: Point) {
        if self.points.last() != Some(&point) {
            self.points.push(point);
        }
    }
}

pub struct FreehandSelection<Message> {
    pub on_select: fn(Vec<Point>) -> Message,
    pub on_drag: fn(bool) -> Message,
}

impl<Message: Clone + 'static> Widget<Message, cosmic::Theme, cosmic::Renderer>
    for FreehandSelection<Message>
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
        use cosmic::iced::advanced::graphics::geometry::{self, Renderer as _};
        use cosmic::widget::canvas::{Frame, Path, Stroke};

        let bounds = layout.bounds();
        let points = &tree.state.downcast_ref::<Drag>().points;
        let outline = |builder: &mut geometry::path::Builder| {
            if let Some(first) = points.first() {
                builder.move_to(*first);
                for point in &points[1..] {
                    builder.line_to(*point);
                }
                builder.close();
            }
        };
        let shade = Path::new(|builder| {
            builder.rectangle(Point::ORIGIN, bounds.size());
            outline(builder);
        });
        let mut frame = Frame::new(renderer, bounds.size());
        frame.fill(
            &shade,
            geometry::Fill {
                style: Color::from_rgba(0.0, 0.0, 0.0, 0.45).into(),
                rule: geometry::fill::Rule::EvenOdd,
            },
        );
        frame.stroke(
            &Path::new(outline),
            Stroke::default().with_color(Color::WHITE).with_width(2.0),
        );
        renderer.with_translation(iced::Vector::new(bounds.x, bounds.y), |renderer| {
            renderer.draw_geometry(frame.into_geometry());
        });
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
                drag.points.clear();
                drag.push(point.unwrap());
                shell.publish((self.on_drag)(true));
            }

            iced::Event::Mouse(mouse::Event::CursorMoved { .. }) if !drag.points.is_empty() => {
                if let Some(point) = point {
                    drag.push(point);
                }
            }

            iced::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
                if !drag.points.is_empty() =>
            {
                if let Some(point) = point {
                    drag.push(point);
                }

                if drag.points.len() >= 3 {
                    shell.publish((self.on_select)(
                        drag.points
                            .iter()
                            .map(|p| Point::new(p.x / bounds.width, p.y / bounds.height))
                            .collect(),
                    ));
                }

                drag.points.clear();
                shell.publish((self.on_drag)(false));
            }
            _ => return,
        }

        shell.request_redraw();
        shell.capture_event();
    }
}

/// Validate before calculating bounds: NaN must never silently disappear in min/max.
pub fn bounds(points: &[Point]) -> Result<[f32; 4], String> {
    if points.len() < 3 {
        return Err("Draw an area to capture".into());
    }

    let mut bounds = [1.0_f32, 1.0_f32, 0.0_f32, 0.0_f32];

    for p in points {
        if !p.x.is_finite()
            || !p.y.is_finite()
            || !(0.0..=1.0).contains(&p.x)
            || !(0.0..=1.0).contains(&p.y)
        {
            return Err("Selection is outside the screenshot".into());
        }

        bounds[0] = bounds[0].min(p.x);
        bounds[1] = bounds[1].min(p.y);
        bounds[2] = bounds[2].max(p.x);
        bounds[3] = bounds[3].max(p.y);
    }

    Ok(bounds)
}

/// Even-odd scanline fill matches the overlay, including self-intersecting paths.
/// Pixels outside the lasso are cleared completely, including their RGB channels.
pub fn mask(
    image: &mut image::RgbaImage,
    points: &[Point],
    origin: [u32; 2],
    size: [u32; 2],
) -> Result<(), String> {
    let points: Vec<Point> = points
        .iter()
        .map(|p| {
            Point::new(
                p.x * size[0] as f32 - origin[0] as f32,
                p.y * size[1] as f32 - origin[1] as f32,
            )
        })
        .collect();
    let mut intersections = Vec::new();
    let mut selected = false;

    for y in 0..image.height() {
        intersections.clear();

        let scan = y as f32 + 0.5;

        for (a, b) in points.iter().zip(points.iter().cycle().skip(1)) {
            if (a.y > scan) != (b.y > scan) {
                intersections.push(a.x + (scan - a.y) * (b.x - a.x) / (b.y - a.y));
            }
        }

        intersections.sort_by(f32::total_cmp);

        let mut spans = intersections.as_chunks::<2>().0.iter().peekable();

        for x in 0..image.width() {
            let center = x as f32 + 0.5;

            while spans.peek().is_some_and(|span| center >= span[1]) {
                spans.next();
            }

            if spans.peek().is_some_and(|span| center >= span[0]) {
                selected = true;
            } else {
                image.put_pixel(x, y, image::Rgba([0; 4]));
            }
        }
    }

    if !selected {
        return Err("Selection is empty; draw an area to capture".into());
    }

    Ok(())
}
