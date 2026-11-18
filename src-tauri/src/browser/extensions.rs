//! Native, profile-scoped extension and password controls. No credentials leave WebView2.
use crate::AppState;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager, Webview};
#[path = "extension_package.rs"]
mod package;

#[derive(Clone)]
pub enum Operation {
    List,
    Add { path: std::path::PathBuf, profile: String },
    Enable { id: String, enabled: bool },
    Remove { id: String },
    ClearPasswords { profile: String },
}

/// Native pickers are the only source of paths. ZIP imports are publisher source,
/// not a replacement for the Chrome Web Store's signed CRX installation flow.
pub async fn pick_and_add(app: AppHandle, package: bool) -> Result<Value, String> {
    use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};
    let selected_profile = app.state::<AppState>().profile.lock().unwrap().clone();
    let picker = app.clone();
    let selected = tauri::async_runtime::spawn_blocking(move || {
        if package {
            picker.dialog().file().set_title("Velg ZIP med utvidelsens kildefiler fra utgiveren")
                .add_filter("Utvidelsens kildefiler (ZIP)", &["zip"]).blocking_pick_file()
        } else {
            picker.dialog().file().set_title("Velg utvidelsesmappe med manifest.json").blocking_pick_folder()
        }
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
    uuid::Uuid::parse_str(&selected_profile).map_err(|_| "Ugyldig profil.")?;
    let managed = app.state::<AppState>().root.join("extension-sources").join(&selected_profile);
    let mut prepared = tauri::async_runtime::spawn_blocking(move || {
        if package { package::prepare_zip(&path, &managed) } else { package::prepare_folder(path) }
    }).await.map_err(|e| e.to_string())??;
    let review = &prepared.review;
    let permissions = if review.permissions.is_empty() { "Ingen ekstra tillatelser oppgitt".to_string() } else { review.permissions.join(", ") };
    let optional = if review.optional_permissions.is_empty() { "Ingen".to_string() } else { review.optional_permissions.join(", ") };
    let message = format!("{} · {}\n\nOppgitte tillatelser og nettsider:\n{}\n\nValgfrie tillatelser:\n{}\n\nInstaller bare kildefiler fra en utgiver du stoler på. Tilgang kan omfatte innhold på nettsidene dine. ELIX bekrefter ikke butikkens signatur eller utgiveridentitet for denne lokale installasjonen. Utvidelsen installeres i den valgte profilen og er av i private faner.", review.name, review.version, permissions, optional);
    let dialog = app.clone();
    let approved = tauri::async_runtime::spawn_blocking(move || dialog.dialog().message(message)
        .title("Installer lokal utvidelse")
        .buttons(MessageDialogButtons::OkCancelCustom("Installer".into(), "Avbryt".into()))
        .blocking_show()).await.map_err(|e| e.to_string())?;
    if !approved { return Ok(Value::Null); }
    if *app.state::<AppState>().profile.lock().unwrap() != selected_profile {
        return Err("Profilen ble endret. Velg utvidelsen på nytt i riktig profil.".into());
    }
    let path = prepared.path.clone();
    prepared.retain_source();
    execute(app, Operation::Add { path, profile: selected_profile }).await
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

/// Fail closed before the first private navigation if the runtime did not apply
/// the profile isolation and autofill restrictions requested by ELIX.
#[cfg(windows)]
pub unsafe fn ensure_private_preferences(
    core: &webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2,
) -> windows::core::Result<()> {
    native::ensure_private_preferences(core)
}

#[cfg(all(windows, feature = "smoke-tests"))]
pub async fn verify_private_preferences(view: &Webview) -> Result<(), String> {
    native::verify_private_preferences(view).await
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
        // These two settings belong to the profile, not to an individual tab.
        // Use the profile API directly and derive privacy from the actual native
        // profile as well as the caller, so opt-in never enables a private view.
        let profile = core.cast::<ICoreWebView2_13>()?.Profile()?;
        let mut native_private = windows::core::BOOL(0);
        profile.IsInPrivateModeEnabled(&mut native_private)?;
        let profile = profile.cast::<ICoreWebView2Profile6>()?;
        let allow = !private && !native_private.as_bool();
        profile.SetIsPasswordAutosaveEnabled(autosave && allow)?;
        profile.SetIsGeneralAutofillEnabled(autofill && allow)
    }

    #[derive(Clone, Copy, Debug)]
    struct PreferencesSnapshot {
        in_private: bool,
        settings_autosave: bool,
        settings_autofill: bool,
        profile_autosave: bool,
        profile_autofill: bool,
    }

    impl PreferencesSnapshot {
        fn matches(&self, autosave: bool, autofill: bool) -> bool {
            self.settings_autosave == autosave
                && self.settings_autofill == autofill
                && self.profile_autosave == autosave
                && self.profile_autofill == autofill
        }
    }

    unsafe fn snapshot(core: &ICoreWebView2) -> windows::core::Result<PreferencesSnapshot> {
        let profile = core.cast::<ICoreWebView2_13>()?.Profile()?;
        let mut in_private = windows::core::BOOL(0);
        profile.IsInPrivateModeEnabled(&mut in_private)?;
        let profile = profile.cast::<ICoreWebView2Profile6>()?;
        let settings = core.Settings()?.cast::<ICoreWebView2Settings4>()?;
        let mut settings_autosave = windows::core::BOOL(0);
        let mut settings_autofill = windows::core::BOOL(0);
        let mut profile_autosave = windows::core::BOOL(0);
        let mut profile_autofill = windows::core::BOOL(0);
        settings.IsPasswordAutosaveEnabled(&mut settings_autosave)?;
        settings.IsGeneralAutofillEnabled(&mut settings_autofill)?;
        profile.IsPasswordAutosaveEnabled(&mut profile_autosave)?;
        profile.IsGeneralAutofillEnabled(&mut profile_autofill)?;
        Ok(PreferencesSnapshot {
            in_private: in_private.as_bool(),
            settings_autosave: settings_autosave.as_bool(),
            settings_autofill: settings_autofill.as_bool(),
            profile_autosave: profile_autosave.as_bool(),
            profile_autofill: profile_autofill.as_bool(),
        })
    }

    pub unsafe fn ensure_private_preferences(core: &ICoreWebView2) -> windows::core::Result<()> {
        let observed = snapshot(core)?;
        if !observed.in_private || !observed.matches(false, false) {
            return Err(windows::core::Error::new(
                windows::core::HRESULT(0x80004005u32 as i32),
                format!("Private profile protection unavailable: {observed:?}"),
            ));
        }
        Ok(())
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
    pub(super) fn profile_view(app: &AppHandle) -> Result<(Webview, bool), String> {
        profile_view_for(app, None)
    }

    fn profile_view_for(app: &AppHandle, target_profile: Option<&str>) -> Result<(Webview, bool), String> {
        let state = app.state::<AppState>();
        let existing = target_profile.is_none().then(|| state
            .session
            .lock()
            .unwrap()
            .tabs
            .iter()
            .filter(|tab| !tab.private)
            .find_map(|tab| app.get_webview(&tab.id))).flatten();
        if let Some(view) = existing {
            return Ok((view, false));
        }
        let profile = target_profile.map(str::to_string).unwrap_or_else(|| state.profile.lock().unwrap().clone());
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
        #[cfg(feature = "smoke-tests")]
        let builder = if std::env::args().any(|arg| arg == "--self-test" || arg == "--elix-smoke-window") {
            builder.additional_browser_args("--use-fake-device-for-media-stream")
        } else { builder };
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
        let target_profile = match &operation {
            Operation::Add { profile, .. } | Operation::ClearPasswords { profile } => Some(profile.as_str()),
            _ => None,
        };
        let (view, temporary) = profile_view_for(&app, target_profile)?;
        let (tx, rx) = std::sync::mpsc::channel();
        let started = view.with_webview(move |platform| {
            let result = (|| -> windows::core::Result<()> { unsafe {
                let core = platform.controller().CoreWebView2()?;
                let profile = core.cast::<ICoreWebView2_13>()?.Profile()?;
                match operation {
                    Operation::ClearPasswords { .. } => {
                        let callback_tx = tx.clone();
                        profile.cast::<ICoreWebView2Profile2>()?.ClearBrowsingData(
                            COREWEBVIEW2_BROWSING_DATA_KINDS_PASSWORD_AUTOSAVE,
                            &ClearBrowsingDataCompletedHandler::create(Box::new(move |result| {
                                finish(&callback_tx, result.map(|_| Value::Null)); Ok(())
                            })),
                        )?;
                    }
                    Operation::Add { path, .. } => {
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
    async fn preferences_snapshot(view: &Webview) -> Result<PreferencesSnapshot, String> {
        let (tx, rx) = std::sync::mpsc::channel();
        view.with_webview(move |platform| {
            let result = (|| -> windows::core::Result<PreferencesSnapshot> {
                unsafe { snapshot(&platform.controller().CoreWebView2()?) }
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
    async fn expect_preferences(
        view: &Webview,
        autosave: bool,
        autofill: bool,
        phase: &str,
    ) -> Result<(), String> {
        let first = preferences_snapshot(view).await?;
        let mut observed = first;
        // Do not write repeatedly or navigate to make this pass. Read both APIs
        // through a few event-loop turns; report every final value if they do not
        // converge to the requested profile state.
        for _ in 0..20 {
            if observed.matches(autosave, autofill) {
                return Ok(());
            }
            tauri::async_runtime::spawn_blocking(|| {
                std::thread::sleep(std::time::Duration::from_millis(100))
            })
            .await
            .map_err(|e| e.to_string())?;
            observed = preferences_snapshot(view).await?;
        }
        Err(format!("{phase}: expected autosave={autosave}, autofill={autofill}; initial={first:?}; final={observed:?}"))
    }

    #[cfg(feature = "smoke-tests")]
    pub async fn verify_private_preferences(view: &Webview) -> Result<(), String> {
        let before = preferences_snapshot(view).await?;
        if !before.in_private || !before.matches(false, false) {
            return Err(format!(
                "Actual private view not protected before use: {before:?}"
            ));
        }
        // Try the user's opt-ins on a real private profile. Native privacy wins.
        apply_preferences(view.clone(), true, true, true).await?;
        let after = preferences_snapshot(view).await?;
        if !after.in_private || !after.matches(false, false) {
            return Err(format!(
                "Actual private view accepted autofill opt-in: before={before:?}; after={after:?}"
            ));
        }
        Ok(())
    }

    #[cfg(feature = "smoke-tests")]
    pub async fn smoke(app: AppHandle) -> Result<(), String> {
        let (view, temporary) = profile_view(&app)?;
        let before = preferences_snapshot(&view).await?;
        let preferences_result = async {
            apply_preferences(view.clone(), true, true, false).await?;
            expect_preferences(&view, true, true, "Regular profile autofill opt-in failed").await?;
            apply_preferences(view.clone(), false, false, false).await?;
            expect_preferences(
                &view,
                false,
                false,
                "Regular profile autofill opt-out failed",
            )
            .await?;
            Ok::<(), String>(())
        }
        .await;
        let restore = apply_preferences(
            view.clone(),
            before.profile_autosave,
            before.profile_autofill,
            false,
        )
        .await;
        if temporary {
            let _ = view.close();
        }
        preferences_result?;
        restore?;

        // Exercise the publisher ZIP path before the real WebView2 install;
        // this catches a source path that disappears after preparation.
        use std::io::{Cursor, Write};
        let root = app.state::<AppState>().root.join("extension-smoke-fixture");
        std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
        let mut archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
        archive.start_file("publisher/manifest.json", zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)).map_err(|e| e.to_string())?;
        archive.write_all(br#"{"manifest_version":3,"name":"ELIX native extension test","version":"1.0"}"#).map_err(|e| e.to_string())?;
        let source = root.join("publisher.zip");
        std::fs::write(&source, archive.finish().map_err(|e| e.to_string())?.into_inner()).map_err(|e| e.to_string())?;
        let mut prepared = package::prepare_zip(&source, &root.join("managed"))?;
        let path = prepared.path.clone();
        prepared.retain_source();
        drop(prepared);
        let profile = app.state::<AppState>().profile.lock().unwrap().clone();
        let installed = execute(app.clone(), Operation::Add { path: path.clone(), profile }).await?;
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
            let _ = std::fs::remove_dir_all(root);
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

#[cfg(windows)]
pub(crate) fn profile_view(app: &AppHandle) -> Result<(Webview, bool), String> {
    native::profile_view(app)
}
