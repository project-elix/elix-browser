//! The only cross-platform Tauri/Wry adapter. Replacing it is the seam for a future CEF engine.
use crate::{
    browser::{self, Bounds, Tab},
    security, AppState,
};
use tauri::webview::{NewWindowResponse, PageLoadEvent, PermissionResponse, WebviewBuilder};
#[cfg(not(windows))]
use tauri::{webview::DownloadEvent, Emitter};
use tauri::{
    AppHandle, LogicalPosition, LogicalSize, Manager, Position, Rect, Size, Webview, WebviewUrl,
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
    let private = tab.private;
    let directory = if private {
        super::private::directory(&state.root)?
    } else {
        crate::profiles::directory(&state.root, &profile)?
    };
    let settings = state.store.lock().unwrap().settings();
    #[cfg(not(windows))]
    let profile_download = profile.clone();
    let nav_app = app.clone();
    let nav_id = tab.id.clone();
    let popup_app = app.clone();
    let profile_load = profile.clone();
    let builder = WebviewBuilder::new(
        &tab.id,
        WebviewUrl::External("about:blank".parse().map_err(|_| "Ugyldig URL")?),
    )
    .data_directory(directory)
    .incognito(private)
    .browser_extensions_enabled(!private)
    .general_autofill_enabled(!private && settings.autofill)
    .devtools(settings.developer)
    .disable_drag_drop_handler()
    .zoom_hotkeys_enabled(true)
    .on_navigation(move |url| {
        if url.as_str() == "about:blank" {
            return true;
        }
        let allowed = security::remote_allowed(url);
        if allowed {
            #[cfg(windows)]
            crate::security::permissions::cancel(&nav_app, &nav_id);
            let state = nav_app.state::<AppState>();
            if let Some(t) = state
                .session
                .lock()
                .unwrap()
                .tabs
                .iter_mut()
                .find(|t| t.id == nav_id)
            {
                if t.url != url.as_str() {
                    t.favicon = None;
                }
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
                if browser::create_with_mode(app.clone(), url.to_string(), private)
                    .await
                    .is_err()
                {
                    app.state::<AppState>().logs.add("warn", "popup_failed");
                }
            });
        }
        NewWindowResponse::Deny
    })
    // The native permission handler defers camera/microphone decisions to the shell.
    // Everything without an explicit native handler remains denied.
    .on_permission_request(|_, _| PermissionResponse::Deny)
    .on_document_title_changed(|view, title| {
        if view.url().is_ok_and(|url| url.as_str() == "about:blank") {
            return;
        }
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
        if payload.url().as_str() == "about:blank" {
            return;
        }
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
            if !private && security::remote_allowed(payload.url()) {
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
    });
    // Windows has a single native DownloadStarting owner. Registering Wry's
    // callback, even a no-op, retrieves separate DownloadOperation wrappers and
    // installs its own state handler / destination handling before ours.
    #[cfg(not(windows))]
    let builder = builder.on_download(move |view, event| {
        let app = view.app_handle();
        let state = app.state::<AppState>();
        match event {
            DownloadEvent::Requested {
                destination, url, ..
            } => {
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
                    .push(crate::downloads::Download::new(
                        id,
                        path,
                        profile_download.clone(),
                        private,
                        url.to_string(),
                        view.label().to_string(),
                    ));
                let _ = crate::downloads::persist(app);
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
    #[cfg(feature = "smoke-tests")]
    // Windows sharing a profile must use identical WebView2 environment options.
    let builder =
        if std::env::args().any(|arg| arg == "--self-test" || arg == "--elix-smoke-window") {
            builder.additional_browser_args("--use-fake-device-for-media-stream")
        } else {
            builder
        };
    let window = app.get_window("main").ok_or("Fant ikke hovedvinduet")?;
    let view = window
        .add_child(
            builder,
            LogicalPosition::new(64., 112.),
            LogicalSize::new(1000., 600.),
        )
        .map_err(|e| e.to_string())?;
    // A dormant page can also be recreated as the second split pane, without
    // activating it. Keep the runtime flag tied to this view in every wake path.
    if let Some(current) = state
        .session
        .lock()
        .unwrap()
        .tabs
        .iter_mut()
        .find(|current| current.id == tab.id)
    {
        current.suspended = false;
        current.crashed = false;
        current.loading = true;
        current.started = Some(std::time::Instant::now());
        current.http_status = None;
        current.navigation_error = None;
    }
    #[cfg(windows)]
    {
        super::windows::attach(&view)?;
        super::favicon::attach(&view)?;
        crate::security::permissions::attach(&view)?;
    }
    super::extensions::configure(
        &view,
        settings.password_autosave,
        settings.autofill,
        private,
    )?;
    if private {
        super::private::launch(&view, &tab.url)?;
    } else {
        view.navigate(tab.url.parse().map_err(|_| "Ugyldig URL")?)
            .map_err(|e| e.to_string())?;
    }
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
