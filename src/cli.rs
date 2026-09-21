use crate::output_selection::CaptureMode;

pub const HELP: &str = "Usage: popshot [--area | --freehand | --fullscreen]

Without a mode, open the snipping toolbar.
  --area        Drag to capture an area (Esc cancels)
  --freehand    Draw a freehand area (Esc cancels)
  --fullscreen  Capture the desktop immediately
  -h, --help    Show this help";

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub service: bool,
    pub mode: Option<CaptureMode>,
}

pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Option<Options>, String> {
    let mut options = Options::default();

    for arg in args {
        let mode = match arg.as_str() {
            "-h" | "--help" => return Ok(None),

            "--gapplication-service" if !options.service && options.mode.is_none() => {
                options.service = true;
                continue;
            }

            "--area" => CaptureMode::Rectangle,

            "--freehand" => CaptureMode::Freehand,

            "--fullscreen" => CaptureMode::Fullscreen,

            _ => return Err(format!("Unknown or repeated option: {arg}")),
        };

        if options.service || options.mode.is_some() {
            return Err("Choose one capture mode; service startup cannot include a mode".into());
        }

        options.mode = Some(mode);
    }

    Ok(Some(options))
}
