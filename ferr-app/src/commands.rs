// commands.rs — Tauri commands exposed to the JS frontend
//
// Every function here is a thin wrapper: build CLI args → spawn sidecar → stream events.
// No file I/O, hashing, or data processing happens here.

use tauri::Emitter;
use tauri_plugin_shell::ShellExt;

use crate::bridge::drain;
use crate::volume::list_volumes;

// ── Folder / file pickers ─────────────────────────────────────────────────────

#[tauri::command]
pub async fn pick_folder(app: tauri::AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let result = app.dialog().file().blocking_pick_folder();
    Ok(result.map(|p| p.to_string()))
}

#[tauri::command]
pub async fn pick_file(
    app: tauri::AppHandle,
    extensions: Vec<String>,
) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let mut builder = app.dialog().file();
    if !extensions.is_empty() {
        builder = builder.add_filter(
            "Files",
            &extensions.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
        );
    }
    let result = builder.blocking_pick_file();
    Ok(result.map(|p| p.to_string()))
}

#[tauri::command]
pub async fn pick_save_location(
    app: tauri::AppHandle,
    default_name: String,
) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let result = app
        .dialog()
        .file()
        .set_file_name(&default_name)
        .blocking_save_file();
    Ok(result.map(|p| p.to_string()))
}

// ── Copy ──────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn run_copy(
    app: tauri::AppHandle,
    source: String,
    destinations: Vec<String>,
    args: Vec<String>,
) -> Result<(), String> {
    let cmd_args = build_copy_args(&source, &destinations, &args, false);
    spawn_and_drain(&app, cmd_args).await
}

#[tauri::command]
pub async fn run_copy_preview(
    app: tauri::AppHandle,
    source: String,
    destinations: Vec<String>,
    args: Vec<String>,
) -> Result<(), String> {
    let mut extra = args.clone();
    extra.push("--dry-run".into());
    let cmd_args = build_copy_args(&source, &destinations, &extra, false);
    spawn_and_drain(&app, cmd_args).await
}

fn build_copy_args(
    source: &str,
    destinations: &[String],
    extra: &[String],
    _resume: bool,
) -> Vec<String> {
    let mut args = vec!["copy".to_string(), source.to_string()];
    if let Some(d) = destinations.first() {
        args.push(d.clone());
    }
    if destinations.len() > 1 {
        args.extend(["--dest2".into(), destinations[1].clone()]);
    }
    if destinations.len() > 2 {
        args.extend(["--dest3".into(), destinations[2].clone()]);
    }
    args.extend_from_slice(extra);
    args.extend(["--progress-format".into(), "machine".into()]);
    args
}

// ── Watch ─────────────────────────────────────────────────────────────────────

// Active watch child handle (single global per app instance)
static WATCH_CHILD: std::sync::Mutex<Option<tauri_plugin_shell::process::CommandChild>> =
    std::sync::Mutex::new(None);

#[tauri::command]
pub async fn run_watch_start(
    app: tauri::AppHandle,
    folder: String,
    destinations: Vec<String>,
    args: Vec<String>,
) -> Result<(), String> {
    let mut cmd_args = vec!["watch".to_string(), folder];
    for dest in &destinations {
        cmd_args.extend(["--dest".into(), dest.clone()]);
    }
    cmd_args.extend_from_slice(&args);
    cmd_args.extend(["--progress-format".into(), "machine".into()]);

    let sidecar = app
        .shell()
        .sidecar("ferr-cli")
        .map_err(|e| e.to_string())?
        .args(&cmd_args);

    let (rx, child) = sidecar.spawn().map_err(|e| format!("Failed to start ferr: {e}"))?;
    *WATCH_CHILD.lock().unwrap() = Some(child);

    let app2 = app.clone();
    tokio::spawn(async move {
        drain(&app2, rx).await.ok();
    });

    app.emit("ferr-watch-started", ()).ok();
    Ok(())
}

#[tauri::command]
pub async fn run_watch_stop(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(child) = WATCH_CHILD.lock().unwrap().take() {
        child.kill().map_err(|e| e.to_string())?;
    }
    app.emit("ferr-watch-stopped", ()).ok();
    Ok(())
}

// ── Health ────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn run_scan(
    app: tauri::AppHandle,
    folder: String,
    since_date: Option<String>,
) -> Result<(), String> {
    let mut args = vec!["scan".to_string(), folder];
    if let Some(date) = since_date {
        args.extend(["--since".into(), date]);
    }
    args.extend(["--progress-format".into(), "machine".into()]);
    spawn_and_drain(&app, args).await
}

#[tauri::command]
pub async fn run_verify(
    app: tauri::AppHandle,
    source_or_manifest: String,
    dest: String,
) -> Result<(), String> {
    let args = vec![
        "verify".to_string(),
        source_or_manifest,
        dest,
        "--progress-format".into(),
        "machine".into(),
    ];
    spawn_and_drain(&app, args).await
}

#[tauri::command]
pub async fn run_repair(app: tauri::AppHandle, folder: String) -> Result<(), String> {
    let args = vec!["repair".to_string(), folder];
    spawn_and_drain(&app, args).await
}

#[tauri::command]
pub async fn run_cert_create(
    app: tauri::AppHandle,
    folder: String,
    output_path: String,
) -> Result<(), String> {
    let args = vec![
        "cert".to_string(),
        "create".to_string(),
        folder,
        "--output".into(),
        output_path,
    ];
    spawn_and_drain(&app, args).await
}

#[tauri::command]
pub async fn run_cert_verify(
    app: tauri::AppHandle,
    cert_path: String,
    folder: String,
) -> Result<(), String> {
    let args = vec!["cert".to_string(), "verify".to_string(), cert_path, folder];
    spawn_and_drain(&app, args).await
}

// ── Export / report ───────────────────────────────────────────────────────────

#[tauri::command]
pub async fn run_export(
    app: tauri::AppHandle,
    manifest_path: String,
    format: String,
    output_path: String,
) -> Result<(), String> {
    let args = vec![
        "export".to_string(),
        manifest_path,
        "--format".into(),
        format,
        "--output".into(),
        output_path,
    ];
    spawn_and_drain(&app, args).await
}

#[tauri::command]
pub async fn run_report(
    app: tauri::AppHandle,
    manifest_path: String,
    output_path: String,
) -> Result<(), String> {
    let args = vec![
        "report".to_string(),
        manifest_path,
        "--output".into(),
        output_path,
    ];
    spawn_and_drain(&app, args).await
}

// ── History ───────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_history(app: tauri::AppHandle) -> Result<String, String> {
    run_ferr_json(&app, &["history", "list", "--json"]).await
}

#[tauri::command]
pub async fn search_history(app: tauri::AppHandle, query: String) -> Result<String, String> {
    run_ferr_json(&app, &["history", "find", &query, "--json"]).await
}

// ── Profiles ──────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_profiles(app: tauri::AppHandle) -> Result<String, String> {
    run_ferr_json(&app, &["profile", "list", "--json"]).await
}

#[tauri::command]
pub async fn save_profile(app: tauri::AppHandle, name: String) -> Result<(), String> {
    let args = vec!["profile".to_string(), "save".to_string(), name];
    spawn_and_drain(&app, args).await
}

// ── Volumes ───────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_volumes() -> Vec<crate::volume::VolumeInfo> {
    list_volumes()
}

// ── Application ───────────────────────────────────────────────────────────────

#[tauri::command]
pub fn quit_app(app: tauri::AppHandle) {
    app.exit(0);
}

// ── Helpers ───────────────────────────────────────────────────────────────────

async fn spawn_and_drain(app: &tauri::AppHandle, args: Vec<String>) -> Result<(), String> {
    let sidecar = app
        .shell()
        .sidecar("ferr-cli")
        .map_err(|e| e.to_string())?
        .args(&args);

    let (rx, _child) = sidecar.spawn().map_err(|e| format!("Failed to start ferr: {e}"))?;
    drain(app, rx).await?;
    Ok(())
}

/// Run ferr, collect all stdout, return as a single String (for JSON commands).
async fn run_ferr_json(app: &tauri::AppHandle, args: &[&str]) -> Result<String, String> {
    let sidecar = app
        .shell()
        .sidecar("ferr-cli")
        .map_err(|e| e.to_string())?
        .args(args);

    let output = sidecar
        .output()
        .await
        .map_err(|e| format!("Failed to run ferr: {e}"))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}
