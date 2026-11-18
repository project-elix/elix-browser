pub mod history;
#[cfg(any(windows, test))]
pub(crate) mod navigation;
#[cfg(windows)]
pub mod windows;
pub use history::Download;
use std::path::PathBuf;
use tauri::{Emitter, Manager};

/// Attach a recovery hint only when an attachment endpoint actually rejected the request.
pub fn link_error(address: &str, status: Option<i32>) -> Option<String> {
    let status = status?;
    if !matches!(status, 401 | 403 | 410 | 618) {
        return None;
    }
    let url = url::Url::parse(address).ok()?;
    let github_asset = url.host_str() == Some("release-assets.githubusercontent.com");
    let google_asset = url.host_str() == Some("drive.usercontent.google.com") && url.path() == "/download";
    let attachment = url.query_pairs().any(|(key, value)| {
        matches!(key.as_ref(), "rscd" | "response-content-disposition")
            && value.split(';').next().is_some_and(|v| v.trim().eq_ignore_ascii_case("attachment"))
    });
    if !(github_asset || google_asset || attachment) || (status == 618 && !github_asset) {
        return None;
    }
    Some(format!("Nettstedet avviste nedlastingen (HTTP {status}). Åpne en ny lenke fra nettstedet, og kontroller at du er logget inn hvis filen krever tilgang."))
}

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
