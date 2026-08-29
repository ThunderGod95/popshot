#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LogicalPoint {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LogicalRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl LogicalRect {
    pub fn from_points(a: LogicalPoint, b: LogicalPoint) -> Self {
        Self {
            x: a.x.min(b.x),
            y: a.y.min(b.y),
            width: (a.x - b.x).abs(),
            height: (a.y - b.y).abs(),
        }
    }

    pub fn right(self) -> f32 {
        self.x + self.width
    }

    pub fn bottom(self) -> f32 {
        self.y + self.height
    }

    pub fn is_valid(self) -> bool {
        self.x.is_finite()
            && self.y.is_finite()
            && self.width.is_finite()
            && self.height.is_finite()
            && self.width >= 2.0
            && self.height >= 2.0
    }

    pub fn intersection(self, other: Self) -> Option<Self> {
        let left = self.x.max(other.x);
        let top = self.y.max(other.y);
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());

        if right <= left || bottom <= top {
            return None;
        }

        Some(Self {
            x: left,
            y: top,
            width: right - left,
            height: bottom - top,
        })
    }

    pub fn contains(self, point: LogicalPoint) -> bool {
        point.x >= self.x && point.x < self.right() && point.y >= self.y && point.y < self.bottom()
    }
}

/// Rectangle normalized relative to the complete screenshot.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Selection {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

impl Selection {
    pub fn from_logical_rect(rect: LogicalRect, bounds: LogicalRect) -> Option<Self> {
        if !rect.is_valid()
            || !bounds.width.is_finite()
            || !bounds.height.is_finite()
            || bounds.width <= 0.0
            || bounds.height <= 0.0
        {
            return None;
        }

        let left = ((rect.x - bounds.x) / bounds.width).clamp(0.0, 1.0);
        let top = ((rect.y - bounds.y) / bounds.height).clamp(0.0, 1.0);

        let right = ((rect.right() - bounds.x) / bounds.width).clamp(0.0, 1.0);

        let bottom = ((rect.bottom() - bounds.y) / bounds.height).clamp(0.0, 1.0);

        if right <= left || bottom <= top {
            return None;
        }

        Some(Self {
            x: left,
            y: top,
            width: right - left,
            height: bottom - top,
        })
    }

    pub fn left(self) -> f32 {
        self.x
    }

    pub fn top(self) -> f32 {
        self.y
    }

    pub fn right(self) -> f32 {
        self.x + self.width
    }

    pub fn bottom(self) -> f32 {
        self.y + self.height
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(x: f32, y: f32) -> LogicalPoint {
        LogicalPoint { x, y }
    }

    #[test]
    fn rect_handles_all_drag_directions() {
        let expected = LogicalRect {
            x: 20.0,
            y: 30.0,
            width: 60.0,
            height: 40.0,
        };

        assert_eq!(
            LogicalRect::from_points(point(20.0, 30.0), point(80.0, 70.0)),
            expected,
        );

        assert_eq!(
            LogicalRect::from_points(point(80.0, 70.0), point(20.0, 30.0)),
            expected,
        );

        assert_eq!(
            LogicalRect::from_points(point(80.0, 30.0), point(20.0, 70.0)),
            expected,
        );

        assert_eq!(
            LogicalRect::from_points(point(20.0, 70.0), point(80.0, 30.0)),
            expected,
        );
    }

    #[test]
    fn global_rect_normalizes_against_offset_desktop() {
        let bounds = LogicalRect {
            x: -1920.0,
            y: 0.0,
            width: 3840.0,
            height: 1080.0,
        };

        let rect = LogicalRect {
            x: -960.0,
            y: 270.0,
            width: 1920.0,
            height: 540.0,
        };

        let selection = Selection::from_logical_rect(rect, bounds).unwrap();

        assert_eq!(selection.left(), 0.25);
        assert_eq!(selection.top(), 0.25);
        assert_eq!(selection.right(), 0.75);
        assert_eq!(selection.bottom(), 0.75);
    }

    #[test]
    fn intersection_works_across_outputs() {
        let selection = LogicalRect {
            x: 1500.0,
            y: 100.0,
            width: 1000.0,
            height: 500.0,
        };

        let output = LogicalRect {
            x: 1920.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        };

        assert_eq!(
            selection.intersection(output),
            Some(LogicalRect {
                x: 1920.0,
                y: 100.0,
                width: 580.0,
                height: 500.0,
            })
        );
    }

    #[test]
    fn tiny_selection_is_invalid() {
        assert!(
            !LogicalRect {
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 20.0,
            }
            .is_valid()
        );
    }
}
