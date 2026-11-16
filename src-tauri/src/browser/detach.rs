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
        if let Ok(session) = serde_json::from_str::<browser::Session>(&data) {
            tabs.extend(
                session
                    .tabs
                    .into_iter()
                    .filter(|t| !t.private && crate::security::destination(&t.url).is_ok()),
            );
            consumed.push(key);
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
        for mut tab in tabs {
            tab.id = format!("tab-{}", uuid::Uuid::new_v4());
            tab.suspended = !tab.url.starts_with("elix:");
            tab.loading = false;
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
            || crate::security::destination(&handoff.tab.url).is_err()
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

pub async fn detach(app: AppHandle, id: String, outside_only: bool) -> Result<bool, String> {
    detach_inner(app, id, outside_only, false).await
}

#[cfg(feature = "smoke-tests")]
pub async fn rejected_detach_for_test(app: AppHandle, id: String) -> Result<bool, String> {
    detach_inner(app, id, false, true).await
}

async fn detach_inner(
    app: AppHandle,
    id: String,
    outside_only: bool,
    _reject_ready: bool,
) -> Result<bool, String> {
    #[cfg(not(windows))]
    {
        let _ = (app, id, outside_only);
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
        if outside_only
            && point.x >= rect.left
            && point.x < rect.right
            && point.y >= rect.top
            && point.y < rect.bottom
        {
            return Ok(false);
        }
        let state = app.state::<AppState>();
        if state.downloads.lock().unwrap().iter().any(|d| {
            d.view_id == id
                && (d.can_resume || matches!(d.status.as_str(), "paused" | "downloading"))
        }) {
            return Err("Fullfør eller avbryt nedlastingen før du flytter fanen".into());
        }
        let tab = state
            .session
            .lock()
            .unwrap()
            .tabs
            .iter()
            .find(|t| t.id == id)
            .cloned()
            .ok_or("Ukjent fane")?;
        let original_url = tab.url.clone();
        let handoff = Handoff {
            profile: state.profile.lock().unwrap().clone(),
            tab,
            x: point.x.saturating_sub(200),
            y: point.y.saturating_sub(40),
        };
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
            #[cfg(feature = "smoke-tests")]
            if let Some(root) = smoke_root {
                command
                    .arg("--elix-smoke-window")
                    .env("ELIX_SMOKE_ROOT", root);
                if _reject_ready {
                    command.arg("--elix-smoke-reject-window");
                }
            }
            let mut child = command
                .arg(format!("--elix-window={}", uuid::Uuid::new_v4()))
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
                .map_err(|e| e.to_string())?;
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
            if ready.is_err() {
                let _ = child.kill();
                let _ = child.wait();
            }
            ready.map_err(|_| "Det nye vinduet svarte ikke. Originalfanen er beholdt.".to_string())
        })
        .await
        .map_err(|e| e.to_string())??;
        if state
            .session
            .lock()
            .unwrap()
            .tabs
            .iter()
            .any(|t| t.id == id && t.url == original_url)
        {
            browser::close(&app, &id)?;
            // Moving is not closing: don't duplicate the moved page in Ctrl+Shift+T.
            state.session.lock().unwrap().closed.retain(|t| t.id != id);
            browser::persist(&app)?;
            browser::emit(&app);
        }
        Ok(true)
    }
}
