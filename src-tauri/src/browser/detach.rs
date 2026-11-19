//! Windows share profile data, but keep independent tab sessions and download operations.
//! A transferred tab crosses an anonymous pipe, never a command-line URL or temp file.
use crate::{
    browser::{self, Tab},
    AppState,
};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};

fn owner_running(store: &crate::storage::Store, id: &str) -> bool {
    let pid = store
        .get(&format!("window_pid:{id}"))
        .and_then(|v| v.parse::<u32>().ok());
    let Some(pid) = pid else {
        return false;
    };
    let system = sysinfo::System::new_all();
    system
        .process(sysinfo::Pid::from_u32(pid))
        .is_some_and(|p| p.exe() == std::env::current_exe().ok().as_deref())
}

pub fn claim_window(store: &crate::storage::Store, requested: String) -> Result<String, String> {
    let tx = store
        .conn
        .unchecked_transaction()
        .map_err(|e| e.to_string())?;
    let id = if requested == "main" && owner_running(store, "main") {
        uuid::Uuid::new_v4().to_string()
    } else {
        requested
    };
    store.set(&format!("window_pid:{id}"), &std::process::id().to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(id)
}

/// Explicit recovery also makes previously closed detached windows accessible.
pub fn recover_windows(app: &AppHandle) -> Result<usize, String> {
    let state = app.state::<AppState>();
    let profile = state.profile.lock().unwrap().clone();
    let rows = {
        let store = state.store.lock().unwrap();
        let mut q = store
            .conn
            .prepare("SELECT key,value FROM settings WHERE key LIKE 'window_session:%'")
            .map_err(|e| e.to_string())?;
        let rows = q
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        rows.into_iter()
            .filter(|(key, _)| {
                let parts: Vec<_> = key.split(':').collect();
                parts.len() == 3
                    && parts[2] == profile
                    && parts[1] != state.window_id
                    && !owner_running(&store, parts[1])
            })
            .collect::<Vec<_>>()
    };
    let mut tabs = Vec::new();
    let mut consumed = Vec::new();
    for (key, data) in rows {
        let backup_key = key.replacen("window_session:", "window_session_backup:", 1);
        let layout = browser::saved_sessions::decode(&data).or_else(|| {
            let backup = state.store.lock().unwrap().get(&backup_key)?;
            browser::saved_sessions::decode(&backup)
        });
        if let Some(session) = layout {
            tabs.extend(session.tabs);
            consumed.extend([key, backup_key]);
        }
    }
    let count = tabs.len();
    if count == 0 {
        return Ok(0);
    }
    {
        let mut s = state.session.lock().unwrap();
        if s.tabs.len() + count > 100 {
            return Err("Lukk noen faner først. Gjenopprettingen vil overstige 100 faner.".into());
        }
        let now = chrono::Utc::now().timestamp();
        for mut tab in tabs {
            tab.id = format!("tab-{}", uuid::Uuid::new_v4());
            tab.reset_runtime_state(now);
            if !s.workspaces.contains(&tab.workspace) {
                s.workspaces.push(tab.workspace.clone());
            }
            s.tabs.push(tab);
        }
    }
    browser::persist(app)?;
    {
        let store = state.store.lock().unwrap();
        for key in consumed {
            store
                .conn
                .execute("DELETE FROM settings WHERE key=?1", [key])
                .map_err(|e| e.to_string())?;
        }
    }
    browser::emit(app);
    Ok(count)
}
use tauri::{AppHandle, Manager};

#[derive(Serialize, Deserialize)]
pub struct Handoff {
    #[serde(default)]
    pub saved_session: Option<String>,
    pub profile: String,
    pub tab: Tab,
    pub x: i32,
    pub y: i32,
}

pub fn startup() -> Result<(String, Option<Handoff>), String> {
    let args: Vec<_> = std::env::args().collect();
    if let Some(id) = args.iter().find_map(|a| a.strip_prefix("--elix-window=")) {
        if uuid::Uuid::parse_str(id).is_err() {
            return Err("Ugyldig vindu".into());
        }
        let mut bytes = Vec::new();
        std::io::stdin()
            .take(1_000_000)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        let handoff: Handoff =
            serde_json::from_slice(&bytes).map_err(|_| "Kunne ikke overføre fanen")?;
        if !crate::security::valid_profile(&handoff.profile)
            || !crate::security::stored_destination_allowed(&handoff.tab.url)
        {
            return Err("Ugyldig fane".into());
        }
        return Ok((id.into(), Some(handoff)));
    }
    // Links launched by Windows get an independent window rather than overwriting a live session.
    if let Some(i) = args.iter().position(|a| a == "--url") {
        let url = args.get(i + 1).ok_or("Mangler adresse")?;
        let url = crate::security::destination(url)?;
        return Ok((
            uuid::Uuid::new_v4().to_string(),
            Some(Handoff {
                saved_session: None,
                profile: String::new(),
                tab: Tab::new(url),
                x: 100,
                y: 100,
            }),
        ));
    }
    Ok(("main".into(), None))
}

pub fn ready(app: &AppHandle) {
    if app.state::<AppState>().window_id != "main" {
        let _ = std::io::stdout().write_all(b"ELIX_WINDOW_READY\n");
        let _ = std::io::stdout().flush();
    }
}

#[derive(Deserialize)]
pub struct DropPoint { x: f64, y: f64 }

#[cfg(any(windows, test))]
mod pending {
    use std::{collections::HashSet, sync::{LazyLock, Mutex}};
    static IN_FLIGHT: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(Mutex::default);
    pub(super) struct Transfer<'a>(&'a str);
    impl<'a> Transfer<'a> {
        pub(super) fn reserve(id: &'a str) -> Result<Self, String> {
            if IN_FLIGHT.lock().unwrap().insert(id.to_owned()) { Ok(Self(id)) }
            else { Err("Fanen er allerede på vei til et annet vindu.".into()) }
        }
    }
    impl Drop for Transfer<'_> {
        fn drop(&mut self) { IN_FLIGHT.lock().unwrap().remove(self.0); }
    }
}

pub async fn detach(app: AppHandle, id: String, outside_only: bool, drop_point: Option<DropPoint>) -> Result<bool, String> {
    detach_inner(app, id, outside_only, false, drop_point).await
}

#[cfg(feature = "smoke-tests")]
pub async fn rejected_detach_for_test(app: AppHandle, id: String) -> Result<bool, String> {
    detach_inner(app, id, false, true, None).await
}

#[cfg(windows)]
fn check_field_edits(app: &AppHandle, id: &str) -> Result<(), String> {
    // An unloaded tab has no live document to lose.
    let Some(view) = app.get_webview(id) else { return Ok(()); };
    let unknown = "ELIX kunne ikke kontrollere alle feltene på siden. Fanen blir i dette vinduet.";
    let (tx, rx) = std::sync::mpsc::channel();
    view.eval_with_callback(include_str!("transfer-input-check.js"), move |value| { let _ = tx.send(value); })
        .map_err(|_| unknown.to_string())?;
    let value = rx.recv_timeout(std::time::Duration::from_secs(2)).map_err(|_| unknown.to_string())?;
    match serde_json::from_str::<String>(&value).ok().as_deref() {
        Some("clean") => Ok(()),
        Some("edited") => Err("Fanen har redigerte felt. Lagre eller send inn endringene og last inn siden på nytt før du flytter fanen.".into()),
        _ => Err(unknown.into()),
    }
}

async fn detach_inner(
    app: AppHandle,
    id: String,
    outside_only: bool,
    _reject_ready: bool,
    drop_point: Option<DropPoint>,
) -> Result<bool, String> {
    #[cfg(not(windows))]
    {
        let _ = (app, id, outside_only, drop_point);
        Err("Krever Windows".into())
    }
    #[cfg(windows)]
    {
        use windows::Win32::{
            Foundation::{POINT, RECT},
            UI::WindowsAndMessaging::{GetCursorPos, GetWindowRect},
        };
        let window = app.get_window("main").ok_or("Fant ikke vinduet")?;
        let mut point = POINT::default();
        let mut rect = RECT::default();
        unsafe {
            GetCursorPos(&mut point).map_err(|e| e.to_string())?;
            GetWindowRect(window.hwnd().map_err(|e| e.to_string())?, &mut rect)
                .map_err(|e| e.to_string())?;
        }
        // Use the release event coordinates. The pointer can move before the
        // asynchronous shell command runs, especially across monitors.
        if let Some(drop) = drop_point {
            if !drop.x.is_finite() || !drop.y.is_finite() { return Err("Ugyldig slippunkt".into()); }
            let origin = window.inner_position().map_err(|e| e.to_string())?;
            let scale = window.scale_factor().map_err(|e| e.to_string())?;
            point.x = origin.x.saturating_add((drop.x * scale).round() as i32);
            point.y = origin.y.saturating_add((drop.y * scale).round() as i32);
        }
        let target = if outside_only { super::window_transfer::destination(&app, point)? } else { None };
        if outside_only && target.is_none()
            && point.x >= rect.left
            && point.x < rect.right
            && point.y >= rect.top
            && point.y < rect.bottom
        {
            app.state::<AppState>()
                .logs
                .add("info", "window_transfer_drop_inside");
            return Ok(false);
        }
        let _pending = pending::Transfer::reserve(&id)?;
        let state = app.state::<AppState>();
        state.logs.add("info", "window_transfer_requested");
        let handoff = {
            let _transition = state.session_transition.lock().unwrap();
            if state.downloads.lock().unwrap().iter().any(|d| {
                d.view_id == id && (d.can_resume || matches!(d.status.as_str(), "paused" | "downloading"))
            }) {
                return Err("Fullfør eller avbryt nedlastingen før du flytter fanen".into());
            }
            let tab = state.session.lock().unwrap().tabs.iter().find(|t| t.id == id)
                .cloned().ok_or("Ukjent fane")?;
            Handoff {
                saved_session: None,
                profile: state.profile.lock().unwrap().clone(),
                tab,
                x: point.x.saturating_sub(200),
                y: point.y.saturating_sub(40),
            }
        };
        if handoff.tab.private { return Err("Private faner blir i vinduet de ble åpnet i.".into()); }
        check_field_edits(&app, &id)?;
        let source_tab = handoff.tab.clone();
        let original_profile = handoff.profile.clone();
        let outcome = if let Some(pid) = target {
            super::window_transfer::send(pid, &handoff).await
        } else { launch_window(&app, handoff, _reject_ready).await };
        if let Err(error) = outcome {
            state.logs.add("error", "window_transfer_startup_failed");
            return Err(error);
        }
        state.logs.add("info", "window_transfer_ready");
        // The user can still edit the source while its destination starts.
        check_field_edits(&app, &id).map_err(|error| format!("Målvinduet har mottatt siden. {error}"))?;
        let _transition = state.session_transition.lock().unwrap();
        let current_profile = state.profile.lock().unwrap().clone();
        if !source_unchanged(&current_profile, &original_profile, &state.session.lock().unwrap(), &source_tab) {
            return Err("Målvinduet mottok fanen. Kildefanen eller profilen ble endret før flyttingen var ferdig.".into());
        }
        // Never publish a transferred page to the source window's reopen stack.
        browser::close_transferred(&app, &id).map_err(|error| format!("Målvinduet mottok fanen, men kilden kunne ikke lukkes. {error}"))?;
        Ok(true)
    }
}

pub async fn launch_window(app: &AppHandle, handoff: Handoff, _reject_ready: bool) -> Result<(),String> {
    #[cfg(not(windows))]
    { let _ = (app, handoff); Err("Krever Windows".into()) }
    #[cfg(windows)]
    {
        #[cfg(feature = "smoke-tests")]
        let state = app.state::<AppState>();
        #[cfg(feature = "smoke-tests")]
        let saved_session_test = handoff.saved_session.is_some();
        let bytes = serde_json::to_vec(&handoff).map_err(|e| e.to_string())?;
        let executable = std::env::current_exe().map_err(|e| e.to_string())?;
        #[cfg(feature = "smoke-tests")]
        let smoke_root = std::env::args()
            .any(|a| a == "--self-test")
            .then(|| state.root.clone());
        tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
            use std::io::{BufRead, BufReader};
            use std::process::{Command, Stdio};
            let mut command = Command::new(executable);
            let stderr = Stdio::null();
            #[cfg(feature = "smoke-tests")]
            let error_log = smoke_root
                .as_ref()
                .map(|root| root.join(format!("handoff-{}.log", uuid::Uuid::new_v4())));
            #[cfg(feature = "smoke-tests")]
            let stderr = match &error_log {
                Some(path) => Stdio::from(std::fs::File::create(path).map_err(|e| e.to_string())?),
                None => stderr,
            };
            #[cfg(feature = "smoke-tests")]
            if let Some(root) = smoke_root {
                command
                    .arg("--elix-smoke-window")
                    .env("ELIX_SMOKE_ROOT", root);
                if saved_session_test { command.env("ELIX_SMOKE_SAVED_SESSION","1"); }
                if _reject_ready {
                    command.arg("--elix-smoke-reject-window");
                }
            }
            let mut child = command
                .arg(format!("--elix-window={}", uuid::Uuid::new_v4()))
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(stderr)
                .spawn()
                .map_err(|e| e.to_string())?;
            let result = (|| -> Result<(), String> {
                child
                    .stdin
                    .take()
                    .ok_or("Kunne ikke overføre fanen")?
                    .write_all(&bytes)
                    .map_err(|e| e.to_string())?;
                let output = child.stdout.take().ok_or("Kunne ikke bekrefte vinduet")?;
                let (tx, rx) = std::sync::mpsc::channel();
                std::thread::spawn(move || {
                    for line in BufReader::new(output).lines().take(100).flatten() {
                        if line == "ELIX_WINDOW_READY" {
                            let _ = tx.send(());
                            break;
                        }
                    }
                });
                let ready = rx.recv_timeout(std::time::Duration::from_secs(25));
                ready.map_err(|_| {
                    "Det nye vinduet svarte ikke. Originalfanen er beholdt.".to_string()
                })
            })();
            if result.is_err() {
                let _ = child.kill();
                let _ = child.wait();
            }
            #[cfg(feature = "smoke-tests")]
            if let (Err(error), Some(path)) = (&result, error_log) {
                let detail = std::fs::read_to_string(path).unwrap_or_default();
                return Err(format!(
                    "{error} Child diagnostics: {}",
                    detail.chars().take(4000).collect::<String>()
                ));
            }
            result
        })
        .await
        .map_err(|e| e.to_string())?
    }
}

#[cfg(any(windows, test))]
fn source_unchanged(profile: &str, original_profile: &str, session: &browser::Session, source: &Tab) -> bool {
    profile == original_profile && session.tabs.iter().any(|current| {
        current.id == source.id && current.url == source.url && current.private == source.private
            && current.pinned == source.pinned && current.muted == source.muted
            && current.workspace == source.workspace && current.group == source.group
            && current.group_manual == source.group_manual
    })
}

#[cfg(test)]
mod transfer_tests {
    use super::*;
    #[test]
    fn only_one_destination_can_receive_a_source_at_a_time() {
        let source = pending::Transfer::reserve("same-tab").unwrap();
        assert!(pending::Transfer::reserve("same-tab").is_err());
        assert!(pending::Transfer::reserve("other-tab").is_ok());
        drop(source);
        assert!(pending::Transfer::reserve("same-tab").is_ok());
    }
    #[test]
    fn changed_source_survives_a_destination_acknowledgment() {
        let source = Tab::new("https://example.com".into());
        let mut session = browser::Session { tabs: vec![source.clone()], ..Default::default() };
        assert!(source_unchanged("one", "one", &session, &source));
        assert!(!source_unchanged("two", "one", &session, &source));
        session.tabs[0].pinned = true;
        assert!(!source_unchanged("one", "one", &session, &source));
        session.tabs[0] = source.clone();
        session.tabs[0].url = "https://example.com/next".into();
        assert!(!source_unchanged("one", "one", &session, &source));
    }
}
