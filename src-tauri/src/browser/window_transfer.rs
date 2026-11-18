//! Transfer to another running ELIX process without publishing a URL through
//! the clipboard, drag data, window messages, temporary files or process args.
use super::{detach::Handoff, Tab};
use crate::AppState;
use serde::{de::DeserializeOwned, Serialize};
use std::{os::windows::io::AsRawHandle, time::Duration};
use tauri::{AppHandle, Manager};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::windows::named_pipe::{ClientOptions, NamedPipeServer, ServerOptions},
    time::timeout,
};
use windows::{
    core::w,
    Win32::{
        Foundation::{HANDLE, POINT},
        System::Pipes::{GetNamedPipeClientProcessId, GetNamedPipeServerProcessId},
        UI::WindowsAndMessaging::{GetAncestor, GetPropW, GetWindowThreadProcessId, SetPropW, WindowFromPoint, GA_ROOT},
    },
};

const MAX_FRAME: usize = 1_000_000;
const TRANSFER_TIMEOUT: Duration = Duration::from_secs(12);

fn pipe_name(pid: u32) -> String { format!(r"\\.\pipe\ELIX-Browser-Tab-v1-{pid}") }

fn same_executable(pid: u32) -> bool {
    use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
    let pid = Pid::from_u32(pid);
    let mut system = System::new();
    system.refresh_processes_specifics(ProcessesToUpdate::Some(&[pid]), false, ProcessRefreshKind::nothing().with_exe(UpdateKind::Always));
    let executable = std::env::current_exe().and_then(std::fs::canonicalize).ok();
    let peer = system.process(pid).and_then(|p| p.exe()).and_then(|p| p.canonicalize().ok());
    peer.is_some() && peer == executable
}

fn registered(app: &AppHandle, pid: u32) -> bool {
    app.state::<AppState>().store.lock().unwrap().conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM settings WHERE key LIKE 'window_pid:%' AND value=?1)",
        [pid.to_string()], |row| row.get::<_, bool>(0)
    ).unwrap_or(false)
}

/// The top-level window under the release point controls routing. A foreign
/// application's HWND is never used as a transport for ELIX tab data.
pub fn destination(app: &AppHandle, point: POINT) -> Result<Option<u32>, String> {
    unsafe {
        let root = GetAncestor(WindowFromPoint(point), GA_ROOT);
        if root.is_invalid() { return Ok(None); }
        let mut pid = 0;
        GetWindowThreadProcessId(root, Some(&mut pid));
        if pid == 0 || pid == std::process::id() || !same_executable(pid) { return Ok(None); }
        if GetPropW(root, w!("ELIXBrowser.TabTransfer.v1")).0 as usize != pid as usize || !registered(app, pid) {
            return Err("Dette ELIX-vinduet kan ikke motta fanen. Originalfanen er beholdt.".into());
        }
        Ok(Some(pid))
    }
}

async fn write_frame<W: AsyncWrite + Unpin, T: Serialize>(pipe: &mut W, value: &T) -> Result<(), String> {
    let data = serde_json::to_vec(value).map_err(|e| e.to_string())?;
    if data.len() > MAX_FRAME { return Err("Fanedata er for store".into()); }
    pipe.write_u32(data.len() as u32).await.map_err(|e| e.to_string())?;
    pipe.write_all(&data).await.map_err(|e| e.to_string())
}

async fn read_frame<R: AsyncRead + Unpin, T: DeserializeOwned>(pipe: &mut R) -> Result<T, String> {
    let size = pipe.read_u32().await.map_err(|e| e.to_string())? as usize;
    if size > MAX_FRAME { return Err("Ugyldig størrelsesfelt".into()); }
    let mut data = vec![0; size];
    pipe.read_exact(&mut data).await.map_err(|e| e.to_string())?;
    serde_json::from_slice(&data).map_err(|_| "Ugyldig melding mellom vinduene".into())
}

pub async fn send(pid: u32, handoff: &Handoff) -> Result<(), String> {
    timeout(TRANSFER_TIMEOUT, async {
        let mut pipe = ClientOptions::new().open(pipe_name(pid)).map_err(|_| "Målvinduet er opptatt eller lukket. Prøv igjen.")?;
        let mut server_pid = 0;
        unsafe { GetNamedPipeServerProcessId(HANDLE(pipe.as_raw_handle()), &mut server_pid) }.map_err(|e| e.to_string())?;
        if server_pid != pid || !same_executable(server_pid) { return Err("Kunne ikke bekrefte ELIX-vinduet".into()); }
        write_frame(&mut pipe, &handoff.profile).await?;
        read_frame::<_, Result<(),String>>(&mut pipe).await??;
        write_frame(&mut pipe, &handoff.tab).await?;
        let result = read_frame::<_, Result<(),String>>(&mut pipe).await?;
        // The server keeps the pipe instance connected until its reply has been
        // read. DisconnectNamedPipe discards unread bytes.
        let _ = pipe.write_u8(1).await;
        result
    }).await.map_err(|_| "Målvinduet bekreftet ikke flyttingen. Originalfanen er beholdt.".to_string())?
}

fn receive(app: &AppHandle, profile: &str, mut tab: Tab) -> Result<(), String> {
    if tab.private || !crate::security::stored_destination_allowed(&tab.url)
        || tab.id.strip_prefix("tab-").and_then(|v| uuid::Uuid::parse_str(v).ok()).is_none() {
        return Err("Fanen kan ikke flyttes til dette vinduet".into());
    }
    let state = app.state::<AppState>();
    let _transition = state.session_transition.lock().unwrap();
    if *state.profile.lock().unwrap() != profile { return Err("Vinduene bruker ulike profiler".into()); }
    let id = tab.id.clone();
    let previous = {
        let mut session = state.session.lock().unwrap();
        // If the acknowledgment was lost, the same transfer must not create a
        // second copy. Refuse to discard a source if the destination has moved on.
        if let Some(existing) = session.tabs.iter().find(|existing| existing.id == id) {
            if existing.url != tab.url || existing.private { return Err("Fanen i målvinduet er endret".into()); }
            drop(session);
            return super::persist(app);
        }
        if session.tabs.len() >= 100 { return Err("Målvinduet har allerede 100 faner".into()); }
        let previous = session.active.clone();
        tab.reset_runtime_state(chrono::Utc::now().timestamp());
        tab.workspace = session.workspace.clone();
        session.tabs.push(tab);
        previous
    };
    if let Err(error) = super::activate(app, &id).and_then(|_| super::persist(app)) {
        // Never signal success until the destination owns and saved the tab.
        let _ = super::close_transferred(app, &id);
        let _ = super::activate(app, &previous);
        return Err(error);
    }
    if let Some(window) = app.get_window("main") { let _ = window.set_focus(); }
    Ok(())
}

async fn serve(app: &AppHandle, pipe: &mut NamedPipeServer) -> Result<(), String> {
    let mut pid = 0;
    unsafe { GetNamedPipeClientProcessId(HANDLE(pipe.as_raw_handle()), &mut pid) }.map_err(|e| e.to_string())?;
    if pid == std::process::id() || !registered(app, pid) || !same_executable(pid) {
        return Err("Ukjent vindu".into());
    }
    let profile: String = read_frame(pipe).await?;
    let accepted = if *app.state::<AppState>().profile.lock().unwrap() == profile {
        Ok(())
    } else { Err("Vinduene bruker ulike profiler. Originalfanen er beholdt.".to_string()) };
    write_frame(pipe, &accepted).await?;
    // Wait for the client to close after a refusal so it can read the reply.
    if accepted.is_err() { let _ = pipe.read_u8().await; return Ok(()); }
    let tab: Tab = read_frame(pipe).await?;
    let target = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || receive(&target, &profile, tab))
        .await.map_err(|e| e.to_string())?;
    write_frame(pipe, &result).await?;
    let _ = pipe.read_u8().await;
    Ok(())
}

pub fn listen(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let pid = std::process::id();
        let Ok(mut pipe) = ServerOptions::new().first_pipe_instance(true)
            .reject_remote_clients(true).max_instances(1).create(pipe_name(pid)) else {
            app.state::<AppState>().logs.add("warn", "window_transfer_unavailable");
            return;
        };
        let Some(window) = app.get_window("main") else { return; };
        let Ok(hwnd) = window.hwnd() else { return; };
        if unsafe { SetPropW(hwnd, w!("ELIXBrowser.TabTransfer.v1"), Some(HANDLE(pid as usize as _))) }.is_err() {
            return;
        }
        while pipe.connect().await.is_ok() {
            let _ = timeout(TRANSFER_TIMEOUT, serve(&app, &mut pipe)).await;
            if pipe.disconnect().is_err() { break; }
        }
    });
}
