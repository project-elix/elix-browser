//! Per-request media permission decisions. COM deferrals stay on WebView2's UI thread.
//! No grant is persisted in the WebView profile, including for private tabs.
use serde::Serialize;
use tauri::AppHandle;

#[derive(Clone, Serialize)]
pub struct PermissionPrompt {
    pub id: String,
    pub tab_id: String,
    pub profile_id: String,
    pub origin: String,
    pub top_origin: String,
    pub kind: String,
}

/// Media capture is only offered to HTTPS origins or the secure loopback exception.
fn media_origin(value: &str) -> Option<String> {
    let url = url::Url::parse(value).ok()?;
    let loopback = url.host_str().is_some_and(|host| {
        host == "localhost"
            || host.ends_with(".localhost")
            || host
                .trim_matches(['[', ']'])
                .parse::<std::net::IpAddr>()
                .is_ok_and(|ip| ip.is_loopback())
    });
    if (url.scheme() != "https" && !(url.scheme() == "http" && loopback))
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return None;
    }
    Some(url.origin().ascii_serialization())
}

#[cfg(windows)]
mod native {
    use super::*;
    use crate::AppState;
    use std::{cell::RefCell, collections::HashMap};
    use tauri::{Emitter, Manager, Webview};
    use webview2_com::{
        GetNonDefaultPermissionSettingsCompletedHandler, Microsoft::Web::WebView2::Win32::*,
        NavigationStartingEventHandler, PermissionRequestedEventHandler, ProcessFailedEventHandler,
        SetPermissionStateCompletedHandler,
    };
    use windows::core::{Interface, PCWSTR, PWSTR};

    struct Pending {
        prompt: PermissionPrompt,
        top_url: String,
        args: ICoreWebView2PermissionRequestedEventArgs,
        deferral: Option<ICoreWebView2Deferral>,
    }

    impl Pending {
        fn finish(&mut self, allow: bool) -> Result<(), String> {
            unsafe {
                // Always complete the deferral, even if setting State fails.
                let state = self.args.SetState(if allow {
                    COREWEBVIEW2_PERMISSION_STATE_ALLOW
                } else {
                    COREWEBVIEW2_PERMISSION_STATE_DENY
                });
                let completion = self.deferral.take().map(|deferral| deferral.Complete());
                state.map_err(|e| e.to_string())?;
                if let Some(result) = completion {
                    result.map_err(|e| e.to_string())?;
                }
                Ok(())
            }
        }
    }

    impl Drop for Pending {
        fn drop(&mut self) {
            if self.deferral.is_some() {
                let _ = self.finish(false);
            }
        }
    }

    thread_local! {
        static PENDING: RefCell<HashMap<String, Pending>> = RefCell::new(HashMap::new());
    }

    fn snapshot(app: &AppHandle) -> Vec<PermissionPrompt> {
        let state = app.state::<AppState>();
        let profile = state.profile.lock().unwrap().clone();
        let active = state.session.lock().unwrap().active.clone();
        PENDING.with(|pending| {
            let mut result: Vec<_> = pending
                .borrow()
                .values()
                .filter(|p| p.prompt.profile_id == profile && p.prompt.tab_id == active)
                .map(|p| p.prompt.clone())
                .collect();
            result.sort_by(|a, b| a.id.cmp(&b.id));
            result
        })
    }

    fn emit(app: &AppHandle) {
        let prompts = snapshot(app);
        app.state::<AppState>()
            .permissions_visible
            .store(!prompts.is_empty(), std::sync::atomic::Ordering::SeqCst);
        let active = app
            .state::<AppState>()
            .session
            .lock()
            .unwrap()
            .active
            .clone();
        if !prompts.is_empty() {
            // Native child WebViews cover HTML dialogs. Hide the page until the
            // entire request batch is resolved so Allow/Deny remains accessible.
            for (id, view) in app.webviews() {
                if id.starts_with("tab-") {
                    let _ = view.hide();
                }
            }
            if let Some(shell) = app.get_webview("shell") {
                let _ = shell.set_focus();
            }
        } else {
            let _ = crate::browser::organization::layout(app);
            if let Some(view) = app.get_webview(&active) {
                let _ = view.set_focus();
            }
        }
        let _ = app.emit_to("shell", "site-permissions", prompts);
    }

    fn cancel_on_ui(app: &AppHandle, tab: Option<&str>) {
        // Take entries out before completing deferrals: COM may call another event.
        let removed: Vec<Pending> = PENDING.with(|pending| {
            let mut pending = pending.borrow_mut();
            let ids: Vec<_> = pending
                .iter()
                .filter(|(_, value)| tab.is_none_or(|id| value.prompt.tab_id == id))
                .map(|(id, _)| id.clone())
                .collect();
            ids.into_iter()
                .filter_map(|id| pending.remove(&id))
                .collect()
        });
        let changed = !removed.is_empty();
        drop(removed);
        if changed {
            emit(app);
        }
    }

    /// Alpha 4 denied every permission using WebView2's persistent default. Remove
    /// only those media decisions so an upgraded profile can show the new prompt.
    unsafe fn reset_saved_media_permissions(core: &ICoreWebView2) -> windows::core::Result<()> {
        let profile = core
            .cast::<ICoreWebView2_13>()?
            .Profile()?
            .cast::<ICoreWebView2Profile4>()?;
        let callback_profile = profile.clone();
        profile.GetNonDefaultPermissionSettings(
            &GetNonDefaultPermissionSettingsCompletedHandler::create(Box::new(
                move |result, values| {
                    result?;
                    if let Some(values) = values {
                        let mut count = 0;
                        values.Count(&mut count)?;
                        for index in 0..count {
                            let value = values.GetValueAtIndex(index)?;
                            let mut kind = COREWEBVIEW2_PERMISSION_KIND_UNKNOWN_PERMISSION;
                            value.PermissionKind(&mut kind)?;
                            if kind != COREWEBVIEW2_PERMISSION_KIND_CAMERA
                                && kind != COREWEBVIEW2_PERMISSION_KIND_MICROPHONE
                                && kind != COREWEBVIEW2_PERMISSION_KIND_GEOLOCATION
                                && kind != COREWEBVIEW2_PERMISSION_KIND_NOTIFICATIONS
                            {
                                continue;
                            }
                            let mut origin = PWSTR::null();
                            value.PermissionOrigin(&mut origin)?;
                            let reset = callback_profile.SetPermissionState(
                                kind,
                                PCWSTR(origin.0),
                                COREWEBVIEW2_PERMISSION_STATE_DEFAULT,
                                &SetPermissionStateCompletedHandler::create(Box::new(|result| {
                                    result
                                })),
                            );
                            windows::Win32::System::Com::CoTaskMemFree(Some(origin.0.cast()));
                            reset?;
                        }
                    }
                    Ok(())
                },
            )),
        )
    }

    pub fn attach(view: &Webview) -> Result<(), String> {
        let app = view.app_handle().clone();
        let tab_id = view.label().to_string();
        view.with_webview(move |platform| {
            let result = (|| -> windows::core::Result<()> {
                unsafe {
                    let core = platform.controller().CoreWebView2()?;
                    // A previous default deny must not silently block future prompts.
                    let _ = reset_saved_media_permissions(&core);
                    let request_app = app.clone();
                    let request_tab = tab_id.clone();
                    let mut token = 0;
                    core.add_PermissionRequested(
                        &PermissionRequestedEventHandler::create(Box::new(move |_, args| {
                            let Some(args) = args else { return Ok(()) };
                            args.SetState(COREWEBVIEW2_PERMISSION_STATE_DENY)?;
                            // Require control of persistence before offering any grant.
                            let Ok(extended) =
                                args.cast::<ICoreWebView2PermissionRequestedEventArgs3>()
                            else {
                                return Ok(());
                            };
                            extended.SetSavesInProfile(false)?;
                            let mut kind = COREWEBVIEW2_PERMISSION_KIND_UNKNOWN_PERMISSION;
                            args.PermissionKind(&mut kind)?;
                            let kind = match kind {
                                COREWEBVIEW2_PERMISSION_KIND_CAMERA => "camera",
                                COREWEBVIEW2_PERMISSION_KIND_MICROPHONE => "microphone",
                                COREWEBVIEW2_PERMISSION_KIND_GEOLOCATION => "location",
                                COREWEBVIEW2_PERMISSION_KIND_NOTIFICATIONS => "notifications",
                                _ => return Ok(()),
                            };
                            let mut raw = PWSTR::null();
                            args.Uri(&mut raw)?;
                            let uri = raw.to_string().ok();
                            windows::Win32::System::Com::CoTaskMemFree(Some(raw.0.cast()));
                            let Some(origin) = uri.as_deref().and_then(media_origin) else {
                                return Ok(());
                            };
                            let state = request_app.state::<AppState>();
                            let tab = {
                                let session = state.session.lock().unwrap();
                                if session.active != request_tab {
                                    return Ok(());
                                }
                                session.tabs.iter().find(|t| t.id == request_tab).cloned()
                            };
                            let Some(tab) = tab else { return Ok(()) };
                            let Some(top_origin) = media_origin(&tab.url) else {
                                return Ok(());
                            };
                            // Saved choices are profile scoped and only apply to the top-level origin.
                            // Private and third-party frame requests always ask separately.
                            if !tab.private && origin == top_origin {
                                let profile = state.profile.lock().unwrap().clone();
                                let choice = state
                                    .store
                                    .lock()
                                    .unwrap()
                                    .permission(&profile, &origin, kind);
                                if choice == "allow" {
                                    args.SetState(COREWEBVIEW2_PERMISSION_STATE_ALLOW)?;
                                    return Ok(());
                                }
                                if choice == "deny" {
                                    return Ok(());
                                }
                            }
                            if PENDING.with(|pending| pending.borrow().len() >= 8) {
                                return Ok(());
                            }
                            let id = uuid::Uuid::new_v4().to_string();
                            let pending = Pending {
                                prompt: PermissionPrompt {
                                    id: id.clone(),
                                    tab_id: request_tab.clone(),
                                    profile_id: state.profile.lock().unwrap().clone(),
                                    origin,
                                    top_origin,
                                    kind: kind.into(),
                                },
                                top_url: tab.url,
                                args: args.clone(),
                                deferral: Some(args.GetDeferral()?),
                            };
                            PENDING.with(|items| items.borrow_mut().insert(id.clone(), pending));
                            emit(&request_app);
                            let timeout_app = request_app.clone();
                            std::thread::spawn(move || {
                                std::thread::sleep(std::time::Duration::from_secs(60));
                                let callback_app = timeout_app.clone();
                                let _ = timeout_app.run_on_main_thread(move || {
                                    let expired =
                                        PENDING.with(|items| items.borrow_mut().remove(&id));
                                    if expired.is_some() {
                                        drop(expired);
                                        emit(&callback_app);
                                    }
                                });
                            });
                            Ok(())
                        })),
                        &mut token,
                    )?;
                    let navigation_app = app.clone();
                    let navigation_tab = tab_id.clone();
                    core.add_NavigationStarting(
                        &NavigationStartingEventHandler::create(Box::new(move |_, _| {
                            cancel_on_ui(&navigation_app, Some(&navigation_tab));
                            Ok(())
                        })),
                        &mut token,
                    )?;
                    let failed_app = app.clone();
                    core.add_ProcessFailed(
                        &ProcessFailedEventHandler::create(Box::new(move |_, _| {
                            cancel_on_ui(&failed_app, Some(&tab_id));
                            Ok(())
                        })),
                        &mut token,
                    )?;
                }
                Ok(())
            })();
            if result.is_err() {
                app.state::<AppState>()
                    .logs
                    .add("warn", "media_permissions_unavailable");
            }
        })
        .map_err(|e| e.to_string())
    }

    pub fn cancel(app: &AppHandle, tab_id: &str) {
        let callback_app = app.clone();
        let tab_id = tab_id.to_string();
        let _ = app.run_on_main_thread(move || cancel_on_ui(&callback_app, Some(&tab_id)));
    }

    pub fn cancel_all(app: &AppHandle) {
        let callback_app = app.clone();
        let _ = app.run_on_main_thread(move || cancel_on_ui(&callback_app, None));
    }

    pub async fn list(app: &AppHandle) -> Result<Vec<PermissionPrompt>, String> {
        let (tx, rx) = std::sync::mpsc::channel();
        let list_app = app.clone();
        app.run_on_main_thread(move || {
            let _ = tx.send(snapshot(&list_app));
        })
        .map_err(|e| e.to_string())?;
        tauri::async_runtime::spawn_blocking(move || {
            rx.recv_timeout(std::time::Duration::from_secs(10))
                .map_err(|_| "Nettstedstillatelsene svarte ikke".into())
        })
        .await
        .map_err(|e| e.to_string())?
    }

    pub async fn respond(app: &AppHandle, id: &str, allow: bool) -> Result<(), String> {
        let (tx, rx) = std::sync::mpsc::channel();
        let callback_app = app.clone();
        let id = id.to_string();
        app.run_on_main_thread(move || {
            let result = (|| -> Result<(), String> {
                let mut pending = PENDING
                    .with(|items| items.borrow_mut().remove(&id))
                    .ok_or("Forespørselen er utløpt eller avbrutt")?;
                let state = callback_app.state::<AppState>();
                let profile_matches = *state.profile.lock().unwrap() == pending.prompt.profile_id;
                let same_page = {
                    let session = state.session.lock().unwrap();
                    session.active == pending.prompt.tab_id
                        && session.tabs.iter().any(|tab| {
                            tab.id == pending.prompt.tab_id && tab.url == pending.top_url
                        })
                };
                let granted = allow
                    && profile_matches
                    && same_page
                    && callback_app.get_webview(&pending.prompt.tab_id).is_some();
                pending.finish(granted)?;
                state.logs.add(
                    "info",
                    if granted {
                        "media_permission_allowed"
                    } else {
                        "media_permission_denied"
                    },
                );
                if allow && !granted {
                    return Err(
                        "Fanen eller nettstedet er endret. Prøv på nytt på nettstedet.".into(),
                    );
                }
                Ok(())
            })();
            emit(&callback_app);
            let _ = tx.send(result);
        })
        .map_err(|e| e.to_string())?;
        tauri::async_runtime::spawn_blocking(move || {
            rx.recv_timeout(std::time::Duration::from_secs(10))
                .map_err(|_| "Nettstedstillatelsen svarte ikke".to_string())?
        })
        .await
        .map_err(|e| e.to_string())?
    }
}

#[cfg(windows)]
pub use native::{attach, cancel, cancel_all, list, respond};

#[cfg(not(windows))]
pub fn cancel(_: &AppHandle, _: &str) {}
#[cfg(not(windows))]
pub fn cancel_all(_: &AppHandle) {}
#[cfg(not(windows))]
pub async fn list(_: &AppHandle) -> Result<Vec<PermissionPrompt>, String> {
    Ok(Vec::new())
}
#[cfg(not(windows))]
pub async fn respond(_: &AppHandle, _: &str, _: bool) -> Result<(), String> {
    Err("Nettstedstillatelser krever Windows/WebView2".into())
}

#[cfg(test)]
mod tests {
    use super::media_origin;

    #[test]
    fn accepts_secure_origins_without_paths() {
        assert_eq!(
            media_origin("https://example.com:8443/call?token=private"),
            Some("https://example.com:8443".into())
        );
        assert_eq!(
            media_origin("http://localhost:8080/test"),
            Some("http://localhost:8080".into())
        );
        assert_eq!(
            media_origin("http://127.0.0.1/test"),
            Some("http://127.0.0.1".into())
        );
        assert_eq!(
            media_origin("http://[::1]/test"),
            Some("http://[::1]".into())
        );
    }

    #[test]
    fn denies_insecure_opaque_and_spoofed_origins() {
        for value in [
            "http://example.com",
            "http://localhost.example.com",
            "http://192.168.1.1",
            "file:///camera",
            "data:text/html,camera",
            "https://user:password@example.com",
            "elix://start",
        ] {
            assert_eq!(media_origin(value), None, "{value}");
        }
    }
}
