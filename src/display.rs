use std::sync::atomic::{AtomicU64, Ordering};

use cosmic::iced::{core::event::wayland::OutputEvent, window};

use wayland_client::protocol::wl_output::WlOutput;

use crate::geometry::LogicalRect;

const MAX_SCALE_DIFFERENCE: f64 = 0.01;

static NEXT_DND_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutputGeometry {
    pub position: (i32, i32),
    pub size: (u32, u32),
}

impl OutputGeometry {
    pub fn logical_rect(self) -> LogicalRect {
        LogicalRect {
            x: self.position.0 as f32,
            y: self.position.1 as f32,
            width: self.size.0 as f32,
            height: self.size.1 as f32,
        }
    }

    pub fn image_scale(self, image_width: u32, image_height: u32) -> Option<(f64, f64)> {
        if self.size.0 == 0 || self.size.1 == 0 || image_width == 0 || image_height == 0 {
            return None;
        }

        let scale_x = f64::from(image_width) / f64::from(self.size.0);

        let scale_y = f64::from(image_height) / f64::from(self.size.1);

        let largest = scale_x.max(scale_y);

        if largest <= 0.0 {
            return None;
        }

        let difference = (scale_x - scale_y).abs() / largest;

        if difference > MAX_SCALE_DIFFERENCE {
            return None;
        }

        Some((scale_x, scale_y))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DesktopBounds {
    pub left: i64,
    pub top: i64,
    pub right: i64,
    pub bottom: i64,
}

impl DesktopBounds {
    pub fn width(self) -> Option<u32> {
        u32::try_from(self.right - self.left).ok()
    }

    pub fn height(self) -> Option<u32> {
        u32::try_from(self.bottom - self.top).ok()
    }

    pub fn logical_rect(self) -> Option<LogicalRect> {
        Some(LogicalRect {
            x: self.left as f32,
            y: self.top as f32,
            width: self.width()? as f32,
            height: self.height()? as f32,
        })
    }

    pub fn image_region(self, output: OutputGeometry) -> Option<(u32, u32, u32, u32)> {
        let x = i64::from(output.position.0) - self.left;

        let y = i64::from(output.position.1) - self.top;

        Some((
            u32::try_from(x).ok()?,
            u32::try_from(y).ok()?,
            output.size.0,
            output.size.1,
        ))
    }
}

#[derive(Debug, Clone)]
pub struct DisplayOutput {
    pub output: WlOutput,
    pub overlay_id: window::Id,
    pub dnd_id: u128,
    pub geometry: OutputGeometry,
}

#[derive(Debug, Default)]
pub struct DisplayState {
    outputs: Vec<DisplayOutput>,
}

impl DisplayState {
    pub fn handle_output_event(&mut self, event: OutputEvent, output: WlOutput) {
        match event {
            OutputEvent::Created(Some(info)) => {
                let Some(position) = info.logical_position else {
                    return;
                };

                let Some(size) = valid_size(info.logical_size) else {
                    return;
                };

                self.insert_output(output, OutputGeometry { position, size });
            }

            OutputEvent::InfoUpdate(info) => {
                if let Some(existing) = self
                    .outputs
                    .iter_mut()
                    .find(|existing| existing.output == output)
                {
                    if let Some(position) = info.logical_position {
                        existing.geometry.position = position;
                    }

                    if let Some(size) = valid_size(info.logical_size) {
                        existing.geometry.size = size;
                    }

                    return;
                }

                let Some(position) = info.logical_position else {
                    return;
                };

                let Some(size) = valid_size(info.logical_size) else {
                    return;
                };

                self.insert_output(output, OutputGeometry { position, size });
            }

            OutputEvent::Removed => {
                self.outputs.retain(|existing| existing.output != output);
            }

            _ => {}
        }
    }

    fn insert_output(&mut self, output: WlOutput, geometry: OutputGeometry) {
        if let Some(existing) = self
            .outputs
            .iter_mut()
            .find(|existing| existing.output == output)
        {
            existing.geometry = geometry;
            return;
        }

        self.outputs.push(DisplayOutput {
            output,
            overlay_id: window::Id::unique(),
            dnd_id: u128::from(NEXT_DND_ID.fetch_add(1, Ordering::Relaxed)),
            geometry,
        });
    }

    pub fn outputs(&self) -> &[DisplayOutput] {
        &self.outputs
    }

    pub fn len(&self) -> usize {
        self.outputs.len()
    }

    pub fn single_output(&self) -> Option<&DisplayOutput> {
        if self.outputs.len() == 1 {
            self.outputs.first()
        } else {
            None
        }
    }

    pub fn output_for_overlay(&self, id: window::Id) -> Option<&DisplayOutput> {
        self.outputs.iter().find(|output| output.overlay_id == id)
    }

    pub fn overlay_ids(&self) -> Vec<window::Id> {
        self.outputs
            .iter()
            .map(|output| output.overlay_id)
            .collect()
    }

    pub fn bounds(&self) -> Option<DesktopBounds> {
        let first = self.outputs.first()?;

        let mut bounds = bounds_for_output(first.geometry);

        for output in &self.outputs[1..] {
            let rect = bounds_for_output(output.geometry);

            bounds.left = bounds.left.min(rect.left);
            bounds.top = bounds.top.min(rect.top);
            bounds.right = bounds.right.max(rect.right);
            bounds.bottom = bounds.bottom.max(rect.bottom);
        }

        Some(bounds)
    }
}

fn bounds_for_output(geometry: OutputGeometry) -> DesktopBounds {
    let left = i64::from(geometry.position.0);
    let top = i64::from(geometry.position.1);

    DesktopBounds {
        left,
        top,
        right: left + i64::from(geometry.size.0),
        bottom: top + i64::from(geometry.size.1),
    }
}

fn valid_size(size: Option<(i32, i32)>) -> Option<(u32, u32)> {
    let (width, height) = size?;

    let width = u32::try_from(width).ok()?;
    let height = u32::try_from(height).ok()?;

    if width == 0 || height == 0 {
        return None;
    }

    Some((width, height))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_bounds_support_negative_coordinates() {
        let geometries = [
            OutputGeometry {
                position: (-1920, 0),
                size: (1920, 1080),
            },
            OutputGeometry {
                position: (0, 0),
                size: (2560, 1440),
            },
        ];

        let first = bounds_for_output(geometries[0]);
        let second = bounds_for_output(geometries[1]);

        let bounds = DesktopBounds {
            left: first.left.min(second.left),
            top: first.top.min(second.top),
            right: first.right.max(second.right),
            bottom: first.bottom.max(second.bottom),
        };

        assert_eq!(bounds.width(), Some(4480));
        assert_eq!(bounds.height(), Some(1440));
    }

    #[test]
    fn image_region_uses_desktop_origin() {
        let bounds = DesktopBounds {
            left: -1920,
            top: 0,
            right: 2560,
            bottom: 1440,
        };

        let left_monitor = OutputGeometry {
            position: (-1920, 0),
            size: (1920, 1080),
        };

        let right_monitor = OutputGeometry {
            position: (0, 0),
            size: (2560, 1440),
        };

        assert_eq!(bounds.image_region(left_monitor), Some((0, 0, 1920, 1080)),);

        assert_eq!(
            bounds.image_region(right_monitor),
            Some((1920, 0, 2560, 1440)),
        );
    }
}
