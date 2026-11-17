pub mod history;
#[cfg(windows)]
pub mod windows;
pub use history::Download;
use std::path::PathBuf;
use tauri::{Emitter, Manager};

pub fn persist(app: &tauri::AppHandle) {
    let state = app.state::<crate::AppState>();
    let downloads = state.downloads.lock().unwrap();
    if history::save(&state.download_root, &downloads).is_err() {
        state.logs.add("error", "download_history_write_failed");
    }
}

pub fn list(app: &tauri::AppHandle) -> Vec<Download> {
    let state = app.state::<crate::AppState>();
    let profile = state.profile.lock().unwrap().clone();
    let result = state
        .downloads
        .lock()
        .unwrap()
        .iter()
        .filter(|d| d.profile == profile)
        .cloned()
        .collect();
    result
}

pub async fn action(app: &tauri::AppHandle, id: &str, operation: &str) -> Result<(), String> {
    let state = app.state::<crate::AppState>();
    let profile = state.profile.lock().unwrap().clone();
    let download = state
        .downloads
        .lock()
        .unwrap()
        .iter()
        .find(|d| d.id == id && d.profile == profile)
        .cloned()
        .ok_or("Ukjent nedlasting")?;
    match operation {
        "pause" if download.status == "downloading" => {},
        "resume" if download.can_resume => {},
        "cancel" if matches!(download.status.as_str(), "downloading" | "paused") || download.can_resume => {},
        "pause" | "resume" | "cancel" => return Err("Denne nedlastingen kan ikke endres nå. Start den på nytt fra nettstedet etter omstart av ELIX.".into()),
        _ => return Err("Ukjent nedlastingshandling".into()),
    }
    #[cfg(windows)]
    {
        windows::action(app, download.path, operation.to_owned()).await
    }
    #[cfg(not(windows))]
    {
        Err("Krever Windows/WebView2".into())
    }
}

/// Called when leaving a private session; downloaded files remain on disk.
pub fn clear_private(app: &tauri::AppHandle) {
    app.state::<crate::AppState>()
        .downloads
        .lock()
        .unwrap()
        .retain(|d| !d.private);
    let _ = app.emit_to("shell", "downloads-changed", ());
}

pub fn destination(
    root: PathBuf,
    suggested: &std::path::Path,
) -> Result<(String, PathBuf), std::io::Error> {
    let id = uuid::Uuid::new_v4().to_string();
    let dir = root.join("ELIX").join(&id);
    std::fs::create_dir_all(&dir)?;
    let name = suggested
        .file_name()
        .and_then(|v| v.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("download");
    Ok((id, dir.join(name)))
}
