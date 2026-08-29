use std::sync::Arc;

use ashpd::desktop::screenshot::Screenshot;
use image::ImageFormat;

#[derive(Debug, Clone)]
pub struct CapturedImage {
    pub width: u32,
    pub height: u32,
    /// Original PMG returned bu the screenshot portal.
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

    let uri = response.uri();

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
