# Repository Metadata
- **Project:** popshot
- **Generated:** 2026-08-29T18:21:15.167Z
- **Files:** 18

---

# Repository Structure
```text
popshot
├── .gitignore
├── Cargo.toml
├── build.rs
├── i18n.toml
├── justfile
├─┬ resources
│ ├── app.desktop
│ ├── app.metainfo.xml
│ └─┬ icons
│   └─┬ hicolor
│     └─┬ scalable
│       └─┬ apps
│         └── icon.svg
└─┬ src
  ├── app.rs
  ├── capture.rs
  ├── clipboard.rs
  ├── display.rs
  ├── geometry.rs
  ├── i18n.rs
  ├── image_ops.rs
  ├── main.rs
  ├── overlay.rs
  └── selection.rs
```

---

# Repository Files

### File: .gitignore

```
.cargo/
*.pdb
**/*.rs.bk
debug/
target/
vendor/
vendor.tar
debian/*
!debian/changelog
!debian/control
!debian/copyright
!debian/install
!debian/rules
!debian/source
```

### File: Cargo.toml

```
[package]
name = "popshot"
version = "0.1.0"
edition = "2024"
license = "MPL-2.0"
description = "Screenshot utility for CosmicDE."
repository = "https://github.com/ThunderGod95/popshot"

[dependencies]
i18n-embed = { version = "0.16", features = [
    "fluent-system",
    "desktop-requester",
] }
i18n-embed-fl = "0.10"
rust-embed = "8.8.0"
tokio = { version = "1.48.0", features = ["full"] }
ashpd = { version = "0.12", default-features = false, features = ["tokio"] }
image = { version = "0.25", default-features = false, features = ["png"] }
wayland-client = "0.31.15"

[build-dependencies]
xdgen = "0.1"

[dependencies.libcosmic]
git = "https://github.com/pop-os/libcosmic.git"
features = [
    "single-instance",
    "wgpu",
]

# Uncomment to test a locally-cloned libcosmic
# [patch.'https://github.com/pop-os/libcosmic']
# libcosmic = { path = "../libcosmic" }
# cosmic-config = { path = "../libcosmic/cosmic-config" }
# cosmic-theme = { path = "../libcosmic/cosmic-theme" }

```

### File: build.rs

```
use std::{env, fs, path::Path};
use xdgen::{App, Context, FluentString};

fn main() {
    let ctx = Context::new("i18n", env::var("CARGO_PKG_NAME").unwrap()).unwrap();
    let app = App::new(FluentString("app-title"))
        .comment(FluentString("app-comment"))
        .keywords(FluentString("app-keywords"));

    let desktop_entry = app.expand_desktop("resources/app.desktop", &ctx).unwrap();
    let metainfo = app
        .expand_metainfo("resources/app.metainfo.xml", &ctx)
        .unwrap();

    let output = Path::new("target/xdgen/");
    fs::create_dir_all(output).unwrap();
    fs::write(output.join("app.desktop"), desktop_entry).unwrap();
    fs::write(output.join("app.metainfo.xml"), metainfo).unwrap();
}

```

### File: i18n.toml

```
fallback_language = "en"

[fluent]
assets_dir = "i18n"
```

### File: justfile

```
# Name of the application's binary.
name := 'popshot'
# The unique ID of the application.
appid := 'io.github.tg.PopShot'

# Path to root file system, which defaults to `/`.
rootdir := ''
# The prefix for the `/usr` directory.
prefix := '/usr'
# The location of the cargo target directory.
cargo-target-dir := env('CARGO_TARGET_DIR', 'target')

# Application's appstream metadata
appdata := appid + '.metainfo.xml'
# Application's desktop entry
desktop := appid + '.desktop'
# Application's icon.
icon-svg := appid + '.svg'

# Install destinations
base-dir := absolute_path(clean(rootdir / prefix))
appdata-dst := base-dir / 'share' / 'appdata' / appdata
bin-dst := base-dir / 'bin' / name
desktop-dst := base-dir / 'share' / 'applications' / desktop
icons-dst := base-dir / 'share' / 'icons' / 'hicolor'
icon-svg-dst := icons-dst / 'scalable' / 'apps'

# Default recipe which runs `just build-release`
default: build-release

# Runs `cargo clean`
clean:
    cargo clean

# Removes vendored dependencies
clean-vendor:
    rm -rf .cargo vendor vendor.tar

# `cargo clean` and removes vendored dependencies
clean-dist: clean clean-vendor

# Compiles with debug profile
build-debug *args:
    cargo build --locked {{args}}

# Compiles with release profile
build-release *args: (build-debug '--release' args)

# Compiles release profile with vendored dependencies
build-vendored *args: vendor-extract (build-release '--frozen --offline' args)

# Runs a clippy check
check *args:
    cargo clippy --all-features --locked {{args}} -- -W clippy::pedantic

# Runs a clippy check with JSON message format
check-json: (check '--message-format=json')

# Run the application for testing purposes
run *args:
    env RUST_BACKTRACE=full cargo run --release --locked {{args}}

# Installs files
install:
    install -Dm0755 {{ cargo-target-dir / 'release' / name }} {{bin-dst}}
    install -Dm0644 {{ 'target' / 'xdgen' / 'app.desktop' }} {{desktop-dst}}
    install -Dm0644 {{ 'target' / 'xdgen' / 'app.metainfo.xml' }} {{appdata-dst}}
    install -Dm0644 {{ 'resources' / 'icons' / 'hicolor' / 'scalable' / 'apps' / 'icon.svg' }} {{icon-svg-dst}}

# Uninstalls installed files
uninstall:
    rm {{bin-dst}} {{desktop-dst}} {{icon-svg-dst}}

# Vendor dependencies locally
vendor:
    mkdir -p .cargo
    cargo vendor | head -n -1 > .cargo/config.toml
    echo 'directory = "vendor"' >> .cargo/config.toml
    tar pcf vendor.tar vendor
    rm -rf vendor

# Extracts vendored dependencies
vendor-extract:
    rm -rf vendor
    tar pxf vendor.tar

# Bump cargo version, create git commit, and create tag
tag version:
    find -type f -name Cargo.toml -exec sed -i '0,/^version/s/^version.*/version = "{{version}}"/' '{}' \; -exec git add '{}' \;
    cargo check
    cargo clean
    git add Cargo.lock
    git commit -m 'release: {{version}}'
    git commit --amend
    git tag -a {{version}} -m ''


```

### File: resources/app.desktop

```
[Desktop Entry]
Name=Popshot
Comment=Screenshot utility for CosmicDE.
Type=Application
Icon=io.github.tg.PopShot
Exec=popshot %F
Terminal=false
StartupNotify=true
Categories=COSMIC
Keywords=COSMIC
MimeType=

```

### File: resources/app.metainfo.xml

```
<?xml version="1.0" encoding="UTF-8"?>
<component type="desktop-application">
  <id>io.github.tg.PopShot</id>
  <metadata_license>CC0-1.0</metadata_license>
  <project_license>MPL-2.0</project_license>
  <name>Popshot</name>
  <summary>Screenshot utility for CosmicDE.</summary>
  <icon type="remote" width="64" height="64" scale="1">
    https://github.com/ThunderGod95/popshot/raw/main/resources/icons/hicolor/scalable/apps/icon.svg
  </icon>
  <url type="vcs-browser">https://github.com/ThunderGod95/popshot</url>
  <launchable type="desktop-id">io.github.tg.PopShot.desktop</launchable>
  <provides>
    <id>com.system76.CosmicApplication</id>
    <binaries>
      <binary>popshot</binary>
    </binaries>
  </provides>
  <requires>
    <display_length compare="ge">360</display_length>
  </requires>
  <supports>
    <control>keyboard</control>
    <control>pointing</control>
    <control>touch</control>
  </supports>
  <categories>
    <category>COSMIC</category>
  </categories>
  <keywords>
    <keyword>COSMIC</keyword>
  </keywords>
  <content_rating type="oars-1.1" />
</component>


```

### File: resources/icons/hicolor/scalable/apps/icon.svg

```
<?xml version="1.0" encoding="UTF-8"?>
<svg viewBox="0 0 128 128" xmlns="http://www.w3.org/2000/svg"/>
```

### File: src/app.rs

```
use crate::{
    capture::{self, CapturedImage},
    display::DisplayState,
    geometry::Selection,
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
    screenshot_handle: Option<Handle>,

    display: DisplayState,

    overlay_id: window::Id,
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

    SelectionFinished(Selection),

    SelectionProcessed(Result<(), String>),

    Cancel,
}

impl AppModel {
    fn try_open_overlay(&mut self) -> Task<cosmic::Action<Message>> {
        if !matches!(self.state, AppState::Capturing) {
            return Task::none();
        }

        let Some(screenshot) = self.screenshot.as_ref() else {
            return Task::none();
        };

        match self.display.len() {
            0 => {
                // Screenshot capture and Wayland output discovery happen
                // independently. Wait until output information arrives.
                Task::none()
            }

            1 => {
                let output = self
                    .display
                    .single_output()
                    .expect("display count was checked above");

                let geometry = output.geometry;

                let Some((scale_x, scale_y)) =
                    geometry.image_scale(screenshot.width, screenshot.height)
                else {
                    eprintln!(
                        "popshot: screenshot geometry does not match \
                         output geometry: screenshot={}x{}, \
                         logical-output={}x{}",
                        screenshot.width,
                        screenshot.height,
                        geometry.logical_width,
                        geometry.logical_height,
                    );

                    return cosmic::iced::exit();
                };

                eprintln!(
                    "popshot: output geometry verified: \
                     logical={}x{}, screenshot={}x{}, \
                     scale={scale_x:.4}x{scale_y:.4}",
                    geometry.logical_width,
                    geometry.logical_height,
                    screenshot.width,
                    screenshot.height,
                );

                self.state = AppState::Selecting;

                crate::overlay::open(self.overlay_id, output.output.clone())
            }

            count => {
                eprintln!(
                    "popshot: {count} outputs detected; \
                     multiple-output selection is not implemented yet"
                );

                cosmic::iced::exit()
            }
        }
    }

    fn close_overlay_and_exit(&self) -> Task<cosmic::Action<Message>> {
        crate::overlay::close(self.overlay_id).chain(cosmic::iced::exit())
    }
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
            screenshot_handle: None,

            display: DisplayState::default(),

            overlay_id: window::Id::unique(),
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
        if id != self.overlay_id {
            return widget::space()
                .width(Length::Fixed(1.0))
                .height(Length::Fixed(1.0))
                .into();
        }

        let Some(handle) = &self.screenshot_handle else {
            return widget::space()
                .width(Length::Fill)
                .height(Length::Fill)
                .into();
        };

        /*
         * The screenshot may be larger than the Wayland surface on
         * HiDPI displays.
         *
         * We deliberately use Fill here because try_open_overlay()
         * has already verified that the X and Y scales are equal
         * within tolerance. Therefore this represents uniform display
         * scaling rather than arbitrary image distortion.
         */
        let screenshot = widget::image(handle.clone())
            .width(Length::Fill)
            .height(Length::Fill)
            .content_fit(ContentFit::Fill);

        let selection = crate::selection::view().map(Message::SelectionFinished);

        Stack::with_children([screenshot.into(), selection])
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
            Message::CaptureFinished(result) => {
                match result {
                    Ok(screenshot) => {
                        self.screenshot_handle = Some(Handle::from_rgba(
                            screenshot.width,
                            screenshot.height,
                            screenshot.rgba.to_vec(),
                        ));

                        self.screenshot = Some(screenshot);

                        /*
                         * Do not transition to Selecting here.
                         *
                         * Screenshot capture and Wayland output
                         * discovery are independent. try_open_overlay()
                         * performs the transition only once both are
                         * available and compatible.
                         */
                        self.try_open_overlay()
                    }

                    Err(error) => {
                        eprintln!("popshot: capture failed: {error}");

                        cosmic::iced::exit()
                    }
                }
            }

            Message::OutputChanged(output_event, output) => {
                self.display.handle_output_event(output_event, output);

                /*
                 * Phase 3A formally supports exactly one output.
                 *
                 * If the display configuration changes after the
                 * overlay has already opened, abort rather than
                 * continuing with potentially-invalid geometry.
                 */
                if matches!(self.state, AppState::Selecting) && self.display.len() != 1 {
                    eprintln!(
                        "popshot: display configuration changed \
                         while selecting"
                    );

                    return self.close_overlay_and_exit();
                }

                self.try_open_overlay()
            }

            Message::Cancel => {
                if !matches!(self.state, AppState::Selecting) {
                    return Task::none();
                }

                self.screenshot = None;
                self.screenshot_handle = None;

                self.close_overlay_and_exit()
            }

            Message::SelectionFinished(selection) => {
                if !matches!(self.state, AppState::Selecting) {
                    return Task::none();
                }

                let Some(screenshot) = self.screenshot.take() else {
                    eprintln!("popshot: captured screenshot disappeared");

                    return self.close_overlay_and_exit();
                };

                /*
                 * Selection is complete. The rendering copy is no
                 * longer needed.
                 */
                self.screenshot_handle = None;

                self.state = AppState::Processing;

                let processing_task = cosmic::task::future(async move {
                    let result = async {
                        let png = crate::image_ops::crop_to_png(&screenshot, selection)?;

                        crate::clipboard::copy_png(&png).await?;

                        Ok(())
                    }
                    .await;

                    cosmic::Action::App(Message::SelectionProcessed(result))
                });

                /*
                 * Close the overlay first so the desktop becomes
                 * immediately usable again. Only then perform crop +
                 * clipboard processing.
                 */
                crate::overlay::close(self.overlay_id).chain(processing_task)
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

```

### File: src/capture.rs

```
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

```

### File: src/clipboard.rs

```
use std::process::Stdio;

use tokio::{io::AsyncWriteExt, process::Command};

pub async fn copy_png(png: &[u8]) -> Result<(), String> {
    let mut child = Command::new("wl-copy")
        .args(["--type", "image/png"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|error| format!("failed to start wl-copy: {error}"))?;

    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| "failed to open wl-copy stdin".to_string())?;

    stdin
        .write_all(png)
        .await
        .map_err(|error| format!("failed to write screenshot to clipboard: {error}"))?;

    // wl-copy reads until EOF.
    drop(stdin);

    let status = child
        .wait()
        .await
        .map_err(|error| format!("failed waiting for wl-copy: {error}"))?;

    if !status.success() {
        return Err(format!("wl-copy exited with status {status}"));
    }

    Ok(())
}

```

### File: src/display.rs

```
use cosmic::iced::core::event::wayland::OutputEvent;
use wayland_client::protocol::wl_output::WlOutput;

const MAX_SCALE_DIFFERENCE: f64 = 0.01;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutputGeometry {
    pub logical_width: u32,
    pub logical_height: u32,
}

impl OutputGeometry {
    pub fn image_scale(self, image_width: u32, image_height: u32) -> Option<(f64, f64)> {
        if self.logical_width == 0
            || self.logical_height == 0
            || image_width == 0
            || image_height == 0
        {
            return None;
        }

        let scale_x = f64::from(image_width) / f64::from(self.logical_width);

        let scale_y = f64::from(image_height) / f64::from(self.logical_height);

        let largest_scale = scale_x.max(scale_y);

        let relative_difference = (scale_x - scale_y).abs() / largest_scale;

        if relative_difference > MAX_SCALE_DIFFERENCE {
            return None;
        }

        Some((scale_x, scale_y))
    }
}

#[derive(Debug, Clone)]
pub struct DisplayOutput {
    pub output: WlOutput,
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
                self.update_output(output, info.logical_size);
            }

            OutputEvent::InfoUpdate(info) => {
                self.update_output(output, info.logical_size);
            }

            OutputEvent::Removed => {
                self.outputs.retain(|existing| existing.output != output);
            }

            _ => {}
        }
    }

    fn update_output(&mut self, output: WlOutput, logical_size: Option<(i32, i32)>) {
        let Some((width, height)) = logical_size else {
            return;
        };

        let (Ok(width), Ok(height)) = (u32::try_from(width), u32::try_from(height)) else {
            return;
        };

        if width == 0 || height == 0 {
            return;
        }

        let geometry = OutputGeometry {
            logical_width: width,
            logical_height: height,
        };

        if let Some(existing) = self
            .outputs
            .iter_mut()
            .find(|existing| existing.output == output)
        {
            existing.geometry = geometry;
            return;
        }

        self.outputs.push(DisplayOutput { output, geometry });
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_to_one_scale_is_valid() {
        let geometry = OutputGeometry {
            logical_width: 1920,
            logical_height: 1080,
        };

        assert_eq!(geometry.image_scale(1920, 1080), Some((1.0, 1.0)),);
    }

    #[test]
    fn integer_hidpi_scale_is_valid() {
        let geometry = OutputGeometry {
            logical_width: 1920,
            logical_height: 1080,
        };

        assert_eq!(geometry.image_scale(3840, 2160), Some((2.0, 2.0)),);
    }

    #[test]
    fn fractional_scale_with_rounding_is_valid() {
        let geometry = OutputGeometry {
            logical_width: 1707,
            logical_height: 960,
        };

        let scale = geometry
            .image_scale(2560, 1440)
            .expect("scale should be valid");

        assert!((scale.0 - 1.5).abs() < 0.01);
        assert!((scale.1 - 1.5).abs() < 0.01);
    }

    #[test]
    fn mismatched_aspect_ratio_is_rejected() {
        let geometry = OutputGeometry {
            logical_width: 1920,
            logical_height: 1080,
        };

        assert!(geometry.image_scale(1920, 1200).is_none());
    }

    #[test]
    fn zero_sized_geometry_is_rejected() {
        let geometry = OutputGeometry {
            logical_width: 0,
            logical_height: 1080,
        };

        assert!(geometry.image_scale(1920, 1080).is_none());
    }
}

```

### File: src/geometry.rs

```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewportRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl ViewportRect {
    pub fn from_points(ax: f32, ay: f32, bx: f32, by: f32) -> Self {
        Self {
            x: ax.min(bx),
            y: ay.min(by),
            width: (ax - bx).abs(),
            height: (ay - by).abs(),
        }
    }

    pub fn is_valid(self) -> bool {
        self.x.is_finite()
            && self.y.is_finite()
            && self.width.is_finite()
            && self.height.is_finite()
            && self.width >= 2.0
            && self.height >= 2.0
    }
}

/// A selection expressed relative to the captured image.
///
/// All coordinates are normalized to the range `0.0..=1.0`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Selection {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

impl Selection {
    pub fn from_viewport_rect(
        rect: ViewportRect,
        viewport_width: f32,
        viewport_height: f32,
    ) -> Option<Self> {
        if !rect.is_valid()
            || !viewport_width.is_finite()
            || !viewport_height.is_finite()
            || viewport_width <= 0.0
            || viewport_height <= 0.0
        {
            return None;
        }

        let left = (rect.x / viewport_width).clamp(0.0, 1.0);
        let top = (rect.y / viewport_height).clamp(0.0, 1.0);

        let right = ((rect.x + rect.width) / viewport_width).clamp(0.0, 1.0);
        let bottom = ((rect.y + rect.height) / viewport_height).clamp(0.0, 1.0);

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

    #[test]
    fn viewport_rect_handles_reverse_drag() {
        let rect = ViewportRect::from_points(80.0, 70.0, 20.0, 30.0);

        assert_eq!(
            rect,
            ViewportRect {
                x: 20.0,
                y: 30.0,
                width: 60.0,
                height: 40.0,
            }
        );
    }

    #[test]
    fn selection_is_normalized() {
        let rect = ViewportRect {
            x: 25.0,
            y: 20.0,
            width: 50.0,
            height: 40.0,
        };

        let selection = Selection::from_viewport_rect(rect, 100.0, 100.0).unwrap();

        assert_eq!(selection.left(), 0.25);
        assert_eq!(selection.top(), 0.20);
        assert_eq!(selection.right(), 0.75);
        assert_eq!(selection.bottom(), 0.60);
    }

    #[test]
    fn selection_is_clamped_to_viewport() {
        let rect = ViewportRect {
            x: -20.0,
            y: -10.0,
            width: 140.0,
            height: 120.0,
        };

        let selection = Selection::from_viewport_rect(rect, 100.0, 100.0).unwrap();

        assert_eq!(selection.left(), 0.0);
        assert_eq!(selection.top(), 0.0);
        assert_eq!(selection.right(), 1.0);
        assert_eq!(selection.bottom(), 1.0);
    }

    #[test]
    fn invalid_viewport_is_rejected() {
        let rect = ViewportRect {
            x: 10.0,
            y: 10.0,
            width: 20.0,
            height: 20.0,
        };

        assert!(Selection::from_viewport_rect(rect, 0.0, 100.0).is_none());
        assert!(Selection::from_viewport_rect(rect, 100.0, 0.0).is_none());
    }

    #[test]
    fn tiny_selection_is_rejected() {
        let rect = ViewportRect {
            x: 10.0,
            y: 10.0,
            width: 1.0,
            height: 1.0,
        };

        assert!(Selection::from_viewport_rect(rect, 100.0, 100.0).is_none());
    }
}

```

### File: src/i18n.rs

```
// SPDX-License-Identifier: MPL-2.0

//! Provides localization support for this crate.

use i18n_embed::{
    DefaultLocalizer, LanguageLoader, Localizer,
    fluent::{FluentLanguageLoader, fluent_language_loader},
    unic_langid::LanguageIdentifier,
};
use rust_embed::RustEmbed;
use std::sync::LazyLock;

/// Applies the requested language(s) to requested translations from the `fl!()` macro.
pub fn init(requested_languages: &[LanguageIdentifier]) {
    if let Err(why) = localizer().select(requested_languages) {
        eprintln!("error while loading fluent localizations: {why}");
    }
}

// Get the `Localizer` to be used for localizing this library.
#[must_use]
pub fn localizer() -> Box<dyn Localizer> {
    Box::from(DefaultLocalizer::new(&*LANGUAGE_LOADER, &Localizations))
}

#[derive(RustEmbed)]
#[folder = "i18n/"]
struct Localizations;

pub static LANGUAGE_LOADER: LazyLock<FluentLanguageLoader> = LazyLock::new(|| {
    let loader: FluentLanguageLoader = fluent_language_loader!();

    loader
        .load_fallback_language(&Localizations)
        .expect("Error while loading fallback language");

    loader
});

/// Request a localized string by ID from the i18n/ directory.
#[macro_export]
macro_rules! fl {
    ($message_id:literal) => {{
        i18n_embed_fl::fl!($crate::i18n::LANGUAGE_LOADER, $message_id)
    }};

    ($message_id:literal, $($args:expr),*) => {{
        i18n_embed_fl::fl!($crate::i18n::LANGUAGE_LOADER, $message_id, $($args), *)
    }};
}

```

### File: src/image_ops.rs

```
use image::{ExtendedColorType, ImageEncoder, codecs::png::PngEncoder};

use crate::{capture::CapturedImage, geometry::Selection};

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

    let source_stride = image.width as usize * 4;
    let crop_stride = width as usize * 4;

    let mut rgba = Vec::with_capacity(crop_stride * height as usize);

    for y in top..bottom {
        let start = y as usize * source_stride + left as usize * 4;

        let end = start + crop_stride;

        let row = image
            .rgba
            .get(start..end)
            .ok_or_else(|| "selection exceeded screenshot bounds".to_string())?;

        rgba.extend_from_slice(row);
    }

    let mut png = Vec::new();

    PngEncoder::new(&mut png)
        .write_image(&rgba, width, height, ExtendedColorType::Rgba8)
        .map_err(|error| format!("failed to encode screenshot: {error}"))?;

    Ok(png)
}

```

### File: src/main.rs

```
// SPDX-License-Identifier: MPL-2.0

mod app;
mod capture;
mod clipboard;
mod display;
mod geometry;
mod i18n;
mod image_ops;
mod overlay;
mod selection;

fn main() -> cosmic::iced::Result {
    let requested_languages = i18n_embed::DesktopLanguageRequester::requested_languages();

    i18n::init(&requested_languages);

    let settings = cosmic::app::Settings::default()
        .no_main_window(true)
        .exit_on_close(false);

    cosmic::app::run::<app::AppModel>(settings, ())
}

```

### File: src/overlay.rs

```
use cosmic::{
    Task,
    iced::{
        advanced::layout::Limits,
        core::window::Id,
        platform_specific::shell::commands::layer_surface::{
            Anchor, KeyboardInteractivity, destroy_layer_surface, get_layer_surface,
        },
        runtime::platform_specific::wayland::layer_surface::{
            IcedOutput, SctkLayerSurfaceSettings,
        },
    },
};

use wayland_client::protocol::wl_output::WlOutput;

pub fn open<Message: 'static>(id: Id, output: WlOutput) -> Task<cosmic::Action<Message>> {
    get_layer_surface(SctkLayerSurfaceSettings {
        id,
        keyboard_interactivity: KeyboardInteractivity::Exclusive,
        anchor: Anchor::all(),
        output: IcedOutput::Output(output),
        namespace: "snip-selection".into(),
        size: Some((None, None)),
        size_limits: Limits::NONE.min_width(1.0).min_height(1.0),
        exclusive_zone: -1,
        ..Default::default()
    })
}

pub fn close<Message: 'static>(id: Id) -> Task<cosmic::Action<Message>> {
    destroy_layer_surface(id)
}

```

### File: src/selection.rs

```
use cosmic::{
    Element,
    iced::{Color, Length, Point, Rectangle, Size, mouse, widget::canvas},
};

use crate::geometry::{Selection, ViewportRect};

#[derive(Default)]
pub struct State {
    start: Option<Point>,
    current: Option<Point>,
    dragging: bool,
}

pub struct SelectionOverlay;

impl canvas::Program<Selection, cosmic::Theme, cosmic::Renderer> for SelectionOverlay {
    type State = State;

    fn update(
        &self,
        state: &mut Self::State,
        event: &canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Selection>> {
        match event {
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let position = cursor.position_in(bounds)?;

                state.start = Some(position);
                state.current = Some(position);
                state.dragging = true;

                Some(canvas::Action::request_redraw().and_capture())
            }

            canvas::Event::Mouse(mouse::Event::CursorMoved { .. }) if state.dragging => {
                if let Some(position) = cursor.position_in(bounds) {
                    state.current = Some(position);
                }

                Some(canvas::Action::request_redraw().and_capture())
            }

            canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
                if state.dragging =>
            {
                if let Some(position) = cursor.position_in(bounds) {
                    state.current = Some(position);
                }

                state.dragging = false;

                let Some(start) = state.start else {
                    return Some(canvas::Action::capture());
                };

                let Some(current) = state.current else {
                    return Some(canvas::Action::capture());
                };

                let viewport_rect =
                    ViewportRect::from_points(start.x, start.y, current.x, current.y);

                if !viewport_rect.is_valid() {
                    state.start = None;
                    state.current = None;

                    return Some(canvas::Action::request_redraw().and_capture());
                }

                let Some(selection) =
                    Selection::from_viewport_rect(viewport_rect, bounds.width, bounds.height)
                else {
                    state.start = None;
                    state.current = None;

                    return Some(canvas::Action::request_redraw().and_capture());
                };

                Some(canvas::Action::publish(selection).and_capture())
            }

            _ => None,
        }
    }

    fn draw(
        &self,
        state: &Self::State,
        renderer: &cosmic::Renderer,
        _theme: &cosmic::Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry<cosmic::Renderer>> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());

        let mut dim = Color::BLACK;
        dim.a = 0.48;

        match (state.start, state.current) {
            (Some(start), Some(current)) => {
                let viewport = ViewportRect::from_points(start.x, start.y, current.x, current.y);

                draw_dimmed_outside(&mut frame, bounds.size(), viewport, dim);

                if viewport.is_valid() {
                    draw_selection(&mut frame, viewport);
                }
            }

            _ => {
                fill_rect(
                    &mut frame,
                    Rectangle::new(Point::ORIGIN, bounds.size()),
                    dim,
                );
            }
        }

        vec![frame.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        _state: &Self::State,
        _bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        mouse::Interaction::Crosshair
    }
}

fn draw_dimmed_outside(
    frame: &mut canvas::Frame,
    canvas_size: Size,
    viewport: ViewportRect,
    color: Color,
) {
    let x = viewport.x;
    let y = viewport.y;
    let right = viewport.x + viewport.width;
    let bottom = viewport.y + viewport.height;

    // Top
    fill_rect(
        frame,
        Rectangle::new(Point::ORIGIN, Size::new(canvas_size.width, y)),
        color,
    );

    // Bottom
    fill_rect(
        frame,
        Rectangle::new(
            Point::new(0.0, bottom),
            Size::new(canvas_size.width, (canvas_size.height - bottom).max(0.0)),
        ),
        color,
    );

    // Left
    fill_rect(
        frame,
        Rectangle::new(Point::new(0.0, y), Size::new(x, viewport.height)),
        color,
    );

    // Right
    fill_rect(
        frame,
        Rectangle::new(
            Point::new(right, y),
            Size::new((canvas_size.width - right).max(0.0), viewport.height),
        ),
        color,
    );
}

fn draw_selection(frame: &mut canvas::Frame, viewport: ViewportRect) {
    let rectangle = Rectangle::new(
        Point::new(viewport.x, viewport.y),
        Size::new(viewport.width, viewport.height),
    );

    let path = canvas::Path::rectangle(rectangle.position(), rectangle.size());

    frame.stroke(
        &path,
        canvas::Stroke::default()
            .with_color(Color::WHITE)
            .with_width(2.0),
    );

    draw_dimensions(frame, viewport);
}

fn draw_dimensions(frame: &mut canvas::Frame, viewport: ViewportRect) {
    let text = format!(
        "{} × {}",
        viewport.width.round() as u32,
        viewport.height.round() as u32,
    );

    let label_width = 96.0;
    let label_height = 26.0;

    let label_x = viewport.x;

    let label_y = if viewport.y >= label_height + 8.0 {
        viewport.y - label_height - 6.0
    } else {
        viewport.y + viewport.height + 6.0
    };

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

pub fn view() -> Element<'static, Selection> {
    canvas::Canvas::new(SelectionOverlay)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

```