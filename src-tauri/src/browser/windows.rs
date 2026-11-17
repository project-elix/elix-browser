use crate::{browser, AppState};
use tauri::{Emitter, Manager, Webview};
use webview2_com::{
    AcceleratorKeyPressedEventHandler, ContainsFullScreenElementChangedEventHandler, FocusChangedEventHandler,
    Microsoft::Web::WebView2::Win32::*, NavigationCompletedEventHandler, NavigationStartingEventHandler, ProcessFailedEventHandler,
};
use windows::{
    core::{Interface, PWSTR},
    Win32::UI::Input::KeyboardAndMouse::{
        GetKeyState, VK_CONTROL, VK_LWIN, VK_MENU, VK_RMENU, VK_RWIN, VK_SHIFT,
    },
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
                let muted=app.state::<AppState>().session.lock().unwrap().tabs.iter().find(|t|t.id==id).is_some_and(|t|t.muted);
                core.cast::<ICoreWebView2_8>()?.SetIsMuted(muted)?;
                let focus_app=app.clone();let focus_id=id.clone();let mut focus_token=0;
                controller.add_GotFocus(&FocusChangedEventHandler::create(Box::new(move|_,_|{
                    let state=focus_app.state::<AppState>();
                    {let mut s=state.session.lock().unwrap();if s.split.as_ref().is_some_and(|pair|pair.contains(&focus_id)){s.active=focus_id.clone();}}
                    let _=focus_app.emit("page-focused", &focus_id);
                    browser::emit(&focus_app);Ok(())
                })),&mut focus_token)?;
                let settings = core.Settings()?;
                settings.SetAreDefaultContextMenusEnabled(true)?;
                crate::downloads::windows::attach(&core, app.clone(), id.clone())?;
                let fullscreen_app = app.clone();
                let fullscreen_id = id.clone();
                let mut fullscreen_token = 0;
                core.add_ContainsFullScreenElementChanged(
                    &ContainsFullScreenElementChangedEventHandler::create(Box::new(move |sender, _| {
                        if let Some(sender) = sender {
                            let mut enabled = windows::core::BOOL::default();
                            sender.ContainsFullScreenElement(&mut enabled)?;
                            if browser::media::html_fullscreen(&fullscreen_app, &fullscreen_id, enabled.as_bool()).is_err() {
                                fullscreen_app.state::<AppState>().logs.add("warn", "media_fullscreen_event_failed");
                            }
                        }
                        Ok(())
                    })),
                    &mut fullscreen_token,
                )?;
                let leaving_app = app.clone();
                let leaving_id = id.clone();
                let mut leaving_token = 0;
                core.add_NavigationStarting(
                    &NavigationStartingEventHandler::create(Box::new(move |_, _| {
                        browser::media::navigation_started(&leaving_app, &leaving_id);
                        Ok(())
                    })),
                    &mut leaving_token,
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
                                #[cfg(feature = "smoke-tests")]
                                if key == 72 && std::env::args().any(|arg| arg == "--self-test") {
                                    keyboard_app.state::<AppState>().logs.add("info", if ctrl { "smoke_native_ctrl_h" } else { "smoke_native_h_without_ctrl" });
                                }

                                if (key == 27 || key == 122) && browser::media::is_fullscreen(&keyboard_app, &keyboard_id) {
                                    args.SetHandled(true)?;
                                    let app = keyboard_app.clone();
                                    let id = keyboard_id.clone();
                                    tauri::async_runtime::spawn(async move { let _ = browser::media::exit(&app, &id); });
                                } else if (key == 122 || (ctrl && key == 87) || (key == 27 && keyboard_app.get_window("media-player").is_some_and(|w| w.is_fullscreen().unwrap_or(false)))) && browser::media::is_player(&keyboard_app, &keyboard_id) {
                                    args.SetHandled(true)?;
                                    let app = keyboard_app.clone();
                                    tauri::async_runtime::spawn(async move {
                                        if key == 87 { let _ = browser::media::perform(&app, "return", None); }
                                        else if let Some(window) = app.get_window("media-player") {
                                            let _ = window.set_fullscreen(key != 27 && !window.is_fullscreen().unwrap_or(false));
                                        }
                                    });
                                } else if !browser::media::is_player(&keyboard_app, &keyboard_id) && (key == 122
                                    || (key == 27
                                        && keyboard_app
                                            .get_window("main")
                                            .is_some_and(|w| w.is_fullscreen().unwrap_or(false))))
                                {
                                    args.SetHandled(true)?;
                                    let app = keyboard_app.clone();
                                    tauri::async_runtime::spawn(async move {
                                        let action = if key == 122 {
                                            crate::window::Action::ToggleFullscreen
                                        } else {
                                            crate::window::Action::ExitFullscreen
                                        };
                                        let _ = crate::window::perform(&app, action);
                                    });
                                } else if key == 123 || (ctrl && shift && key == 73) {
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
                                } else if ctrl
                                    && key == 83
                                    && !shift
                                    && GetKeyState(VK_MENU.0 as i32) < 0
                                    && GetKeyState(VK_RMENU.0 as i32) >= 0
                                    && GetKeyState(VK_LWIN.0 as i32) >= 0
                                    && GetKeyState(VK_RWIN.0 as i32) >= 0
                                {
                                    // Handle split-view even when the native page owns focus.
                                    // Right Alt is reserved for AltGr text input. Holding the
                                    // shortcut must not repeatedly open and close the split.
                                    args.SetHandled(true)?;
                                    if browser::media::is_player(&keyboard_app, &keyboard_id) {
                                        let _ = keyboard_app.emit_to("shell", "notice", "Flytt minispilleren tilbake til fanen før du bruker delt visning.");
                                        return Ok(());
                                    }
                                    let mut status = COREWEBVIEW2_PHYSICAL_KEY_STATUS::default();
                                    args.PhysicalKeyStatus(&mut status)?;
                                    if !status.WasKeyDown.as_bool() {
                                        let _ = keyboard_app.emit_to("shell", "shortcut", "split");
                                        if let Some(shell) = keyboard_app.get_webview("shell") {
                                            let _ = shell.set_focus();
                                        }
                                    }
                                } else if ctrl
                                    && ([72, 74, 76, 84, 87, 65].contains(&key) || (shift && key == 78)) && (key != 65 || shift)
                                {
                                    args.SetHandled(true)?;
                                    let shortcut = match key {
                                            72 => "history",
                                            74 => "downloads",
                                            76 => "address",
                                            78 => "private_tab",
                                            84 if shift => "reopen_tab",
                                            84 => "new_tab",
                                            65 if shift => "search_tabs",
                                            65 => "select_all",
                                            _ => "close_tab",
                                        };
                                    let app = keyboard_app.clone();
                                    let id = keyboard_id.clone();
                                    tauri::async_runtime::spawn(async move {
                                        // Address/history shortcuts from a floating page
                                        // must not act on an unrelated main-window tab.
                                        if browser::media::is_player(&app, &id)
                                            && browser::media::perform(&app, "return", None).is_err() { return; }
                                        let _ = app.emit_to("shell", "shortcut", shortcut);
                                        if let Some(shell) = app.get_webview("shell") { let _ = shell.set_focus(); }
                                    });
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
                                match code {
                                    COREWEBVIEW2_WEB_ERROR_STATUS_OPERATION_CANCELED => None,
                                    COREWEBVIEW2_WEB_ERROR_STATUS_HOST_NAME_NOT_RESOLVED => Some("Fant ikke nettstedet. Kontroller adressen og internettforbindelsen.".into()),
                                    COREWEBVIEW2_WEB_ERROR_STATUS_TIMEOUT => Some("Nettstedet brukte for lang tid på å svare. Prøv igjen.".into()),
                                    COREWEBVIEW2_WEB_ERROR_STATUS_CANNOT_CONNECT | COREWEBVIEW2_WEB_ERROR_STATUS_SERVER_UNREACHABLE | COREWEBVIEW2_WEB_ERROR_STATUS_DISCONNECTED => Some("Kunne ikke koble til nettstedet. Kontroller internettforbindelsen og prøv igjen.".into()),
                                    COREWEBVIEW2_WEB_ERROR_STATUS_CERTIFICATE_COMMON_NAME_IS_INCORRECT | COREWEBVIEW2_WEB_ERROR_STATUS_CERTIFICATE_EXPIRED | COREWEBVIEW2_WEB_ERROR_STATUS_CERTIFICATE_IS_INVALID | COREWEBVIEW2_WEB_ERROR_STATUS_CERTIFICATE_REVOKED => Some("Nettstedets sikkerhetssertifikat kunne ikke godkjennes. Tilkoblingen er blokkert.".into()),
                                    _ => Some(format!("Siden kunne ikke lastes inn (nettverksfeil {}). Prøv igjen.", code.0)),
                                }
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
                        let cleanup_app = fail_app.clone();
                        let cleanup_id = fail_id.clone();
                        tauri::async_runtime::spawn(async move { let _ = browser::media::release(&cleanup_app, &cleanup_id); });
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
                                t.crashed = true;
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
                    "mute" => core.cast::<ICoreWebView2_8>()?.SetIsMuted(enabled)?,
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
