use std::sync::Arc;

use ashpd::desktop::{notification::DisplayHint, screenshot::Screenshot};
use image::ImageFormat;

#[derive(Debug, Clone)]
pub struct CapturedImage {
    pub width: u32,
    pub height: u32,
    /// Original PNG returned by the screenshot portal.
    pub png: Arc<[u8]>,
    /// Decoded RGBA image for rendering/editing.
    pub rgba: Arc<[u8]>,
}

pub async fn capture_desktop() -> Result<CapturedImage, String> {
    let request = Screenshot::request()
        .interactive(false)
        .modal(false)
        .send()
        .await
        .map_err(|error| format!("failed to request screenshot: {error}"))?;

    let response = request
        .response()
        .map_err(|error| format!("screenshot request failed: {error}"))?;

    let uri = url::Url::parse(response.uri().as_str())
        .map_err(|error| format!("invalid screenshot URI: {error}"))?;

    if uri.scheme() != "file" {
        return Err(format!("unsupported screenshot URI: {uri}"));
    }

    let path = uri
        .to_file_path()
        .map_err(|_| format!("invalid screenshot file URI: {uri}"))?;

    let png = tokio::fs::read(&path)
        .await
        .map_err(|error| format!("failed to read screenshot {}: {error}", path.display()))?;

    let _ = tokio::fs::remove_file(&path).await;

    decode(png)
}

pub fn decode(png: Vec<u8>) -> Result<CapturedImage, String> {
    let image = image::load_from_memory_with_format(&png, ImageFormat::Png)
        .map_err(|error| format!("failed to decode screenshot: {error}"))?
        .into_rgba8();

    let width = image.width();
    let height = image.height();

    Ok(CapturedImage {
        width,
        height,
        png: Arc::from(png),
        rgba: Arc::from(image.into_raw()),
    })
}

/// All selectors produce image-space bounds; freehand can additionally apply a mask here.
pub fn crop(source: &CapturedImage, region: [f32; 4]) -> Result<CapturedImage, String> {
    if region
        .iter()
        .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
    {
        return Err("Selection is outside the screenshot".into());
    }

    let [left, top, right, bottom] = region;

    let x = (left * source.width as f32).floor() as u32;
    let y = (top * source.height as f32).floor() as u32;
    let right = (right * source.width as f32).ceil() as u32;
    let bottom = (bottom * source.height as f32).ceil() as u32;

    if right <= x || bottom <= y {
        return Err("Selection is empty".into());
    }

    let image = image::RgbaImage::from_raw(source.width, source.height, source.rgba.to_vec())
        .ok_or("Invalid screenshot pixels")?;
    let image = image::imageops::crop_imm(&image, x, y, right - x, bottom - y).to_image();
    let mut png = std::io::Cursor::new(Vec::new());

    image
        .write_to(&mut png, ImageFormat::Png)
        .map_err(|e| e.to_string())?;

    Ok(CapturedImage {
        width: image.width(),
        height: image.height(),
        png: png.into_inner().into(),
        rgba: image.into_raw().into(),
    })
}

pub async fn save(png: &[u8]) -> Result<String, String> {
    use ashpd::desktop::file_chooser::{FileFilter, SelectedFiles};

    let request = SelectedFiles::save_file()
        .title("Save screenshot")
        .current_name("Screenshot.png")
        .filter(FileFilter::new("PNG image").glob("*.png"))
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let response = match request.response() {
        Ok(response) => response,
        Err(ashpd::Error::Response(ashpd::desktop::ResponseError::Cancelled)) => {
            return Ok("Save cancelled".into());
        }
        Err(e) => return Err(e.to_string()),
    };

    let uri = response.uris().first().ok_or("No file selected")?;
    let path = url::Url::parse(uri.as_str())
        .map_err(|_| "Choose a local file")?
        .to_file_path()
        .map_err(|_| "Choose a local file")?;

    // Write beside the destination, then rename so a failed write preserves the existing file.
    let temporary = path.with_file_name(format!(
        ".popshot-{}-{}.png",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));

    let result = async {
        use tokio::io::AsyncWriteExt;
        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .await?;
        file.write_all(png).await?;
        file.sync_all().await?;
        tokio::fs::rename(&temporary, &path).await
    }
    .await;

    if let Err(error) = result {
        let _ = tokio::fs::remove_file(&temporary).await;
        return Err(format!("Could not save screenshot: {error}"));
    }

    Ok(format!("Saved to {}", path.display()))
}

fn notification_thumbnail(image: &CapturedImage) -> Result<Vec<u8>, String> {
    let pixels = image::RgbaImage::from_raw(image.width, image.height, image.rgba.to_vec())
        .ok_or("Invalid screenshot pixels")?;
    let thumbnail = image::DynamicImage::ImageRgba8(pixels)
        .thumbnail(128, 128)
        .into_rgba8();
    let mut square = image::RgbaImage::new(128, 128);

    image::imageops::overlay(
        &mut square,
        &thumbnail,
        i64::from((128 - thumbnail.width()) / 2),
        i64::from((128 - thumbnail.height()) / 2),
    );

    let mut png = std::io::Cursor::new(Vec::new());

    square
        .write_to(&mut png, ImageFormat::Png)
        .map_err(|e| e.to_string())?;

    Ok(png.into_inner())
}

pub async fn notify(image: &CapturedImage, capture_id: &str) -> Result<(), String> {
    use ashpd::desktop::{Icon, notification::Notification};

    let portal = ashpd::desktop::notification::NotificationProxy::new()
        .await
        .map_err(|e| e.to_string())?;

    let portal_version = portal.version();

    let notification = || {
        let notification = Notification::new("Screenshot captured")
            .body(
                "Your screenshot has been copied to the clipboard. \
                     Click to preview or save it.",
            )
            .default_action("app.preview")
            .default_action_target(capture_id);

        if portal_version >= 2 {
            notification.display_hint([DisplayHint::HideContentOnLockScreen])
        } else {
            notification
        }
    };

    if let Ok(thumbnail) = notification_thumbnail(image)
        && portal
            .add_notification(capture_id, notification().icon(Icon::Bytes(thumbnail)))
            .await
            .is_ok()
    {
        return Ok(());
    }

    // Still deliver confirmation if the desktop rejects the thumbnail.
    portal
        .add_notification(capture_id, notification())
        .await
        .map_err(|e| e.to_string())
}
