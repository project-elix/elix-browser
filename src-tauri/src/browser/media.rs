//! Live miniplayer hosting and HTML fullscreen. No page receives privileged IPC.
use super::{engine, Bounds};
use crate::AppState;
use serde::Serialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};

const PLAYER_WINDOW: &str = "media-player";

#[derive(Clone, Serialize)]
pub struct Player {
    pub id: String,
    pub hidden: bool,
}

#[derive(Clone)]
struct Fullscreen {
    id: String,
    window: String,
    was_fullscreen: bool,
}

#[derive(Default)]
pub struct State {
    player: Option<Player>,
    fullscreen: Option<Fullscreen>,
    // A forced exit can be followed by WebView2's delayed fullscreen=false event.
    // Remember the previous mode until that event restores it after Tauri's handler.
    exit_restore: Option<Fullscreen>,
    generation: u64,
}

pub fn snapshot(app: &AppHandle) -> Value {
    let state = app.state::<AppState>();
    let media = state.media.lock().unwrap();
    json!({"player": media.player, "fullscreen": media.fullscreen.as_ref().map(|f| &f.id)})
}

fn emit(app: &AppHandle) {
    let _ = app.emit_to("shell", "media-changed", snapshot(app));
}

pub fn is_player(app: &AppHandle, id: &str) -> bool {
    app.state::<AppState>().media.lock().unwrap().player.as_ref().is_some_and(|p| p.id == id)
}

pub fn is_fullscreen(app: &AppHandle, id: &str) -> bool {
    app.state::<AppState>().media.lock().unwrap().fullscreen.as_ref().is_some_and(|f| f.id == id)
}

fn client_bounds(window: &tauri::Window) -> Result<Bounds, String> {
    let size = window.inner_size().map_err(|e| e.to_string())?
        .to_logical::<f64>(window.scale_factor().map_err(|e| e.to_string())?);
    Ok(Bounds { x: 0., y: 0., width: size.width.max(1.), height: size.height.max(1.) })
}

/// Called before the normal main-window layout. A reparented view must never be
/// resized or hidden by the main browser's tab/split layout.
pub fn layout(app: &AppHandle) -> Result<bool, String> {
    let state = app.state::<AppState>();
    let permissions = state.permissions_visible.load(std::sync::atomic::Ordering::SeqCst);
    let (player, fullscreen) = {
        let media = state.media.lock().unwrap();
        (media.player.clone(), media.fullscreen.clone())
    };
    if let Some(player) = &player {
        if let Some(window) = app.get_window(PLAYER_WINDOW) {
            if permissions || player.hidden {
                if window.is_visible().map_err(|e| e.to_string())? { window.hide().map_err(|e| e.to_string())?; }
            } else {
                if let Some(view) = app.get_webview(&player.id) {
                    engine::bounds(&view, client_bounds(&window)?)?;
                    view.show().map_err(|e| e.to_string())?;
                }
                if !window.is_visible().map_err(|e| e.to_string())? { window.show().map_err(|e| e.to_string())?; }
            }
        }
    }
    let Some(fullscreen) = fullscreen else { return Ok(false); };
    if permissions {
        exit(app, &fullscreen.id)?;
        return Ok(false);
    }
    let Some(view) = app.get_webview(&fullscreen.id) else {
        exit(app, &fullscreen.id)?;
        return Ok(false);
    };
    let window = view.window();
    // A delayed fullscreen=false notification from the previous page must not
    // take the current fullscreen owner out of fullscreen.
    if !window.is_fullscreen().map_err(|e| e.to_string())? {
        window.set_fullscreen(true).map_err(|e| e.to_string())?;
    }
    if fullscreen.window == "main" {
        if let Some(shell) = app.get_webview("shell") {
            shell.hide().map_err(|e| e.to_string())?;
        }
        for (id, other) in app.webviews() {
            if id.starts_with("tab-") && id != fullscreen.id && !is_player(app, &id) {
                other.hide().map_err(|e| e.to_string())?;
            }
        }
    }
    engine::bounds(&view, client_bounds(&window)?)?;
    view.show().map_err(|e| e.to_string())?;
    Ok(fullscreen.window == "main")
}

fn restore(app: &AppHandle, fullscreen: &Fullscreen) -> Result<(), String> {
    if let Some(window) = app.get_window(&fullscreen.window) {
        window.set_fullscreen(fullscreen.was_fullscreen).map_err(|e| e.to_string())?;
    }
    if let Some(shell) = app.get_webview("shell") {
        shell.show().map_err(|e| e.to_string())?;
    }
    super::organization::layout(app)?;
    crate::window::emit(app);
    emit(app);
    Ok(())
}

/// WebView2 owns the truth about HTML fullscreen, including iframe video. Tauri
/// also queues a window fullscreen transition; restore after that queued event.
#[cfg(windows)]
pub fn html_fullscreen(app: &AppHandle, id: &str, enabled: bool) -> Result<(), String> {
    let state = app.state::<AppState>();
    let view = app.get_webview(id).ok_or("Fant ikke nettvisningen")?;
    let window = view.window();
    if enabled {
        let previous = state.media.lock().unwrap().fullscreen.clone();
        if let Some(previous) = previous {
            if previous.id != id { exit(app, &previous.id)?; }
        }
        let visible = {
            let session = state.session.lock().unwrap();
            session.active == id || session.split.as_ref().is_some_and(|pair| pair.iter().any(|t| t == id))
        } || state.media.lock().unwrap().player.as_ref().is_some_and(|p| p.id == id && !p.hidden);
        if !visible || state.permissions_visible.load(std::sync::atomic::Ordering::SeqCst) {
            state.media.lock().unwrap().exit_restore = Some(Fullscreen {
                id: id.into(), window: window.label().into(),
                was_fullscreen: window.is_fullscreen().map_err(|e| e.to_string())?,
            });
            let _ = view.eval("if(document.fullscreenElement)document.exitFullscreen().catch(()=>{});");
            return Ok(());
        }
    }
    let (generation, prior) = {
        let mut media = state.media.lock().unwrap();
        if !enabled && !media.fullscreen.as_ref().is_some_and(|f| f.id == id)
            && !media.exit_restore.as_ref().is_some_and(|f| f.id == id) {
            return Ok(());
        }
        media.generation = media.generation.wrapping_add(1);
        if enabled {
            // Duplicate notifications must not replace the original window mode.
            if media.fullscreen.as_ref().is_none_or(|f| f.id != id) {
                media.fullscreen = Some(Fullscreen {
                    id: id.into(), window: window.label().into(),
                    was_fullscreen: window.is_fullscreen().map_err(|e| e.to_string())?,
                });
            }
            media.exit_restore = None;
            (media.generation, None)
        } else {
            let prior = if media.fullscreen.as_ref().is_some_and(|f| f.id == id) {
                media.fullscreen.take()
            } else if media.exit_restore.as_ref().is_some_and(|f| f.id == id) {
                media.exit_restore.take()
            } else { None };
            (media.generation, prior)
        }
    };
    let app = app.clone();
    std::thread::spawn(move || {
        // Queue one reconciliation behind Tauri's own fullscreen event. Keeping
        // its generation check and native updates on the UI thread prevents a
        // newer HTML event from interleaving between the check and restoration.
        let target = app.clone();
        let _ = app.run_on_main_thread(move || {
            if target.state::<AppState>().media.lock().unwrap().generation != generation { return; }
            let result = if let Some(prior) = prior { restore(&target, &prior) }
                else { super::organization::layout(&target) };
            if result.is_err() { target.state::<AppState>().logs.add("warn", "media_fullscreen_layout_failed"); }
            emit(&target);
        });
    });
    Ok(())
}

pub fn exit(app: &AppHandle, id: &str) -> Result<(), String> {
    let prior = {
        let state = app.state::<AppState>();
        let mut media = state.media.lock().unwrap();
        if !media.fullscreen.as_ref().is_some_and(|f| f.id == id) { return Ok(()); }
        media.generation = media.generation.wrapping_add(1);
        let prior = media.fullscreen.take().unwrap();
        media.exit_restore = Some(prior.clone());
        prior
    };
    if let Some(view) = app.get_webview(id) {
        let _ = view.eval("if(document.fullscreenElement)document.exitFullscreen().catch(()=>{});");
    }
    restore(app, &prior)
}

pub fn before_activate(app: &AppHandle, id: &str) -> Result<(), String> {
    let prior = app.state::<AppState>().media.lock().unwrap().fullscreen.clone();
    if let Some(prior) = prior {
        if prior.window == "main" && prior.id != id { exit(app, &prior.id)?; }
    }
    if is_player(app, id) { return_to_browser(app)?; }
    Ok(())
}

pub fn before_navigation(app: &AppHandle, id: &str) -> Result<(), String> { exit(app, id) }

/// Release presentation state before the owning tab is destroyed or recovered.
pub fn release(app: &AppHandle, id: &str) -> Result<(), String> {
    let _ = exit(app, id);
    let removed = {
        let state = app.state::<AppState>();
        let mut media = state.media.lock().unwrap();
        if media.player.as_ref().is_some_and(|p| p.id == id) { media.player.take(); true } else { false }
    };
    if removed {
        // Teardown/recovery must remain possible even after a controller crash.
        // Returning a live player is strict; destroying one clears ownership first.
        if let Some(view) = app.get_webview(id) {
            let reparented = app.get_window("main").is_some_and(|window| view.reparent(&window).is_ok());
            if !reparented { let _ = view.close(); }
        }
        if let Some(window) = app.get_window(PLAYER_WINDOW) { let _ = window.hide(); }
        emit(app);
    }
    Ok(())
}

fn return_to_browser(app: &AppHandle) -> Result<Option<String>, String> {
    let player = app.state::<AppState>().media.lock().unwrap().player.clone();
    let Some(player) = player else { return Ok(None); };
    exit(app, &player.id)?;
    if let Some(view) = app.get_webview(&player.id) {
        let main = app.get_window("main").ok_or("Fant ikke hovedvinduet")?;
        view.reparent(&main).map_err(|e| e.to_string())?;
    }
    app.state::<AppState>().media.lock().unwrap().player = None;
    if let Some(window) = app.get_window(PLAYER_WINDOW) { window.hide().map_err(|e| e.to_string())?; }
    super::organization::layout(app)?;
    emit(app);
    Ok(Some(player.id))
}

fn open(app: &AppHandle, id: &str) -> Result<(), String> {
    if app.state::<AppState>().permissions_visible.load(std::sync::atomic::Ordering::SeqCst) {
        return Err("Svar på nettstedstillatelsen før du åpner minispilleren.".into());
    }
    let tab = app.state::<AppState>().session.lock().unwrap().tabs.iter()
        .find(|t| t.id == id && !t.url.starts_with("elix:")).cloned().ok_or("Åpne en nettside først")?;
    let view = app.get_webview(id).ok_or("Vekk fanen før du åpner minispilleren")?;
    if is_player(app, id) { return set_hidden(app, false); }
    // Validate first, then return any existing player without restarting it.
    return_to_browser(app)?;
    exit(app, id)?;
    let window = if let Some(window) = app.get_window(PLAYER_WINDOW) { window } else {
        tauri::window::WindowBuilder::new(app, PLAYER_WINDOW)
            .title("ELIX Minispiller").inner_size(640., 400.).min_inner_size(320., 180.)
            .always_on_top(true).visible(false).theme(Some(tauri::Theme::Dark))
            .build().map_err(|e| e.to_string())?
    };
    window.set_title(&format!("ELIX Minispiller · {}", tab.title)).map_err(|e| e.to_string())?;
    view.reparent(&window).map_err(|e| e.to_string())?;
    app.state::<AppState>().media.lock().unwrap().player = Some(Player { id: id.into(), hidden: false });
    // Keep the return control available even if a later window/focus call fails.
    emit(app);
    let result = (|| -> Result<(), String> {
      let next = {
        let state = app.state::<AppState>();
        let mut session = state.session.lock().unwrap();
        session.clear_split(chrono::Utc::now().timestamp());
        if session.active != id { session.active.clone() }
        else if let Some(tab) = session.tabs.iter().find(|t| t.id != id && t.workspace == session.workspace)
            .or_else(|| if session.tabs.len() >= 100 { session.tabs.iter().find(|t| t.id != id) } else { None }) { tab.id.clone() }
        else {
            let mut tab = super::Tab::new("elix://start".into());
            tab.workspace = session.workspace.clone();
            let id = tab.id.clone(); session.tabs.push(tab); id
        }
      };
      super::activate(app, &next)?;
      layout(app)?;
      window.set_focus().map_err(|e| e.to_string())?;
      view.set_focus().map_err(|e| e.to_string())?;
      super::persist(app)?;
      Ok(())
    })();
    if result.is_err() {
        // A lazy next tab can fail to create. Return the still-live source page
        // rather than leaving it trapped inside an invisible native window.
        if return_to_browser(app).is_ok() { let _ = super::activate(app, id); }
        else { let _ = window.show(); }
    }
    emit(app);
    result
}

fn set_hidden(app: &AppHandle, hidden: bool) -> Result<(), String> {
    let id = app.state::<AppState>().media.lock().unwrap().player.as_ref().map(|p| p.id.clone()).ok_or("Ingen minispiller er åpen")?;
    if hidden { exit(app, &id)?; }
    if let Some(player) = app.state::<AppState>().media.lock().unwrap().player.as_mut() { player.hidden = hidden; }
    layout(app)?;
    if !hidden {
        if let Some(window) = app.get_window(PLAYER_WINDOW) { window.set_focus().map_err(|e| e.to_string())?; }
    }
    emit(app);
    Ok(())
}

/// Called only by the already-authorized shell dispatcher.
pub fn perform(app: &AppHandle, operation: &str, id: Option<&str>) -> Result<Value, String> {
    match operation {
        "state" => {},
        "open" => open(app, id.ok_or("Velg en fane")?)?,
        "hide" => set_hidden(app, true)?,
        "show" => set_hidden(app, false)?,
        "return" => {
            if let Some(id) = return_to_browser(app)? { super::activate(app, &id)?; }
        },
        _ => return Err("Ukjent mediehandling".into()),
    }
    Ok(snapshot(app))
}

pub fn reset(app: &AppHandle) -> Result<(), String> {
    let fullscreen = app.state::<AppState>().media.lock().unwrap().fullscreen.clone();
    if let Some(fullscreen) = fullscreen { let _ = exit(app, &fullscreen.id); }
    let player = app.state::<AppState>().media.lock().unwrap().player.clone();
    if let Some(player) = player { release(app, &player.id)?; }
    Ok(())
}

pub fn shutdown(app: &AppHandle) {
    let _ = reset(app);
    if let Some(window) = app.get_window(PLAYER_WINDOW) { let _ = window.destroy(); }
}

/// Return true for every miniplayer event so it cannot enter main-window cleanup.
pub fn window_event(window: &tauri::Window, event: &tauri::WindowEvent) -> bool {
    if window.label() != PLAYER_WINDOW {
        if matches!(event, tauri::WindowEvent::Resized(_) | tauri::WindowEvent::ScaleFactorChanged { .. }) {
            let _ = layout(window.app_handle());
        }
        return false;
    }
    match event {
        tauri::WindowEvent::CloseRequested { api, .. } => {
            api.prevent_close();
            let app = window.app_handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(error) = perform(&app, "return", None) { let _ = app.emit_to("shell", "notice", error); }
            });
        },
        tauri::WindowEvent::Resized(_) | tauri::WindowEvent::ScaleFactorChanged { .. } => { let _ = layout(window.app_handle()); },
        _ => {},
    }
    true
}
