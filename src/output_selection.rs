pub mod freehand;
pub mod rectangle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureMode {
    Rectangle,
    Freehand,
    Fullscreen,
}

impl CaptureMode {
    pub const ALL: [Self; 3] = [Self::Rectangle, Self::Freehand, Self::Fullscreen];

    pub fn label(self) -> &'static str {
        match self {
            Self::Rectangle => "Rectangle",
            Self::Freehand => "Freehand",
            Self::Fullscreen => "Fullscreen",
        }
    }
}

/// Coordinates are normalized to the screenshot, independent of output scale.
#[derive(Debug, Clone)]
pub enum Selection {
    Rectangle([f32; 4]),
    Freehand(Vec<cosmic::iced::Point>),
}
