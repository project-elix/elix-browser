use crate::{browser, AppState};
use tauri::{Emitter, Manager, Webview};
use webview2_com::{
    AcceleratorKeyPressedEventHandler, DownloadStartingEventHandler,
    Microsoft::Web::WebView2::Win32::*, NavigationCompletedEventHandler, ProcessFailedEventHandler,
    StateChangedEventHandler,
};
use windows::{
    core::{Interface, PWSTR},
    Win32::UI::Input::KeyboardAndMouse::{GetKeyState, VK_CONTROL, VK_SHIFT},
};

pub fn attach(view: &Webview) -> Result<(), String> {
    let app = view.app_handle().clone();
    let id = view.label().to_string();
    view.with_webview(move |platform| {
        let result = (|| -> windows::core::Result<()> {
            unsafe {
                let controller = platform.controller();
                let core = controller.CoreWebView2()?;
                let mut version = PWSTR::null();
                platform.environment().BrowserVersionString(&mut version)?;
                let value = version.to_string().ok();
                windows::Win32::System::Com::CoTaskMemFree(Some(version.0.cast()));
                *app.state::<AppState>().runtime_version.lock().unwrap() = value;
                let settings = core.Settings()?;
                settings.SetAreDefaultContextMenusEnabled(true)?;
                // Wry deliberately returns path=None for interrupted downloads. Read
                // ResultFilePath from the real operation to identify the failed item,
                // including concurrent downloads of the same URL.
                let download_app = app.clone();
                let mut download_token = 0;
                core.cast::<ICoreWebView2_4>()?.add_DownloadStarting(
                    &DownloadStartingEventHandler::create(Box::new(move |_, args| {
                        if let Some(args) = args {
                            let app = download_app.clone();
                            let mut token = 0;
                            let operation = args.DownloadOperation()?;
                            let callback_app = app.clone();
                            operation.add_StateChanged(
                                &StateChangedEventHandler::create(Box::new(move |operation, _| {
                                    if let Some(operation) = operation {
                                        update_download_state(&callback_app, &operation)?;
                                    }
                                    Ok(())
                                })),
                                &mut token,
                            )?;
                            // A short response can finish before handlers are registered.
                            // Snapshot the actual state as well as observing future events.
                            let mut initial = COREWEBVIEW2_DOWNLOAD_STATE_IN_PROGRESS;
                            operation.State(&mut initial)?;
                            app.state::<AppState>().logs.add(
                                "info",
                                match initial {
                                    COREWEBVIEW2_DOWNLOAD_STATE_INTERRUPTED => {
                                        "download_started_interrupted"
                                    }
                                    COREWEBVIEW2_DOWNLOAD_STATE_COMPLETED => {
                                        "download_started_complete"
                                    }
                                    _ => "download_started_in_progress",
                                },
                            );
                            update_download_state(&app, &operation)?;
                        }
                        Ok(())
                    })),
                    &mut download_token,
                )?;
                let keyboard_app = app.clone();
                let keyboard_id = id.clone();
                let mut token = 0;
                controller.add_AcceleratorKeyPressed(
                    &AcceleratorKeyPressedEventHandler::create(Box::new(move |_, args| {
                        if let Some(args) = args {
                            let mut key = 0;
                            let mut kind = COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN;
                            args.VirtualKey(&mut key)?;
                            args.KeyEventKind(&mut kind)?;
                            if kind == COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN
                                || kind == COREWEBVIEW2_KEY_EVENT_KIND_SYSTEM_KEY_DOWN
                            {
                                let ctrl = GetKeyState(VK_CONTROL.0 as i32) < 0;
                                let shift = GetKeyState(VK_SHIFT.0 as i32) < 0;
                                if key == 123 || (ctrl && shift && key == 73) {
                                    args.SetHandled(true)?;
                                    let app = keyboard_app.clone();
                                    let id = keyboard_id.clone();
                                    tauri::async_runtime::spawn(async move {
                                        let enabled = app
                                            .state::<AppState>()
                                            .store
                                            .lock()
                                            .unwrap()
                                            .settings()
                                            .developer;
                                        if enabled {
                                            if let Some(v) = app.get_webview(&id) {
                                                v.open_devtools();
                                            }
                                        } else {
                                            let _ = app.emit_to(
                                                "shell",
                                                "notice",
                                                "Aktiver utviklermodus i Innstillinger → Utvikler.",
                                            );
                                        }
                                    });
                                } else if ctrl && [76, 84, 87].contains(&key) {
                                    args.SetHandled(true)?;
                                    let _ = keyboard_app.emit_to(
                                        "shell",
                                        "shortcut",
                                        match key {
                                            76 => "address",
                                            84 => "new_tab",
                                            _ => "close_tab",
                                        },
                                    );
                                    if let Some(shell) = keyboard_app.get_webview("shell") {
                                        let _ = shell.set_focus();
                                    }
                                }
                            }
                        }
                        Ok(())
                    })),
                    &mut token,
                )?;
                let nav_app = app.clone();
                let nav_id = id.clone();
                core.add_NavigationCompleted(
                    &NavigationCompletedEventHandler::create(Box::new(move |_, args| {
                        if let Some(args) = args {
                            let mut status = None;
                            if let Ok(args2) =
                                args.cast::<ICoreWebView2NavigationCompletedEventArgs2>()
                            {
                                let mut code = 0;
                                args2.HttpStatusCode(&mut code)?;
                                if code > 0 {
                                    status = Some(code);
                                }
                            }
                            let mut success = windows::core::BOOL(0);
                            args.IsSuccess(&mut success)?;
                            let error = if !success.as_bool() {
                                let mut code = COREWEBVIEW2_WEB_ERROR_STATUS_UNKNOWN;
                                args.WebErrorStatus(&mut code)?;
                                Some(format!("WebView2 navigation error {}", code.0))
                            } else {
                                None
                            };
                            {
                                let state = nav_app.state::<AppState>();
                                let mut session = state.session.lock().unwrap();
                                if let Some(t) = session.tabs.iter_mut().find(|t| t.id == nav_id) {
                                    t.http_status = status;
                                    t.navigation_error = error;
                                    t.loading = false;
                                }
                            }
                            browser::emit(&nav_app);
                        }
                        Ok(())
                    })),
                    &mut token,
                )?;
                let fail_app = app.clone();
                let fail_id = id.clone();
                core.add_ProcessFailed(
                    &ProcessFailedEventHandler::create(Box::new(move |_, _| {
                        fail_app
                            .state::<AppState>()
                            .logs
                            .add("error", "webview_process_failed");
                        {
                            let state = fail_app.state::<AppState>();
                            if let Some(t) = state
                                .session
                                .lock()
                                .unwrap()
                                .tabs
                                .iter_mut()
                                .find(|t| t.id == fail_id)
                            {
                                t.navigation_error =
                                    Some("Nettprosessen stoppet. Last inn fanen på nytt.".into());
                                t.loading = false;
                            };
                        }
                        browser::emit(&fail_app);
                        Ok(())
                    })),
                    &mut token,
                )?;
                Ok(())
            }
        })();
        if result.is_err() {
            app.state::<AppState>()
                .logs
                .add("warn", "native_hooks_unavailable");
        }
    })
    .map_err(|e| e.to_string())
}
pub async fn action(view: Webview, action: String, enabled: bool) -> Result<(), String> {
    let (tx, rx) = std::sync::mpsc::channel();
    view.with_webview(move |platform| {
        let result = (|| -> windows::core::Result<()> {
            unsafe {
                let core = platform.controller().CoreWebView2()?;
                match action.as_str() {
                    "back" => core.GoBack()?,
                    "forward" => core.GoForward()?,
                    "reload" => core.Reload()?,
                    "stop" => core.Stop()?,
                    "devtools" => core.OpenDevToolsWindow()?,
                    "set_developer" => {
                        core.Settings()?.SetAreDevToolsEnabled(enabled)?;
                    }
                    _ => {
                        return Err(windows::core::Error::from_hresult(windows::core::HRESULT(
                            0x80070057u32 as i32,
                        )))
                    }
                }
                Ok(())
            }
        })()
        .map_err(|e| e.to_string());
        let _ = tx.send(result);
    })
    .map_err(|e| e.to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        rx.recv_timeout(std::time::Duration::from_secs(10))
            .map_err(|_| "WebView2 svarte ikke".to_string())?
    })
    .await
    .map_err(|e| e.to_string())?
}

unsafe fn update_download_state(
    app: &tauri::AppHandle,
    operation: &ICoreWebView2DownloadOperation,
) -> windows::core::Result<()> {
    let mut status = COREWEBVIEW2_DOWNLOAD_STATE_IN_PROGRESS;
    operation.State(&mut status)?;
    if status == COREWEBVIEW2_DOWNLOAD_STATE_IN_PROGRESS {
        return Ok(());
    }
    let mut raw = PWSTR::null();
    operation.ResultFilePath(&mut raw)?;
    let path = raw.to_string().ok().map(std::path::PathBuf::from);
    windows::Win32::System::Com::CoTaskMemFree(Some(raw.0.cast()));
    if let Some(path) = path {
        let state = app.state::<AppState>();
        if let Some(d) = state
            .downloads
            .lock()
            .unwrap()
            .iter_mut()
            .find(|d| d.path == path)
        {
            d.status = if status == COREWEBVIEW2_DOWNLOAD_STATE_COMPLETED {
                "complete"
            } else {
                "failed"
            }
            .into();
        };
    }
    let _ = app.emit_to("shell", "downloads-changed", ());
    Ok(())
}
