// bridge.rs — shared helpers for spawning the ferr sidecar
//
// The actual command implementations live in commands.rs.
// This module provides utilities used across commands.

use tauri_plugin_shell::process::CommandEvent;

/// Drain a command event stream, emitting Tauri events for each line.
/// Returns Ok(exit_code) on success or Err on spawn failure.
pub async fn drain(
    app: &tauri::AppHandle,
    mut rx: tokio::sync::mpsc::Receiver<CommandEvent>,
) -> Result<i32, String> {
    use tauri::Emitter;

    let mut exit_code = 0;
    while let Some(event) = rx.recv().await {
        match event {
            CommandEvent::Stdout(bytes) => {
                let line = String::from_utf8_lossy(&bytes).to_string();
                app.emit("ferr-progress", &line).ok();
            }
            CommandEvent::Stderr(bytes) => {
                let line = String::from_utf8_lossy(&bytes).to_string();
                app.emit("ferr-error", &line).ok();
            }
            CommandEvent::Terminated(status) => {
                exit_code = status.code.unwrap_or(-1);
                app.emit("ferr-complete", exit_code).ok();
            }
            _ => {}
        }
    }
    Ok(exit_code)
}
