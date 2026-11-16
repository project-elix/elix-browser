//! InPrivate controllers use a disposable data directory, separate from every normal profile.
use crate::{browser, AppState};
use std::{
    path::{Path, PathBuf},
    sync::Mutex,
};
use tauri::{AppHandle, Manager, Webview};

static DIRECTORY: Mutex<Option<PathBuf>> = Mutex::new(None);

pub fn directory(root: &Path) -> Result<PathBuf, String> {
    let mut current = DIRECTORY.lock().unwrap();
    if let Some(path) = current.as_ref() {
        return Ok(path.clone());
    }
    let path = root
        .join("private-sessions")
        .join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    *current = Some(path.clone());
    Ok(path)
}

pub fn cleanup_if_empty(app: &AppHandle) {
    if !app
        .state::<AppState>()
        .session
        .lock()
        .unwrap()
        .tabs
        .iter()
        .any(|t| t.private)
    {
        crate::downloads::clear_private(app);
        cleanup();
    }
}

/// Call after closing all private views, including on profile switch and app exit.
pub fn cleanup() {
    if let Some(path) = DIRECTORY.lock().unwrap().take() {
        // WebView2 may release its directory just after controller.Close returns.
        // A fresh private session always uses a new directory, even if removal fails.
        std::thread::spawn(move || {
            for _ in 0..20 {
                if std::fs::remove_dir_all(&path).is_ok() || !path.exists() {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(250));
            }
        });
    }
}

/// Verify the actual native profile before navigating away from the blank initial page.
/// Older WebView2 runtimes can silently ignore .incognito(true); fail closed instead.
#[cfg(windows)]
pub fn launch(view: &Webview, url: &str) -> Result<(), String> {
    use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2_13;
    use windows::core::{Interface, BOOL, HSTRING};
    let app = view.app_handle().clone();
    let id = view.label().to_string();
    let url = url.to_string();
    view.with_webview(move |platform| {
        let result = (|| -> windows::core::Result<()> {
            unsafe {
                let core = platform.controller().CoreWebView2()?;
                let profile = core.cast::<ICoreWebView2_13>()?.Profile()?;
                let mut private = BOOL(0);
                profile.IsInPrivateModeEnabled(&mut private)?;
                if !private.as_bool() {
                    return Err(windows::core::Error::from_hresult(windows::core::HRESULT(0x80004005u32 as i32)));
                }
                core.Navigate(&HSTRING::from(url))?;
            }
            Ok(())
        })();
        if result.is_err() {
            browser::failed(&app, &id, "Privatmodus kunne ikke startes. Oppdater Microsoft Edge WebView2 Runtime og prøv igjen.");
        }
    }).map_err(|e| e.to_string())
}

#[cfg(not(windows))]
pub fn launch(view: &Webview, url: &str) -> Result<(), String> {
    view.navigate(url.parse().map_err(|_| "Ugyldig URL")?)
        .map_err(|e| e.to_string())
}

#[cfg(all(windows, feature = "smoke-tests"))]
pub async fn is_in_private(view: &Webview) -> Result<bool, String> {
    use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2_13;
    use windows::core::{Interface, BOOL};
    let (tx, rx) = std::sync::mpsc::channel();
    view.with_webview(move |platform| {
        let result = (|| -> windows::core::Result<bool> {
            unsafe {
                let profile = platform
                    .controller()
                    .CoreWebView2()?
                    .cast::<ICoreWebView2_13>()?
                    .Profile()?;
                let mut private = BOOL(0);
                profile.IsInPrivateModeEnabled(&mut private)?;
                Ok(private.as_bool())
            }
        })()
        .map_err(|e| e.to_string());
        let _ = tx.send(result);
    })
    .map_err(|e| e.to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        rx.recv_timeout(std::time::Duration::from_secs(10))
            .map_err(|_| "Privatmodus svarte ikke".to_string())?
    })
    .await
    .map_err(|e| e.to_string())?
}
