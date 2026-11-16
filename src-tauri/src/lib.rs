mod browser;
mod developer;
mod diagnostics;
mod downloads;
mod profiles;
mod security;
mod storage;
mod updater;
#[cfg(feature = "smoke-tests")]
mod upgrade_test;
mod window;
use serde::Deserialize;
use serde_json::{json, Value};
use std::{path::PathBuf, sync::Mutex};
use tauri::{Emitter, Manager};
use tauri_plugin_dialog::DialogExt;

pub struct AppState {
    root: PathBuf,
    download_root: PathBuf,
    window_id: String,
    recovered: bool,
    permissions_visible: std::sync::atomic::AtomicBool,
    store: Mutex<storage::Store>,
    profile: Mutex<String>,
    session: Mutex<browser::Session>,
    bounds: Mutex<browser::Bounds>,
    metrics: Mutex<developer::Metrics>,
    runtime_version: Mutex<Option<String>>,
    downloads: Mutex<Vec<downloads::Download>>,
    logs: diagnostics::Logs,
    updates: updater::Updates,
}
#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
enum Request {
    Snapshot,
    WindowReady,
    ReopenTab,
    RecoverWindows,
    ReorderTab {
        id: String,
        before: String,
    },
    DetachTab {
        id: String,
        #[serde(default)]
        outside_only: bool,
    },
    Workspace {
        name: String,
    },
    GroupTab {
        id: String,
        group: String,
        workspace: String,
    },
    Split {
        other: Option<String>,
    },
    MuteTab {
        id: String,
        muted: bool,
    },
    RecoverTab {
        id: String,
    },
    ImportBookmarks {
        items: Vec<storage::Entry>,
    },
    Cookies {
        origin: String,
        #[serde(default)]
        delete: bool,
    },
    SavedPermissions {
        origin: String,
    },
    SavePermission {
        origin: String,
        kind: String,
        decision: String,
    },
    DefaultBrowser,
    Account {
        profile: String,
        operation: String,
        #[serde(default)]
        payload: Value,
    },
    SyncExport {
        profile: String,
    },
    SyncApply {
        profile: String,
        items: Vec<storage::Entry>,
        appearance: storage::Appearance,
    },
    Window {
        operation: window::Action,
    },
    NewTab {
        url: String,
        #[serde(default)]
        private: bool,
    },
    PinTab {
        id: String,
        pinned: bool,
    },
    Activate {
        id: String,
    },
    Navigate {
        id: String,
        url: String,
    },
    CloseTab {
        id: String,
    },
    Suspend {
        id: String,
    },
    Bounds {
        bounds: browser::Bounds,
    },
    Engine {
        id: String,
        operation: String,
    },
    ListEntries {
        kind: String,
    },
    Bookmark {
        id: String,
    },
    RemoveBookmark {
        id: i64,
    },
    ClearHistory,
    Settings {
        settings: storage::Settings,
    },
    CreateProfile {
        name: String,
    },
    SwitchProfile {
        id: String,
    },
    Extensions,
    AddExtension,
    EnableExtension {
        id: String,
        enabled: bool,
    },
    RemoveExtension {
        id: String,
    },
    ClearPasswords {
        consent: bool,
    },
    SitePermissions,
    RespondPermission {
        id: String,
        allow: bool,
    },
    Downloads,
    ShowDownload {
        id: String,
    },
    DownloadAction {
        id: String,
        operation: String,
    },
    Diagnostics,
    ExportDiagnostics {
        consent: bool,
    },
    CheckUpdate,
    DownloadUpdate,
    InstallUpdate,
}
fn authorize(view: &tauri::Webview) -> Result<(), String> {
    if view.label() != "shell" {
        return Err("Forbidden".into());
    }
    let url = view.url().map_err(|_| "Forbidden")?;
    let local = matches!(
        (url.scheme(), url.host_str()),
        ("tauri", Some("localhost"))
            | ("http", Some("tauri.localhost"))
            | ("https", Some("tauri.localhost"))
    );
    let dev = cfg!(debug_assertions)
        && url.scheme() == "http"
        && url.host_str() == Some("127.0.0.1")
        && url.port() == Some(1420);
    if !local && !dev {
        return Err("Forbidden".into());
    }
    Ok(())
}
fn system_info(app: &tauri::AppHandle) -> Value {
    let state = app.state::<AppState>();
    json!({"version":app.package_info().version.to_string(),"backend_version":env!("CARGO_PKG_VERSION"),"tauri":"2.12.5","webview2":state.runtime_version.lock().unwrap().clone(),"os":sysinfo::System::long_os_version(),"arch":std::env::consts::ARCH,"windows":app.windows().len(),"tabs":state.session.lock().unwrap().tabs.len()})
}
#[tauri::command]
async fn dispatch(
    app: tauri::AppHandle,
    webview: tauri::Webview,
    request: Request,
) -> Result<Value, String> {
    authorize(&webview)?;
    let state = app.state::<AppState>();
    match request {
        Request::Account {
            profile,
            operation,
            payload,
        } => profiles::accounts::request(app.clone(), profile, operation, payload).await,
        Request::SyncExport { profile } => {
            if *state.profile.lock().unwrap() != profile {
                return Err("Profilen ble endret".into());
            }
            let p = state.profile.lock().unwrap().clone();
            let store = state.store.lock().unwrap();
            let mut items = store.entries(&p, "bookmarks")?;
            if items.len() > 5000 {
                return Err("Maksimalt 5000 bokmerker for synkronisering".into());
            }
            for item in &mut items {
                item.favicon = None;
            }
            Ok(json!({"version":1,"items":items,"appearance":store.settings().appearance}))
        }
        Request::SyncApply {
            profile,
            items,
            appearance,
        } => {
            if *state.profile.lock().unwrap() != profile {
                return Err("Profilen ble endret".into());
            }
            let p = state.profile.lock().unwrap().clone();
            let store = state.store.lock().unwrap();
            let mut settings = store.settings();
            settings.appearance = appearance;
            if !settings.valid() {
                return Err("Ugyldig synkronisert utseende".into());
            }
            let count = store.import_bookmarks(&p, items)?;
            store.set(
                "preferences",
                &serde_json::to_string(&settings).map_err(|e| e.to_string())?,
            )?;
            Ok(json!(count))
        }
        Request::Window { operation } => window::perform(&app, operation),
        Request::WindowReady => {
            browser::detach::ready(&app);
            Ok(Value::Null)
        }
        Request::RecoverWindows => Ok(json!(browser::detach::recover_windows(&app)?)),
        Request::ReopenTab => {
            browser::organization::reopen(&app)?;
            Ok(Value::Null)
        }
        Request::ReorderTab { id, before } => {
            browser::organization::reorder(&app, &id, &before)?;
            Ok(Value::Null)
        }
        Request::DetachTab { id, outside_only } => Ok(json!(
            browser::detach::detach(app.clone(), id, outside_only).await?
        )),
        Request::Workspace { name } => {
            browser::organization::workspace(&app, &name)?;
            Ok(Value::Null)
        }
        Request::GroupTab {
            id,
            group,
            workspace,
        } => {
            browser::organization::group(&app, &id, &group, &workspace)?;
            Ok(Value::Null)
        }
        Request::Split { other } => {
            browser::organization::split(&app, other)?;
            Ok(Value::Null)
        }
        Request::MuteTab { id, muted } => {
            browser::organization::mute(&app, &id, muted).await?;
            Ok(Value::Null)
        }
        Request::RecoverTab { id } => {
            browser::organization::recover(&app, &id)?;
            Ok(Value::Null)
        }
        Request::Cookies { origin, delete } => {
            browser::site_data::cookies(&app, origin, delete).await
        }
        Request::SavedPermissions { origin } => {
            let p = state.profile.lock().unwrap().clone();
            let store = state.store.lock().unwrap();
            Ok(json!(["camera", "microphone", "location", "notifications"]
                .iter()
                .map(|kind| json!({"kind":kind,"decision":store.permission(&p,&origin,kind)}))
                .collect::<Vec<_>>()))
        }
        Request::SavePermission {
            origin,
            kind,
            decision,
        } => {
            let p = state.profile.lock().unwrap().clone();
            state
                .store
                .lock()
                .unwrap()
                .set_permission(&p, &origin, &kind, &decision)?;
            Ok(Value::Null)
        }
        Request::DefaultBrowser => {
            #[cfg(windows)]
            std::process::Command::new("explorer.exe")
                .arg("ms-settings:defaultapps?registeredAppUser=ELIX%20Browser")
                .spawn()
                .map_err(|e| e.to_string())?;
            #[cfg(not(windows))]
            return Err("Krever Windows".into());
            #[allow(unreachable_code)]
            Ok(Value::Null)
        }
        Request::ImportBookmarks { items } => {
            let p = state.profile.lock().unwrap().clone();
            let count = state.store.lock().unwrap().import_bookmarks(&p, items)?;
            Ok(json!(count))
        }
        Request::Snapshot => {
            let store = state.store.lock().unwrap();
            Ok(
                json!({"session":state.session.lock().unwrap().clone(),"profiles":store.profiles()?,"profile":state.profile.lock().unwrap().clone(),"settings":store.settings(),"version":env!("CARGO_PKG_VERSION"),"updater_configured":updater::configured(),"recovered":state.recovered}),
            )
        }
        Request::NewTab { url, private } => Ok(json!(
            browser::create_with_mode(app.clone(), url, private).await?
        )),
        Request::PinTab { id, pinned } => {
            browser::pin(&app, &id, pinned)?;
            Ok(Value::Null)
        }
        Request::Activate { id } => {
            browser::activate(&app, &id)?;
            browser::persist(&app)?;
            Ok(Value::Null)
        }
        Request::Navigate { id, url } => {
            browser::navigate(&app, &id, &url)?;
            Ok(Value::Null)
        }
        Request::CloseTab { id } => {
            browser::close(&app, &id)?;
            Ok(Value::Null)
        }
        Request::Suspend { id } => {
            browser::suspend(&app, &id)?;
            Ok(Value::Null)
        }
        Request::Bounds { bounds } => {
            *state.bounds.lock().unwrap() = bounds;
            browser::organization::layout(&app)?;
            Ok(Value::Null)
        }
        Request::Engine { id, operation } => {
            if !["back", "forward", "reload", "stop", "devtools"].contains(&operation.as_str()) {
                return Err("Ukjent handling".into());
            }
            if operation == "reload"
                && state
                    .session
                    .lock()
                    .unwrap()
                    .tabs
                    .iter()
                    .any(|t| t.id == id && t.crashed)
            {
                browser::organization::recover(&app, &id)?;
                return Ok(Value::Null);
            }
            let dev = state.store.lock().unwrap().settings().developer;
            if operation == "devtools" && !dev {
                return Err("Aktiver utviklermodus i Innstillinger → Utvikler".into());
            }
            let view = app
                .get_webview(&id)
                .filter(|v| v.label().starts_with("tab-"))
                .ok_or("Åpne en nettside først")?;
            browser::engine::action(view, operation, dev).await?;
            Ok(Value::Null)
        }
        Request::ListEntries { kind } => {
            let p = state.profile.lock().unwrap().clone();
            let rows = state.store.lock().unwrap().entries(&p, &kind)?;
            Ok(json!(rows))
        }
        Request::Bookmark { id } => {
            let tab = state
                .session
                .lock()
                .unwrap()
                .tabs
                .iter()
                .find(|t| t.id == id)
                .cloned()
                .ok_or("Fant ikke fanen")?;
            if tab.url.starts_with("elix:") {
                return Err("Åpne en nettside først".into());
            };
            let p = state.profile.lock().unwrap().clone();
            state
                .store
                .lock()
                .unwrap()
                .bookmark(&p, &tab.url, &tab.title)?;
            state
                .store
                .lock()
                .unwrap()
                .bookmark_icon(&p, &tab.url, tab.favicon.as_deref())?;
            Ok(Value::Null)
        }
        Request::RemoveBookmark { id } => {
            let p = state.profile.lock().unwrap().clone();
            state.store.lock().unwrap().remove_bookmark(&p, id)?;
            Ok(Value::Null)
        }
        Request::ClearHistory => {
            let p = state.profile.lock().unwrap().clone();
            state.store.lock().unwrap().clear_history(&p)?;
            Ok(Value::Null)
        }
        Request::Settings { settings } => {
            if !settings.valid() {
                return Err("Ugyldige innstillinger".into());
            }
            let previous = state.store.lock().unwrap().settings();
            let channel_changed = previous.channel != settings.channel;
            if channel_changed && state.updates.busy.load(std::sync::atomic::Ordering::SeqCst) {
                return Err("Vent til oppdateringsoperasjonen er ferdig".into());
            }
            if previous.developer != settings.developer {
                for (id, v) in app.webviews() {
                    if id.starts_with("tab-") {
                        browser::engine::action(v, "set_developer".into(), settings.developer)
                            .await?;
                    }
                }
            }
            if previous.password_autosave != settings.password_autosave
                || previous.autofill != settings.autofill
            {
                let tabs = state.session.lock().unwrap().tabs.clone();
                for tab in tabs {
                    if let Some(view) = app.get_webview(&tab.id) {
                        browser::extensions::apply_preferences(
                            view,
                            settings.password_autosave,
                            settings.autofill,
                            tab.private,
                        )
                        .await?;
                    }
                }
            }
            if channel_changed {
                *state.updates.pending.lock().unwrap() = None;
                *state.updates.bytes.lock().unwrap() = None;
            }
            state.store.lock().unwrap().set(
                "preferences",
                &serde_json::to_string(&settings).map_err(|e| e.to_string())?,
            )?;
            Ok(Value::Null)
        }
        Request::CreateProfile { name } => {
            let p = state.store.lock().unwrap().create_profile(&name)?;
            Ok(json!(p))
        }
        Request::SwitchProfile { id } => {
            if !security::valid_profile(&id)
                || !state
                    .store
                    .lock()
                    .unwrap()
                    .profiles()?
                    .iter()
                    .any(|p| p.id == id)
            {
                return Err("Ukjent profil".into());
            }
            browser::persist(&app)?;
            #[cfg(windows)]
            security::permissions::cancel_all(&app);
            downloads::persist(&app);
            downloads::clear_private(&app);
            for (label, v) in app.webviews() {
                if label.starts_with("tab-") {
                    v.close().map_err(|e| e.to_string())?;
                }
            }
            browser::private::cleanup();
            state.store.lock().unwrap().set("active_profile", &id)?;
            *state.profile.lock().unwrap() = id;
            browser::restore(&app);
            let id = state.session.lock().unwrap().active.clone();
            browser::activate(&app, &id)?;
            Ok(Value::Null)
        }
        Request::Extensions => {
            browser::extensions::execute(app.clone(), browser::extensions::Operation::List).await
        }
        Request::AddExtension => browser::extensions::pick_and_add(app.clone()).await,
        Request::EnableExtension { id, enabled } => {
            browser::extensions::execute(
                app.clone(),
                browser::extensions::Operation::Enable { id, enabled },
            )
            .await
        }
        Request::RemoveExtension { id } => {
            browser::extensions::execute(app.clone(), browser::extensions::Operation::Remove { id })
                .await
        }
        Request::ClearPasswords { consent } => {
            if !consent {
                return Err("Sletting av passord krever bekreftelse".into());
            }
            browser::extensions::execute(
                app.clone(),
                browser::extensions::Operation::ClearPasswords,
            )
            .await
        }
        Request::SitePermissions => {
            #[cfg(windows)]
            {
                return Ok(json!(security::permissions::list(&app).await?));
            }
            #[cfg(not(windows))]
            {
                Ok(json!([]))
            }
        }
        Request::RespondPermission { id, allow } => {
            #[cfg(windows)]
            {
                security::permissions::respond(&app, &id, allow).await?;
            }
            #[cfg(not(windows))]
            {
                let _ = (id, allow);
            }
            Ok(Value::Null)
        }
        Request::Downloads => Ok(json!(downloads::list(&app))),
        Request::DownloadAction { id, operation } => {
            downloads::action(&app, &id, &operation).await?;
            Ok(Value::Null)
        }
        Request::ShowDownload { id } => {
            let profile = state.profile.lock().unwrap().clone();
            let path = state
                .downloads
                .lock()
                .unwrap()
                .iter()
                .find(|d| d.id == id && d.profile == profile)
                .map(|d| d.path.clone())
                .ok_or("Ukjent nedlasting")?;
            #[cfg(windows)]
            {
                std::process::Command::new("explorer.exe")
                    .arg(path.parent().ok_or("Ugyldig mappe")?)
                    .spawn()
                    .map_err(|e| e.to_string())?;
            }
            #[cfg(not(windows))]
            {
                let _ = path;
                return Err("Krever Windows".into());
            }
            #[allow(unreachable_code)]
            Ok(Value::Null)
        }
        Request::Diagnostics => {
            if !state.store.lock().unwrap().settings().developer {
                return Err("Utviklermodus er av".into());
            }
            Ok(
                json!({"system":system_info(&app),"samples":state.metrics.lock().unwrap().samples,"logs":state.logs.list(),"network":state.session.lock().unwrap().tabs,"unavailable":["DNS-tid","TLS-detaljer","Overførte byte","Cache-aktivitet","CPU/RAM per fane","Alle nettverksforespørsler"],"crash_reports":"Prosessfeil loggføres lokalt. Minidumper samles ikke."}),
            )
        }
        Request::ExportDiagnostics { consent } => {
            if !consent {
                return Err("Eksport krever samtykke".into());
            }
            let report = json!({"system":system_info(&app),"samples":state.metrics.lock().unwrap().samples,"logs":state.logs.list(),"privacy":"Ingen URL-er, profilnavn, historikk, cookies eller lokale stier inkludert."});
            let app2 = app.clone();
            tauri::async_runtime::spawn_blocking(move || -> Result<Value, String> {
                if let Some(path) = app2
                    .dialog()
                    .file()
                    .set_file_name("elix-diagnostikk.json")
                    .add_filter("JSON", &["json"])
                    .blocking_save_file()
                {
                    let path = path.into_path().map_err(|e| e.to_string())?;
                    std::fs::write(
                        path,
                        serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?,
                    )
                    .map_err(|e| e.to_string())?;
                    Ok(json!(true))
                } else {
                    Ok(json!(false))
                }
            })
            .await
            .map_err(|e| e.to_string())?
        }
        Request::CheckUpdate => updater::check(&app).await,
        Request::DownloadUpdate => updater::download(&app).await,
        Request::InstallUpdate => updater::install(&app),
    }
}
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![dispatch])
        .setup(|app| {
            let root = app.path().app_local_data_dir()?;
            #[cfg(feature = "smoke-tests")]
            let root = if std::env::args().any(|a| a == "--self-test") {
                std::env::temp_dir().join(format!("elix-smoke-{}", uuid::Uuid::new_v4()))
            } else if std::env::args().any(|a| a == "--upgrade-self-test") {
                std::env::temp_dir().join("elix-upgrade-self-test")
            } else if std::env::args().any(|a| a == "--elix-smoke-window") {
                browser::smoke::detached_root().map_err(std::io::Error::other)?
            } else {
                root
            };
            #[cfg(feature = "smoke-tests")]
            if std::env::args().any(|a| a == "--elix-smoke-reject-window") {
                return Err(
                    std::io::Error::other("Deliberate smoke-test startup rejection").into(),
                );
            }
            std::fs::create_dir_all(&root)?;
            let store =
                storage::Store::open(&root.join("elix.sqlite")).map_err(std::io::Error::other)?;
            let (window_id, handoff) = browser::detach::startup().map_err(std::io::Error::other)?;
            let window_id =
                browser::detach::claim_window(&store, window_id).map_err(std::io::Error::other)?;
            let mut profile = store
                .get("active_profile")
                .filter(|id| {
                    store
                        .profiles()
                        .is_ok_and(|ps| ps.iter().any(|p| &p.id == id))
                })
                .unwrap_or(
                    store.profiles().map_err(std::io::Error::other)?[0]
                        .id
                        .clone(),
                );
            if let Some(h) = &handoff {
                if !h.profile.is_empty()
                    && store
                        .profiles()
                        .map_err(std::io::Error::other)?
                        .iter()
                        .any(|p| p.id == h.profile)
                {
                    profile = h.profile.clone();
                }
            }
            let recovered = store.get(&format!("dirty:{window_id}")).as_deref() == Some("true");
            store
                .set(&format!("dirty:{window_id}"), "true")
                .map_err(std::io::Error::other)?;
            let download_root = if window_id == "main" {
                root.clone()
            } else {
                root.join("windows").join(&window_id)
            };
            std::fs::create_dir_all(&download_root)?;
            let downloads =
                downloads::history::load(&download_root).map_err(std::io::Error::other)?;
            app.manage(AppState {
                root,
                download_root,
                window_id,
                recovered,
                permissions_visible: std::sync::atomic::AtomicBool::new(false),
                store: Mutex::new(store),
                profile: Mutex::new(profile),
                session: Mutex::new(browser::Session::default()),
                bounds: Mutex::new(browser::Bounds::default()),
                metrics: Mutex::new(developer::Metrics::default()),
                runtime_version: Mutex::new(None),
                downloads: Mutex::new(downloads),
                logs: diagnostics::Logs::default(),
                updates: updater::Updates::default(),
            });
            let window = tauri::window::WindowBuilder::new(app, "main")
                .title("ELIX Browser")
                .decorations(false)
                .shadow(true)
                .theme(Some(tauri::Theme::Dark))
                .inner_size(1000., 700.)
                .min_inner_size(900., 600.)
                .maximized(true)
                .build()?;
            let shell = tauri::webview::WebviewBuilder::new(
                "shell",
                tauri::WebviewUrl::App("index.html".into()),
            )
            .devtools(cfg!(debug_assertions))
            .on_navigation(|url| {
                matches!(
                    (url.scheme(), url.host_str()),
                    ("tauri", Some("localhost"))
                        | ("http", Some("tauri.localhost"))
                        | ("https", Some("tauri.localhost"))
                ) || (cfg!(debug_assertions)
                    && url.host_str() == Some("127.0.0.1")
                    && url.port() == Some(1420))
            });
            let shell = window.add_child(
                shell,
                tauri::LogicalPosition::new(0., 0.),
                window.inner_size()?,
            )?;
            shell.set_auto_resize(true)?;
            browser::restore(app.handle());
            if let Some(mut handoff) = handoff {
                handoff.tab.id = format!("tab-{}", uuid::Uuid::new_v4());
                handoff.tab.loading = false;
                handoff.tab.suspended = false;
                let workspace = handoff.tab.workspace.clone();
                let active = handoff.tab.id.clone();
                *app.state::<AppState>().session.lock().unwrap() = browser::Session {
                    tabs: vec![handoff.tab],
                    active,
                    workspace: workspace.clone(),
                    workspaces: vec![workspace],
                    ..Default::default()
                };
                window.unmaximize()?;
                window.set_position(tauri::PhysicalPosition::new(handoff.x, handoff.y))?;
            }
            #[cfg(feature = "smoke-tests")]
            if std::env::args().any(|a| a == "--self-test") {
                browser::smoke::start(app.handle().clone());
            }
            #[cfg(feature = "smoke-tests")]
            if std::env::args().any(|a| a == "--elix-smoke-window") {
                browser::smoke::start_detached(app.handle().clone());
            }
            #[cfg(feature = "smoke-tests")]
            if std::env::args().any(|a| a == "--upgrade-self-test") {
                upgrade_test::start(app.handle().clone());
            }
            app.state::<AppState>()
                .logs
                .add("info", "application_started");
            let handle = app.handle().clone();
            std::thread::spawn(move || loop {
                std::thread::sleep(std::time::Duration::from_secs(5));
                let state = handle.state::<AppState>();
                let sample = state.metrics.lock().unwrap().sample();
                let _ = handle.emit_to("shell", "metrics", &sample);
                let minutes = state.store.lock().unwrap().settings().suspend_minutes;
                if minutes > 0 {
                    let ids = {
                        let s = state.session.lock().unwrap();
                        s.tabs
                            .iter()
                            .filter(|t| {
                                t.id != s.active
                                    && !t.suspended
                                    && !t.private
                                    && !state.downloads.lock().unwrap().iter().any(|d| {
                                        d.view_id == t.id
                                            && (d.can_resume
                                                || ["downloading", "paused"]
                                                    .contains(&d.status.as_str()))
                                    })
                                    && !t.url.starts_with("elix:")
                                    && chrono::Utc::now().timestamp() - t.last_active
                                        > minutes as i64 * 60
                            })
                            .map(|t| t.id.clone())
                            .collect::<Vec<_>>()
                    };
                    for id in ids {
                        let _ = browser::suspend(&handle, &id);
                    }
                }
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::Resized(_)) {
                window::emit(window.app_handle());
            }
            if matches!(event, tauri::WindowEvent::CloseRequested { .. }) {
                let _ = browser::persist(window.app_handle());
                downloads::persist(window.app_handle());
                #[cfg(windows)]
                security::permissions::cancel_all(window.app_handle());
                for (label, view) in window.app_handle().webviews() {
                    if label.starts_with("tab-") {
                        let _ = view.close();
                    }
                }
                browser::private::cleanup();
                let state = window.app_handle().state::<AppState>();
                let _ = state
                    .store
                    .lock()
                    .unwrap()
                    .set(&format!("window_pid:{}", state.window_id), "0");
                let _ = state
                    .store
                    .lock()
                    .unwrap()
                    .set(&format!("dirty:{}", state.window_id), "false");
            }
        })
        .run(tauri::generate_context!())
        .expect("ELIX kunne ikke starte");
}
