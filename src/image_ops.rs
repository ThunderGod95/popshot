use image::{ExtendedColorType, ImageEncoder, codecs::png::PngEncoder};

use crate::{capture::CapturedImage, geometry::Selection};

pub fn extract_rgba(
    image: &CapturedImage,
    left: u32,
    top: u32,
    width: u32,
    height: u32,
) -> Result<Vec<u8>, String> {
    if width == 0 || height == 0 {
        return Err("image region is empty".into());
    }

    let right = left
        .checked_add(width)
        .ok_or_else(|| "image region overflowed".to_string())?;

    let bottom = top
        .checked_add(height)
        .ok_or_else(|| "image region overflowed".to_string())?;

    if right > image.width || bottom > image.height {
        return Err("image region exceeded screenshot bounds".into());
    }

    let source_stride = image.width as usize * 4;
    let crop_stride = width as usize * 4;

    let capacity = crop_stride
        .checked_mul(height as usize)
        .ok_or_else(|| "image region is too large".to_string())?;

    let mut rgba = Vec::with_capacity(capacity);

    for y in top..bottom {
        let start = y as usize * source_stride + left as usize * 4;

        let end = start + crop_stride;

        let row = image
            .rgba
            .get(start..end)
            .ok_or_else(|| "image region exceeded screenshot bounds".to_string())?;

        rgba.extend_from_slice(row);
    }

    Ok(rgba)
}

pub fn crop_to_png(image: &CapturedImage, selection: Selection) -> Result<Vec<u8>, String> {
    let image_width = image.width as f32;
    let image_height = image.height as f32;

    let left = (selection.left() * image_width)
        .floor()
        .clamp(0.0, image_width) as u32;

    let top = (selection.top() * image_height)
        .floor()
        .clamp(0.0, image_height) as u32;

    let right = (selection.right() * image_width)
        .ceil()
        .clamp(0.0, image_width) as u32;

    let bottom = (selection.bottom() * image_height)
        .ceil()
        .clamp(0.0, image_height) as u32;

    if right <= left || bottom <= top {
        return Err("selection is empty".into());
    }

    let width = right - left;
    let height = bottom - top;

    let rgba = extract_rgba(image, left, top, width, height)?;

    let mut png = Vec::new();

    PngEncoder::new(&mut png)
        .write_image(&rgba, width, height, ExtendedColorType::Rgba8)
        .map_err(|error| format!("failed to encode screenshot: {error}"))?;

    Ok(png)
}
