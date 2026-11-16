//! The only cross-platform Tauri/Wry adapter. Replacing it is the seam for a future CEF engine.
use crate::{
    browser::{self, Bounds, Tab},
    security, AppState,
};
use tauri::webview::{
    DownloadEvent, NewWindowResponse, PageLoadEvent, PermissionResponse, WebviewBuilder,
};
use tauri::{
    AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, Position, Rect, Size, Webview,
    WebviewUrl,
};
pub fn bounds(view: &Webview, b: Bounds) -> Result<(), String> {
    if ![b.x, b.y, b.width, b.height].iter().all(|n| n.is_finite())
        || b.x < 0.
        || b.y < 0.
        || b.width < 1.
        || b.height < 1.
        || b.width > 16000.
        || b.height > 16000.
    {
        return Err("Ugyldig visningsområde".into());
    }
    view.set_bounds(Rect {
        position: Position::Logical(LogicalPosition::new(b.x, b.y)),
        size: Size::Logical(LogicalSize::new(b.width, b.height)),
    })
    .map_err(|e| e.to_string())
}
pub fn create(app: &AppHandle, tab: &Tab) -> Result<(), String> {
    let state = app.state::<AppState>();
    let profile = state.profile.lock().unwrap().clone();
    let directory = crate::profiles::directory(&state.root, &profile)?;
    let dev = state.store.lock().unwrap().settings().developer;
    let nav_app = app.clone();
    let nav_id = tab.id.clone();
    let popup_app = app.clone();
    let profile_load = profile.clone();
    let builder = WebviewBuilder::new(
        &tab.id,
        WebviewUrl::External(tab.url.parse().map_err(|_| "Ugyldig URL")?),
    )
    .data_directory(directory)
    .devtools(dev)
    .disable_drag_drop_handler()
    .zoom_hotkeys_enabled(true)
    .on_navigation(move |url| {
        let allowed = security::remote_allowed(url);
        if allowed {
            let state = nav_app.state::<AppState>();
            if let Some(t) = state
                .session
                .lock()
                .unwrap()
                .tabs
                .iter_mut()
                .find(|t| t.id == nav_id)
            {
                t.url = url.to_string();
            }
            browser::emit(&nav_app);
        }
        allowed
    })
    .on_new_window(move |url, _| {
        if security::remote_allowed(&url) {
            let app = popup_app.clone();
            tauri::async_runtime::spawn(async move {
                if browser::create(app.clone(), url.to_string()).await.is_err() {
                    app.state::<AppState>().logs.add("warn", "popup_failed");
                }
            });
        }
        NewWindowResponse::Deny
    })
    .on_permission_request(|view, _| {
        let app = view.app_handle();
        app.state::<AppState>()
            .logs
            .add("warn", "site_permission_denied");
        let _ = app.emit_to(
            "shell",
            "notice",
            "Nettstedstillatelser som kamera og mikrofon er ikke støttet i 0.1.0.",
        );
        PermissionResponse::Deny
    })
    .on_document_title_changed(|view, title| {
        let app = view.app_handle();
        let state = app.state::<AppState>();
        if let Some(t) = state
            .session
            .lock()
            .unwrap()
            .tabs
            .iter_mut()
            .find(|t| t.id == view.label())
        {
            t.title = title.chars().take(250).collect();
        }
        browser::emit(app);
        let _ = browser::persist(app);
    })
    .on_page_load(move |view, payload| {
        let app = view.app_handle();
        let state = app.state::<AppState>();
        let mut visit = None;
        {
            let mut session = state.session.lock().unwrap();
            if let Some(t) = session.tabs.iter_mut().find(|t| t.id == view.label()) {
                t.url = payload.url().to_string();
                match payload.event() {
                    PageLoadEvent::Started => {
                        t.loading = true;
                        t.started = Some(std::time::Instant::now());
                        t.http_status = None;
                        t.navigation_error = None;
                    }
                    PageLoadEvent::Finished => {
                        t.loading = false;
                        t.load_ms = t.started.take().map(|i| i.elapsed().as_millis() as u64);
                        visit = Some((t.url.clone(), t.title.clone()));
                    }
                }
            }
        }
        if let Some((url, title)) = visit {
            if security::remote_allowed(payload.url()) {
                if state
                    .store
                    .lock()
                    .unwrap()
                    .visit(&profile_load, &url, &title)
                    .is_err()
                {
                    state.logs.add("error", "history_write_failed");
                }
            }
        }
        browser::emit(app);
        let _ = browser::persist(app);
    })
    .on_download(|view, event| {
        let app = view.app_handle();
        let state = app.state::<AppState>();
        match event {
            DownloadEvent::Requested { destination, .. } => {
                let Ok(root) = app.path().download_dir() else {
                    return false;
                };
                let Ok((id, path)) = crate::downloads::destination(root, destination) else {
                    return false;
                };
                *destination = path.clone();
                state
                    .downloads
                    .lock()
                    .unwrap()
                    .push(crate::downloads::Download {
                        id,
                        name: path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into(),
                        path,
                        status: "downloading".into(),
                        time: chrono::Utc::now().to_rfc3339(),
                    });
            }
            DownloadEvent::Finished { path, success, .. } => {
                if let Some(path) = path {
                    if let Some(d) = state
                        .downloads
                        .lock()
                        .unwrap()
                        .iter_mut()
                        .find(|d| d.path == path)
                    {
                        d.status = if success { "complete" } else { "failed" }.into();
                    }
                }
                state.logs.add(
                    if success { "info" } else { "warn" },
                    if success {
                        "download_complete"
                    } else {
                        "download_failed"
                    },
                );
            }
            _ => {}
        }
        let _ = app.emit_to("shell", "downloads-changed", ());
        true
    });
    let window = app.get_window("main").ok_or("Fant ikke hovedvinduet")?;
    let view = window
        .add_child(
            builder,
            LogicalPosition::new(64., 112.),
            LogicalSize::new(1000., 600.),
        )
        .map_err(|e| e.to_string())?;
    #[cfg(windows)]
    super::windows::attach(&view)?;
    Ok(())
}
pub async fn action(view: Webview, action: String, enabled: bool) -> Result<(), String> {
    #[cfg(windows)]
    {
        super::windows::action(view, action, enabled).await
    }
    #[cfg(not(windows))]
    {
        let _ = enabled;
        match action.as_str() {
            "reload" => view.reload().map_err(|e| e.to_string()),
            "devtools" => {
                view.open_devtools();
                Ok(())
            }
            _ => Err("Krever Windows/WebView2".into()),
        }
    }
}
