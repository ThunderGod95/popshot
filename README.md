# Popshot

Screenshot utility for CosmicDE.

## Installation

A [justfile](./justfile) is included by default for the [casey/just][just] command runner.

- `just` builds the application with the default `just build-release` recipe
- `just run` builds and runs the application
- `just install` installs the project into the system
- `just vendor` creates a vendored tarball
- `just build-vendored` compiles with vendored dependencies from that tarball
- `just check` runs clippy on the project to check for linter warnings
- `just check-json` can be used by IDEs that support LSP

## Translators

[Fluent][fluent] is used for localization of the software. Fluent's translation files are found in the [i18n directory](./i18n). New translations may copy the [English (en) localization](./i18n/en) of the project, rename `en` to the desired [ISO 639-1 language code][iso-codes], and then translations can be provided for each [message identifier][fluent-guide]. If no translation is necessary, the message may be omitted.

## Packaging

If packaging for a Linux distribution, vendor dependencies locally with the `vendor` rule, and build with the vendored sources using the `build-vendored` rule. When installing files, use the `rootdir` and `prefix` variables to change installation paths.

```sh
just vendor
just build-vendored
just rootdir=debian/popshot prefix=/usr install
```

It is recommended to build a source tarball with the vendored dependencies, which can typically be done by running `just vendor` on the host system before it enters the build environment.

## Developers

Developers should install [rustup][rustup] and configure their editor to use [rust-analyzer][rust-analyzer]. To improve compilation times, disable LTO in the release profile, install the [mold][mold] linker, and configure [sccache][sccache] for use with Rust. The [mold][mold] linker will only improve link times if LTO is disabled.

[fluent]: https://projectfluent.org/
[fluent-guide]: https://projectfluent.org/fluent/guide/hello.html
[iso-codes]: https://en.wikipedia.org/wiki/List_of_ISO_639-1_codes
[just]: https://github.com/casey/just
[rustup]: https://rustup.rs/
[rust-analyzer]: https://rust-analyzer.github.io/
[mold]: https://github.com/rui314/mold
[sccache]: https://github.com/mozilla/sccache

## Snipping

Run `cargo run --locked` in a COSMIC Wayland session with one monitor. Popshot
requests a desktop snapshot through the screenshot portal before opening its overlay.
The portal may ask for permission, depending on your desktop settings.

- Drag in any direction to capture a rectangle; release to finish. Tiny clicks are ignored.
- Click **Fullscreen** or press **F** to capture the complete snapshot.
- Press **Esc** to cancel. The toolbar disappears while dragging and is never in the image.
- The preview automatically copies the PNG. **Ctrl+C** retries copying; **Ctrl+S** opens
  Save As. Copy/save errors leave the image available in the preview.

Clipboard copying requires `wl-copy` (the `wl-clipboard` package). Saving uses the
native file chooser portal and works independently of clipboard support. Files are PNG.
Launch Popshot again for another capture; bind `popshot` to your preferred desktop
shortcut for a snipping-tool workflow.

## Capture architecture

`capture.rs` owns portal acquisition, PNG cropping and saving; `clipboard.rs` owns
clipboard delivery. `app.rs` coordinates capture → selection → preview. A single
layer-shell overlay in `overlay.rs` covers the active output, with exclusive keyboard
input so Escape works even when another app was focused.

`output_selection.rs` contains the capture modes and rectangle input/rendering.
Selections use normalized image bounds so logical desktop coordinates map to the
original pixel resolution, including fractional scaling. The toolbar uses the mode's
availability flag; Window and Freehand are deliberately disabled until implemented.

To add window capture, obtain window bounds from a compositor-supported backend and
feed normalized bounds into the existing crop/preview path. The screenshot portal
alone does not supply those bounds. To add freehand capture, extend the selection
payload with a normalized polygon, crop its bounding box, and mask pixels outside the
polygon to transparent before the same preview/copy/save steps. No changes to output
delivery are needed. Multi-monitor mapping is outside the current scope.

Validation: `cargo test --locked` and `cargo clippy --locked --all-targets -- -D warnings`.
Manual checks: drag in all four directions, cancel, fullscreen, test at 100% and
fractional scaling, copy into another app, save/cancel/overwrite a PNG, and retry after
clipboard or save errors.
