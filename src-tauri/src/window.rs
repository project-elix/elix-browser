//! Privileged native window controls; only the trusted shell may call dispatch.
use serde::Deserialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};
#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    State,
    Minimize,
    ToggleMaximize,
    ToggleFullscreen,
    ExitFullscreen,
    Drag,
    Close,
}
pub fn perform(app: &AppHandle, action: Action) -> Result<Value, String> {
    let window = app.get_window("main").ok_or("Fant ikke hovedvinduet")?;
    let result = match action {
        Action::State => Ok(()),
        Action::Minimize => window.minimize(),
        Action::ToggleMaximize => {
            if window.is_fullscreen().map_err(|e| e.to_string())? {
                window.set_fullscreen(false).map_err(|e| e.to_string())?;
            }
            if window.is_maximized().map_err(|e| e.to_string())? {
                window.unmaximize()
            } else {
                window.maximize()
            }
        }
        Action::ToggleFullscreen => {
            window.set_fullscreen(!window.is_fullscreen().map_err(|e| e.to_string())?)
        }
        Action::ExitFullscreen => window.set_fullscreen(false),
        Action::Drag => window.start_dragging(),
        Action::Close => {
            crate::browser::persist(app)?;
            window.close()
        }
    };
    result.map_err(|e| e.to_string())?;
    if !matches!(action, Action::Close | Action::Drag) {
        emit(app);
    }
    Ok(snapshot(&window))
}
fn snapshot(window: &tauri::Window) -> Value {
    json!({"maximized":window.is_maximized().unwrap_or(false),"fullscreen":window.is_fullscreen().unwrap_or(false)})
}
pub fn emit(app: &AppHandle) {
    if let Some(window) = app.get_window("main") {
        let _ = app.emit_to("shell", "window-state", snapshot(&window));
    }
}
