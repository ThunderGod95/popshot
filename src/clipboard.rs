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
