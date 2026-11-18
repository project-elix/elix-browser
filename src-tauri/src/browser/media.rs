//! Live miniplayer hosting and HTML fullscreen. No page receives privileged IPC.
use super::{engine, Bounds};
use crate::AppState;
use serde::Serialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};

const PLAYER_WINDOW: &str = "media-player";
const PRESENT_MEDIA: &str = include_str!("media-presentation.js");
const RESTORE_MEDIA: &str = "(()=>{const p=window.__elixMediaPresentation;return p&&typeof p.restore==='function'?p.restore():true})()";

// Only open() waits for JavaScript, from the authorized async shell command or
// the smoke worker. Cleanup paths also run on the UI thread and must never wait.
fn present_media(view: &tauri::Webview) -> Result<(), String> {
    let (tx, rx) = std::sync::mpsc::channel();
    view.eval_with_callback(PRESENT_MEDIA, move |value| { let _ = tx.send(value); })
        .map_err(|e| e.to_string())?;
    let result = rx.recv_timeout(std::time::Duration::from_secs(5))
        .map_err(|_| "Nettsiden svarte ikke da mediespilleren skulle åpnes".to_string())
        .and_then(|value| serde_json::from_str::<Value>(&value).map_err(|_| "Kunne ikke lese mediespilleren".to_string()));
    match result {
        Ok(value) if value["ok"] == true => Ok(()),
        result => {
            // FIFO evaluation also removes a presentation whose completion was
            // delayed past the timeout. No popout ownership was granted yet.
            let _ = view.eval(RESTORE_MEDIA);
            match result {
                Ok(value) if value["reason"] == "no-media" => Err("Fant ingen tilgjengelig video eller lydspiller. Start avspillingen først. Innebygde spillere fra andre nettsteder kan kreve at du åpner originalnettstedet.".into()),
                Err(error) => Err(error),
                _ => Err("Denne mediespilleren kunne ikke vises alene. Fanen er beholdt.".into()),
            }
        }
    }
}

fn restore_media(view: &tauri::Webview) { let _ = view.eval(RESTORE_MEDIA); }

#[inline]
fn checkpoint(stage: &str) {
    #[cfg(feature = "smoke-tests")]
    super::smoke::progress(stage);
    #[cfg(not(feature = "smoke-tests"))]
    let _ = stage;
}

// Keep the pinned Tauri runtime's reparent mutex on the UI thread; a worker
// must not hold it while waiting for callbacks on that same event loop.
// See tauri-apps/tauri#15489. Focus preflight avoids synchronous SetParent
// focus callbacks re-entering the moving view's dispatcher.
fn reparent_live(
    app: &tauri::AppHandle,
    view: &tauri::Webview,
    destination: &tauri::Window,
) -> Result<(), String> {
    use std::sync::{atomic::{AtomicU8, Ordering}, mpsc, Arc};
    const PENDING: u8 = 0;
    const RUNNING: u8 = 1;
    const CANCELLED: u8 = 2;
    let phase = Arc::new(AtomicU8::new(PENDING));
    let scheduled_phase = phase.clone();
    let target_app = app.clone();
    let view = view.clone();
    let destination_label = destination.label().to_owned();
    let (tx, rx) = mpsc::channel();
    app.run_on_main_thread(move || {
        // A timed-out queued request may not run after its caller has rolled back.
        if scheduled_phase.compare_exchange(PENDING, RUNNING, Ordering::SeqCst, Ordering::SeqCst).is_err() {
            return;
        }
        let result = (|| -> Result<(), String> {
            // Re-resolve the target on the UI thread. A queued request must not
            // pass a destroyed/replaced window handle to the runtime's Reparent.
            let destination = target_app.get_window(&destination_label).ok_or("Målvinduet ble lukket")?;
            if target_app.get_webview(view.label()).is_none() {
                return Err("Fanen ble lukket før den kunne flyttes".into());
            }
            checkpoint("media.reparent.ui.focus.begin");
            let main = target_app.get_window("main").ok_or("Fant ikke hovedvinduet")?;
            let shell = target_app.get_webview("shell").ok_or("Fant ikke nettlesermenyen")?;
            main.set_focus().map_err(|e| e.to_string())?;
            shell.set_focus().map_err(|e| e.to_string())?;
            #[cfg(windows)]
            {
                // Already on the UI thread: with_webview executes inline in the
                // pinned runtime. try_recv avoids introducing a blocking UI wait.
                let (focus_tx, focus_rx) = mpsc::channel();
                view.with_webview(move |platform| {
                    let result = (|| -> windows::core::Result<bool> {
                        use windows::Win32::{
                            Foundation::HWND,
                            UI::{Input::KeyboardAndMouse::GetFocus, WindowsAndMessaging::IsChild},
                        };
                        unsafe {
                            let mut container = HWND::default();
                            platform.controller().ParentWindow(&mut container)?;
                            let focused = GetFocus();
                            Ok(focused != container && !IsChild(container, focused).as_bool())
                        }
                    })();
                    let _ = focus_tx.send(result.map_err(|e| e.to_string()));
                }).map_err(|e| e.to_string())?;
                if !focus_rx.try_recv().map_err(|_| "Kunne ikke kontrollere fokus før flytting av fanen")?? {
                    return Err("Fanen beholdt fokus. Velg nettlesermenyen og prøv igjen.".into());
                }
            }
            checkpoint("media.reparent.ui.focus.done");
            checkpoint("media.reparent.ui.move.begin");
            view.reparent(&destination).map_err(|e| e.to_string())?;
            checkpoint("media.reparent.ui.move.end");
            Ok(())
        })();
        let _ = tx.send(result);
    }).map_err(|e| e.to_string())?;

    // On the UI thread the pinned runtime ran the closure inline, so its result
    // is already queued. Only an off-thread caller can wait for a pending task.
    match rx.recv_timeout(std::time::Duration::from_secs(10)) {
        Ok(result) => result,
        Err(mpsc::RecvTimeoutError::Disconnected) => Err("Flytting av fanen ble avbrutt".into()),
        Err(mpsc::RecvTimeoutError::Timeout) => {
            if phase.compare_exchange(PENDING, CANCELLED, Ordering::SeqCst, Ordering::SeqCst).is_ok() {
                Err("Nettleservinduet svarte ikke. Fanen ble ikke flyttet.".into())
            } else {
                // The native operation has started; do not pretend it was
                // cancelled and race caller rollback against an active move.
                rx.recv().map_err(|_| "Flytting av fanen ble avbrutt".to_string())?
            }
        }
    }
}

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
    presentation_generation: u64,
    pending_player: Option<(String, u64)>,
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
    // Selecting the source shows the keep/return choice in the shell. The sole
    // live page stays in its always-on-top host until the user chooses return.
    if is_player(app, id) {
        if let Some(window) = app.get_window(PLAYER_WINDOW) {
            window.unminimize().map_err(|e| e.to_string())?;
        }
        if let Some(player) = app.state::<AppState>().media.lock().unwrap().player.as_mut() {
            if player.id == id { player.hidden = false; }
        }
        emit(app);
    }
    Ok(())
}

pub fn before_navigation(app: &AppHandle, id: &str) -> Result<(), String> {
    {
        let state = app.state::<AppState>();
        let mut media = state.media.lock().unwrap();
        if media.pending_player.as_ref().is_some_and(|(pending, _)| pending == id) { media.pending_player = None; }
    }
    exit(app, id)?;
    if is_player(app, id) { return_to_browser(app)?; }
    Ok(())
}

/// Page-initiated navigation may replace the media document. Hide the separate
/// window immediately, then return ownership on a worker without blocking UI.
#[cfg(windows)]
pub fn navigation_started(app: &AppHandle, id: &str) {
    {
        let state = app.state::<AppState>();
        let mut media = state.media.lock().unwrap();
        if media.pending_player.as_ref().is_some_and(|(pending, _)| pending == id) { media.pending_player = None; }
    }
    let _ = exit(app, id);
    if !is_player(app, id) { return; }
    if let Some(window) = app.get_window(PLAYER_WINDOW) { let _ = window.hide(); }
    if let Some(player) = app.state::<AppState>().media.lock().unwrap().player.as_mut() {
        if player.id == id { player.hidden = true; }
    }
    let app = app.clone();
    let id = id.to_owned();
    tauri::async_runtime::spawn(async move {
        if is_player(&app, &id) {
            if let Err(error) = perform(&app, "return", None) { let _ = app.emit_to("shell", "notice", error); }
        }
    });
}

/// Release presentation state before the owning tab is destroyed or recovered.
pub fn release(app: &AppHandle, id: &str) -> Result<(), String> {
    checkpoint("media.release.begin");
    {
        let state = app.state::<AppState>();
        let mut media = state.media.lock().unwrap();
        if media.pending_player.as_ref().is_some_and(|(pending, _)| pending == id) { media.pending_player = None; }
    }
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
            restore_media(&view);
            checkpoint("media.release.reparent.begin");
            let reparented = app.get_window("main").is_some_and(|window| reparent_live(app, &view, &window).is_ok());
            checkpoint("media.release.reparent.end");
            if !reparented { let _ = view.close(); }
        }
        if let Some(window) = app.get_window(PLAYER_WINDOW) { let _ = window.hide(); }
        emit(app);
    }
    checkpoint("media.release.end");
    Ok(())
}

fn return_to_browser(app: &AppHandle) -> Result<Option<String>, String> {
    checkpoint("media.return.begin");
    let player = app.state::<AppState>().media.lock().unwrap().player.clone();
    let Some(player) = player else { return Ok(None); };
    exit(app, &player.id)?;
    checkpoint("media.return.fullscreen.exit.done");
    if let Some(view) = app.get_webview(&player.id) {
        let main = app.get_window("main").ok_or("Fant ikke hovedvinduet")?;
        checkpoint("media.return.reparent.begin");
        reparent_live(app, &view, &main)?;
        checkpoint("media.return.reparent.end");
        restore_media(&view);
    }
    app.state::<AppState>().media.lock().unwrap().player = None;
    checkpoint("media.return.ownership.cleared");
    if let Some(window) = app.get_window(PLAYER_WINDOW) { window.hide().map_err(|e| e.to_string())?; }
    checkpoint("media.return.window.hidden");
    super::organization::layout(app)?;
    checkpoint("media.return.layout.done");
    emit(app);
    checkpoint("media.return.emit.done");
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
    // Find and isolate real media before moving native views. A page without
    // accessible media never becomes a whole-website popout.
    exit(app, id)?;
    let generation = {
        let state = app.state::<AppState>();
        let mut media = state.media.lock().unwrap();
        if media.pending_player.is_some() { return Err("En mediespiller er allerede i ferd med å åpnes".into()); }
        media.presentation_generation = media.presentation_generation.wrapping_add(1);
        let generation = media.presentation_generation;
        media.pending_player = Some((id.into(), generation));
        generation
    };
    let pending = || app.state::<AppState>().media.lock().unwrap().pending_player.as_ref()
        .is_some_and(|(pending, current)| pending == id && *current == generation);
    let abort = || {
        let state = app.state::<AppState>();
        let mut media = state.media.lock().unwrap();
        if media.pending_player.as_ref().is_some_and(|(pending, current)| pending == id && *current == generation) {
            media.pending_player = None;
        }
        drop(media);
        restore_media(&view);
    };
    if let Err(error) = present_media(&view) { abort(); return Err(error); }
    if !pending() { abort(); return Err("Nettsiden ble endret mens mediespilleren ble åpnet. Prøv igjen.".into()); }
    if let Err(error) = return_to_browser(app) { abort(); return Err(error); }
    let window_result = (|| -> Result<tauri::Window, String> { Ok(if let Some(window) = app.get_window(PLAYER_WINDOW) { window } else {
        tauri::window::WindowBuilder::new(app, PLAYER_WINDOW)
            .title("ELIX Minispiller").inner_size(640., 400.).min_inner_size(320., 180.)
            .always_on_top(true).visible(false).theme(Some(tauri::Theme::Dark))
            .build().map_err(|e| e.to_string())?
    }) })();
    let window = match window_result { Ok(window) => window, Err(error) => { abort(); return Err(error); } };
    if let Err(error) = window.set_title(&format!("ELIX Minispiller · {}", tab.title)) {
        abort(); return Err(error.to_string());
    }
    if !pending() { abort(); return Err("Nettsiden ble endret mens mediespilleren ble åpnet. Prøv igjen.".into()); }
    checkpoint("media.open.reparent.begin");
    if let Err(error) = reparent_live(app, &view, &window) { abort(); return Err(error); }
    checkpoint("media.open.reparent.end");
    let accepted = {
        let state = app.state::<AppState>();
        let mut media = state.media.lock().unwrap();
        if media.pending_player.as_ref().is_some_and(|(pending, current)| pending == id && *current == generation) {
            media.pending_player = None;
            media.player = Some(Player { id: id.into(), hidden: false });
            true
        } else { false }
    };
    if !accepted {
        abort();
        let restored = app.get_window("main").is_some_and(|main| reparent_live(app, &view, &main).is_ok());
        if !restored {
            // Preserve an explicit return control if the native rollback fails,
            // but do not display the navigated whole page in the player window.
            app.state::<AppState>().media.lock().unwrap().player = Some(Player { id: id.into(), hidden: true });
            emit(app);
        }
        let _ = window.hide();
        let _ = super::organization::layout(app);
        return Err("Nettsiden ble endret mens mediespilleren ble åpnet. Prøv igjen.".into());
    }
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
      window.unminimize().map_err(|e| e.to_string())?;
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
        if let Some(window) = app.get_window(PLAYER_WINDOW) {
            window.unminimize().map_err(|e| e.to_string())?;
            window.set_focus().map_err(|e| e.to_string())?;
        }
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
            if id.is_some_and(|source| !is_player(app, source)) {
                return Err("Minispilleren har byttet fane.".into());
            }
            if let Some(id) = return_to_browser(app)? {
                checkpoint("media.return.activate.begin");
                super::activate(app, &id)?;
                checkpoint("media.return.activate.end");
            }
        },
        _ => return Err("Ukjent mediehandling".into()),
    }
    Ok(snapshot(app))
}

pub fn reset(app: &AppHandle) -> Result<(), String> {
    checkpoint("media.reset.begin");
    app.state::<AppState>().media.lock().unwrap().pending_player = None;
    let fullscreen = app.state::<AppState>().media.lock().unwrap().fullscreen.clone();
    if let Some(fullscreen) = fullscreen { let _ = exit(app, &fullscreen.id); }
    let player = app.state::<AppState>().media.lock().unwrap().player.clone();
    if let Some(player) = player { release(app, &player.id)?; }
    checkpoint("media.reset.end");
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
