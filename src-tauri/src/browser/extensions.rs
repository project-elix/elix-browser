//! Native, profile-scoped extension and password controls. No credentials leave WebView2.
use crate::AppState;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager, Webview};

#[derive(Clone)]
pub enum Operation {
    List,
    Add(std::path::PathBuf),
    Enable { id: String, enabled: bool },
    Remove { id: String },
    ClearPasswords,
}

/// The user selects an unpacked extension; the path is never supplied by a webpage.
pub async fn pick_and_add(app: AppHandle) -> Result<Value, String> {
    use tauri_plugin_dialog::DialogExt;
    let selected_profile = app.state::<AppState>().profile.lock().unwrap().clone();
    let picker = app.clone();
    let selected = tauri::async_runtime::spawn_blocking(move || {
        picker
            .dialog()
            .file()
            .set_title("Velg utvidelsesmappe med manifest.json")
            .blocking_pick_folder()
    })
    .await
    .map_err(|e| e.to_string())?;
    let Some(selected) = selected else {
        return Ok(Value::Null);
    };
    if *app.state::<AppState>().profile.lock().unwrap() != selected_profile {
        return Err("Profilen ble endret. Velg utvidelsen på nytt i riktig profil.".into());
    }
    let path = selected.into_path().map_err(|e| e.to_string())?;
    let manifest = path.join("manifest.json");
    if !manifest.is_file() {
        return Err("Velg den utpakkede utvidelsesmappen som inneholder manifest.json.".into());
    }
    execute(app, Operation::Add(path)).await
}

/// Configure every new view, including private views. Called immediately after creation.
pub fn configure(
    view: &Webview,
    password_autosave: bool,
    autofill: bool,
    private: bool,
) -> Result<(), String> {
    #[cfg(windows)]
    {
        let app = view.app_handle().clone();
        view.with_webview(move |platform| {
            let result = unsafe {
                platform.controller().CoreWebView2().and_then(|core| {
                    native::set_preferences(&core, password_autosave, autofill, private)
                })
            };
            if result.is_err() {
                app.state::<AppState>()
                    .logs
                    .add("warn", "autofill_settings_unavailable");
            }
        })
        .map_err(|e| e.to_string())
    }
    #[cfg(not(windows))]
    {
        let _ = (view, password_autosave, autofill, private);
        Ok(())
    }
}

pub async fn apply_preferences(
    view: Webview,
    password_autosave: bool,
    autofill: bool,
    private: bool,
) -> Result<(), String> {
    #[cfg(windows)]
    {
        let (tx, rx) = std::sync::mpsc::channel();
        view.with_webview(move |platform| {
            let result = unsafe {
                platform.controller().CoreWebView2().and_then(|core| {
                    native::set_preferences(&core, password_autosave, autofill, private)
                })
            };
            let _ = tx.send(result.map_err(|e| e.to_string()));
        })
        .map_err(|e| e.to_string())?;
        tauri::async_runtime::spawn_blocking(move || {
            rx.recv_timeout(std::time::Duration::from_secs(15))
                .map_err(|_| "WebView2 svarte ikke".to_string())?
        })
        .await
        .map_err(|e| e.to_string())?
    }
    #[cfg(not(windows))]
    {
        let _ = (view, password_autosave, autofill, private);
        Err("Krever Windows/WebView2".into())
    }
}

pub async fn execute(app: AppHandle, operation: Operation) -> Result<Value, String> {
    #[cfg(windows)]
    {
        native::execute(app, operation).await
    }
    #[cfg(not(windows))]
    {
        let _ = (app, operation);
        Err("Krever Windows/WebView2".into())
    }
}

#[cfg(all(windows, feature = "smoke-tests"))]
pub async fn smoke(app: AppHandle) -> Result<(), String> {
    native::smoke(app).await
}

#[cfg(windows)]
mod native {
    use super::*;
    use std::sync::mpsc::Sender;
    use webview2_com::{
        BrowserExtensionEnableCompletedHandler, BrowserExtensionRemoveCompletedHandler,
        ClearBrowsingDataCompletedHandler, Microsoft::Web::WebView2::Win32::*,
        ProfileAddBrowserExtensionCompletedHandler, ProfileGetBrowserExtensionsCompletedHandler,
    };
    use windows::core::{Interface, HSTRING, PWSTR};

    pub unsafe fn set_preferences(
        core: &ICoreWebView2,
        autosave: bool,
        autofill: bool,
        private: bool,
    ) -> windows::core::Result<()> {
        let settings = core.Settings()?.cast::<ICoreWebView2Settings4>()?;
        settings.SetIsPasswordAutosaveEnabled(autosave && !private)?;
        settings.SetIsGeneralAutofillEnabled(autofill && !private)
    }

    unsafe fn take_string(raw: PWSTR) -> String {
        let value = raw.to_string().unwrap_or_default();
        windows::Win32::System::Com::CoTaskMemFree(Some(raw.0.cast()));
        value
    }

    unsafe fn extension_data(
        extension: &ICoreWebView2BrowserExtension,
    ) -> windows::core::Result<Value> {
        let mut id = PWSTR::null();
        extension.Id(&mut id)?;
        let id = take_string(id);
        let mut name = PWSTR::null();
        extension.Name(&mut name)?;
        let name = take_string(name);
        let mut enabled = windows::core::BOOL(0);
        extension.IsEnabled(&mut enabled)?;
        Ok(json!({"id": id, "name": name, "enabled": enabled.as_bool()}))
    }

    fn finish(tx: &Sender<Result<Value, String>>, result: windows::core::Result<Value>) {
        let _ = tx.send(result.map_err(|e| format!("WebView2: {e}")));
    }

    // A temporary hidden about:blank view gives access to the selected profile even
    // when all visible tabs are internal pages. It has no shell IPC authorization,
    // cannot navigate, and is closed after the native operation completes.
    fn profile_view(app: &AppHandle) -> Result<(Webview, bool), String> {
        let state = app.state::<AppState>();
        let existing = state
            .session
            .lock()
            .unwrap()
            .tabs
            .iter()
            .filter(|tab| !tab.private)
            .find_map(|tab| app.get_webview(&tab.id));
        if let Some(view) = existing {
            return Ok((view, false));
        }
        let profile = state.profile.lock().unwrap().clone();
        let directory = crate::profiles::directory(&state.root, &profile)?;
        let preferences = state.store.lock().unwrap().settings();
        let builder = tauri::webview::WebviewBuilder::new(
            format!("profile-tools-{}", uuid::Uuid::new_v4()),
            tauri::WebviewUrl::External("about:blank".parse().unwrap()),
        )
        .data_directory(directory)
        .browser_extensions_enabled(true)
        .general_autofill_enabled(preferences.autofill)
        .on_navigation(|url| url.as_str() == "about:blank")
        .on_new_window(|_, _| tauri::webview::NewWindowResponse::Deny)
        .on_permission_request(|_, _| tauri::webview::PermissionResponse::Deny);
        let window = app.get_window("main").ok_or("Fant ikke hovedvinduet")?;
        let view = window
            .add_child(
                builder,
                tauri::LogicalPosition::new(-10., -10.),
                tauri::LogicalSize::new(1., 1.),
            )
            .map_err(|e| e.to_string())?;
        view.hide().map_err(|e| e.to_string())?;
        configure(
            &view,
            preferences.password_autosave,
            preferences.autofill,
            false,
        )?;
        Ok((view, true))
    }

    pub async fn execute(app: AppHandle, operation: Operation) -> Result<Value, String> {
        let (view, temporary) = profile_view(&app)?;
        let (tx, rx) = std::sync::mpsc::channel();
        let started = view.with_webview(move |platform| {
            let result = (|| -> windows::core::Result<()> { unsafe {
                let core = platform.controller().CoreWebView2()?;
                let profile = core.cast::<ICoreWebView2_13>()?.Profile()?;
                match operation {
                    Operation::ClearPasswords => {
                        let callback_tx = tx.clone();
                        profile.cast::<ICoreWebView2Profile2>()?.ClearBrowsingData(
                            COREWEBVIEW2_BROWSING_DATA_KINDS_PASSWORD_AUTOSAVE,
                            &ClearBrowsingDataCompletedHandler::create(Box::new(move |result| {
                                finish(&callback_tx, result.map(|_| Value::Null)); Ok(())
                            })),
                        )?;
                    }
                    Operation::Add(path) => {
                        let callback_tx = tx.clone();
                        profile.cast::<ICoreWebView2Profile7>()?.AddBrowserExtension(
                            &HSTRING::from(path.as_os_str()),
                            &ProfileAddBrowserExtensionCompletedHandler::create(Box::new(move |result, extension| {
                                finish(&callback_tx, result.and_then(|_| extension.map(|extension| extension_data(&extension)).unwrap_or(Ok(Value::Null))));
                                Ok(())
                            })),
                        )?;
                    }
                    operation => {
                        let callback_tx = tx.clone();
                        profile.cast::<ICoreWebView2Profile7>()?.GetBrowserExtensions(
                            &ProfileGetBrowserExtensionsCompletedHandler::create(Box::new(move |result, list| {
                                let result = (|| -> windows::core::Result<()> {
                                    result?;
                                    let Some(list) = list else {
                                        let _ = callback_tx.send(if matches!(operation, Operation::List) { Ok(json!([])) } else { Err("Fant ikke utvidelsen i denne profilen".into()) });
                                        return Ok(());
                                    };
                                    let mut count = 0;
                                    list.Count(&mut count)?;
                                    let mut output = Vec::new();
                                    for index in 0..count {
                                        let extension = list.GetValueAtIndex(index)?;
                                        let data = extension_data(&extension)?;
                                        match &operation {
                                            Operation::List => output.push(data),
                                            Operation::Enable { id, enabled } if data["id"].as_str() == Some(id.as_str()) => {
                                                let completion_tx = callback_tx.clone();
                                                extension.Enable(*enabled, &BrowserExtensionEnableCompletedHandler::create(Box::new(move |result| {
                                                    finish(&completion_tx, result.map(|_| Value::Null)); Ok(())
                                                })))?;
                                                return Ok(());
                                            }
                                            Operation::Remove { id } if data["id"].as_str() == Some(id.as_str()) => {
                                                let completion_tx = callback_tx.clone();
                                                extension.Remove(&BrowserExtensionRemoveCompletedHandler::create(Box::new(move |result| {
                                                    finish(&completion_tx, result.map(|_| Value::Null)); Ok(())
                                                })))?;
                                                return Ok(());
                                            }
                                            _ => (),
                                        }
                                    }
                                    let _ = callback_tx.send(if matches!(operation, Operation::List) { Ok(json!(output)) } else { Err("Fant ikke utvidelsen i denne profilen".into()) });
                                    Ok(())
                                })();
                                if let Err(error) = result { finish(&callback_tx, Err(error)); }
                                Ok(())
                            })),
                        )?;
                    }
                }
                Ok(())
            }})();
            if let Err(error) = result { finish(&tx, Err(error)); }
        }).map_err(|e| e.to_string());
        let result = match started {
            Err(error) => Err(error),
            Ok(()) => match tauri::async_runtime::spawn_blocking(move || {
                rx.recv_timeout(std::time::Duration::from_secs(30))
                    .map_err(|_| "WebView2 svarte ikke. Prøv igjen.".to_string())?
            })
            .await
            {
                Ok(result) => result,
                Err(error) => Err(error.to_string()),
            },
        };
        if temporary {
            let _ = view.close();
        }
        result
    }

    #[cfg(feature = "smoke-tests")]
    async fn preferences_snapshot(view: &Webview) -> Result<(bool, bool), String> {
        let (tx, rx) = std::sync::mpsc::channel();
        view.with_webview(move |platform| {
            let result = (|| -> windows::core::Result<(bool, bool)> {
                unsafe {
                    let settings = platform
                        .controller()
                        .CoreWebView2()?
                        .Settings()?
                        .cast::<ICoreWebView2Settings4>()?;
                    let mut autosave = windows::core::BOOL(0);
                    let mut autofill = windows::core::BOOL(0);
                    settings.IsPasswordAutosaveEnabled(&mut autosave)?;
                    settings.IsGeneralAutofillEnabled(&mut autofill)?;
                    Ok((autosave.as_bool(), autofill.as_bool()))
                }
            })();
            let _ = tx.send(result.map_err(|e| e.to_string()));
        })
        .map_err(|e| e.to_string())?;
        tauri::async_runtime::spawn_blocking(move || {
            rx.recv_timeout(std::time::Duration::from_secs(10))
                .map_err(|_| "autofill property read timeout".to_string())?
        })
        .await
        .map_err(|e| e.to_string())?
    }

    #[cfg(feature = "smoke-tests")]
    pub async fn smoke(app: AppHandle) -> Result<(), String> {
        let (view, temporary) = profile_view(&app)?;
        let before = preferences_snapshot(&view).await?;
        let preferences_result = async {
            apply_preferences(view.clone(), true, true, false).await?;
            if preferences_snapshot(&view).await? != (true, true) {
                return Err("native autofill opt-in failed".into());
            }
            apply_preferences(view.clone(), true, true, true).await?;
            if preferences_snapshot(&view).await? != (false, false) {
                return Err("private autofill override failed".into());
            }
            Ok::<(), String>(())
        }
        .await;
        let restore = apply_preferences(view.clone(), before.0, before.1, false).await;
        if temporary {
            let _ = view.close();
        }
        preferences_result?;
        restore?;

        let path = app.state::<AppState>().root.join("extension-smoke-fixture");
        std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
        std::fs::write(
            path.join("manifest.json"),
            r#"{"manifest_version":3,"name":"ELIX native extension test","version":"1.0"}"#,
        )
        .map_err(|e| e.to_string())?;
        let installed = execute(app.clone(), Operation::Add(path.clone())).await?;
        let id = installed["id"]
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or("native extension install returned no id")?
            .to_string();
        let result = async {
            let listed = execute(app.clone(), Operation::List).await?;
            if !listed.as_array().is_some_and(|list| {
                list.iter()
                    .any(|item| item["id"] == id && item["enabled"] == true)
            }) {
                return Err("native extension missing after install".into());
            }
            execute(
                app.clone(),
                Operation::Enable {
                    id: id.clone(),
                    enabled: false,
                },
            )
            .await?;
            let listed = execute(app.clone(), Operation::List).await?;
            if !listed.as_array().is_some_and(|list| {
                list.iter()
                    .any(|item| item["id"] == id && item["enabled"] == false)
            }) {
                return Err("native extension disable failed".into());
            }
            execute(
                app.clone(),
                Operation::Enable {
                    id: id.clone(),
                    enabled: true,
                },
            )
            .await?;
            let listed = execute(app.clone(), Operation::List).await?;
            if !listed.as_array().is_some_and(|list| {
                list.iter()
                    .any(|item| item["id"] == id && item["enabled"] == true)
            }) {
                return Err("native extension enable failed".into());
            }
            Ok::<(), String>(())
        }
        .await;
        let removed = execute(app.clone(), Operation::Remove { id: id.clone() }).await;
        if removed.is_ok() {
            let _ = std::fs::remove_dir_all(path);
        }
        result?;
        removed?;
        let listed = execute(app, Operation::List).await?;
        if listed
            .as_array()
            .is_some_and(|list| list.iter().any(|item| item["id"] == id))
        {
            return Err("native extension remove failed".into());
        }
        Ok(())
    }
}
