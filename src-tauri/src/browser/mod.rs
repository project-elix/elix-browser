pub mod engine;
#[cfg(feature = "smoke-tests")]
pub mod smoke;
#[cfg(windows)]
mod windows;
use crate::{security, AppState};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
#[derive(Clone, Serialize, Deserialize)]
pub struct Tab {
    pub id: String,
    pub url: String,
    pub title: String,
    #[serde(default)]
    pub loading: bool,
    #[serde(default)]
    pub suspended: bool,
    #[serde(default)]
    pub last_active: i64,
    #[serde(default)]
    pub load_ms: Option<u64>,
    #[serde(default)]
    pub http_status: Option<i32>,
    #[serde(default)]
    pub navigation_error: Option<String>,
    #[serde(skip)]
    pub started: Option<std::time::Instant>,
}
impl Tab {
    pub fn new(url: String) -> Self {
        Self {
            id: format!("tab-{}", uuid::Uuid::new_v4()),
            title: if url.starts_with("elix:") {
                "ELIX".into()
            } else {
                url.clone()
            },
            url,
            loading: false,
            suspended: false,
            last_active: chrono::Utc::now().timestamp(),
            load_ms: None,
            http_status: None,
            navigation_error: None,
            started: None,
        }
    }
}
#[derive(Clone, Serialize, Deserialize, Default)]
pub struct Session {
    pub tabs: Vec<Tab>,
    pub active: String,
}
#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct Bounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}
impl Default for Bounds {
    fn default() -> Self {
        Self {
            x: 64.,
            y: 112.,
            width: 1000.,
            height: 600.,
        }
    }
}
pub fn emit(app: &AppHandle) {
    let state = app.state::<AppState>();
    let snapshot = state.session.lock().unwrap().clone();
    let _ = app.emit_to("shell", "browser-changed", &snapshot);
}
pub fn persist(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let s = state.session.lock().unwrap().clone();
    let profile = state.profile.lock().unwrap().clone();
    let result = state.store.lock().unwrap().save_session(
        &profile,
        &serde_json::to_string(&s).map_err(|e| e.to_string())?,
    );
    result
}
pub fn restore(app: &AppHandle) {
    let state = app.state::<AppState>();
    let p = state.profile.lock().unwrap().clone();
    let mut s = state
        .store
        .lock()
        .unwrap()
        .load_session(&p)
        .and_then(|v| serde_json::from_str::<Session>(&v).ok())
        .unwrap_or_default();
    s.tabs.retain(|t| security::destination(&t.url).is_ok());
    s.tabs.truncate(100);
    let active_index = s.tabs.iter().position(|t| t.id == s.active).unwrap_or(0);
    for t in &mut s.tabs {
        t.id = format!("tab-{}", uuid::Uuid::new_v4());
        t.loading = false;
        t.suspended = !t.url.starts_with("elix:");
        t.started = None;
    }
    if s.tabs.is_empty() {
        s.tabs.push(Tab::new("elix://start".into()));
    }
    s.active = s.tabs[active_index.min(s.tabs.len() - 1)].id.clone();
    *state.session.lock().unwrap() = s;
}
pub async fn create(app: AppHandle, input: String) -> Result<String, String> {
    let url = security::destination(&input)?;
    let tab = Tab::new(url);
    let id = tab.id.clone();
    {
        let state = app.state::<AppState>();
        let mut s = state.session.lock().unwrap();
        if s.tabs.len() >= 100 {
            return Err("Maksimalt 100 faner".into());
        }
        s.tabs.push(tab);
    }
    activate(&app, &id)?;
    persist(&app)?;
    Ok(id)
}
pub fn activate(app: &AppHandle, id: &str) -> Result<(), String> {
    let state = app.state::<AppState>();
    let tab = {
        let mut s = state.session.lock().unwrap();
        let tab = s
            .tabs
            .iter_mut()
            .find(|t| t.id == id)
            .ok_or("Fant ikke fanen")?;
        tab.last_active = chrono::Utc::now().timestamp();
        let t = tab.clone();
        s.active = id.into();
        t
    };
    for (label, view) in app.webviews() {
        if label.starts_with("tab-") {
            view.hide().map_err(|e| e.to_string())?;
        }
    }
    if !tab.url.starts_with("elix:") {
        if app.get_webview(id).is_none() {
            engine::create(app, &tab)?;
        }
        let view = app
            .get_webview(id)
            .ok_or("Nettvisningen kunne ikke opprettes")?;
        engine::bounds(&view, *state.bounds.lock().unwrap())?;
        view.show().map_err(|e| e.to_string())?;
        view.set_focus().map_err(|e| e.to_string())?;
        if let Some(t) = state
            .session
            .lock()
            .unwrap()
            .tabs
            .iter_mut()
            .find(|t| t.id == id)
        {
            t.suspended = false;
        }
    } else if let Some(shell) = app.get_webview("shell") {
        shell.set_focus().map_err(|e| e.to_string())?;
    }
    emit(app);
    Ok(())
}
pub fn navigate(app: &AppHandle, id: &str, input: &str) -> Result<(), String> {
    let url = security::destination(input)?;
    {
        let state = app.state::<AppState>();
        let mut s = state.session.lock().unwrap();
        let tab = s
            .tabs
            .iter_mut()
            .find(|t| t.id == id)
            .ok_or("Fant ikke fanen")?;
        tab.url = url.clone();
        tab.navigation_error = None;
    }
    if url.starts_with("elix:") {
        if let Some(v) = app.get_webview(id) {
            v.close().map_err(|e| e.to_string())?;
        }
    } else if let Some(v) = app.get_webview(id) {
        v.navigate(url.parse().map_err(|_| "Ugyldig URL")?)
            .map_err(|e| e.to_string())?;
    }
    activate(app, id)?;
    persist(app)
}
pub fn close(app: &AppHandle, id: &str) -> Result<(), String> {
    if let Some(v) = app.get_webview(id) {
        v.close().map_err(|e| e.to_string())?;
    }
    let next = {
        let state = app.state::<AppState>();
        let mut s = state.session.lock().unwrap();
        let index = s
            .tabs
            .iter()
            .position(|t| t.id == id)
            .ok_or("Fant ikke fanen")?;
        s.tabs.remove(index);
        if s.tabs.is_empty() {
            s.tabs.push(Tab::new("elix://start".into()));
        }
        if s.active == id {
            s.active = s.tabs[index.min(s.tabs.len() - 1)].id.clone();
        }
        s.active.clone()
    };
    activate(app, &next)?;
    persist(app)
}
pub fn suspend(app: &AppHandle, id: &str) -> Result<(), String> {
    let state = app.state::<AppState>();
    {
        let s = state.session.lock().unwrap();
        if s.active == id {
            return Err("Bytt fane før du setter den i dvale".into());
        }
        if !s.tabs.iter().any(|t| t.id == id) {
            return Err("Fant ikke fanen".into());
        }
    }
    if let Some(v) = app.get_webview(id) {
        v.close().map_err(|e| e.to_string())?;
    }
    if let Some(t) = state
        .session
        .lock()
        .unwrap()
        .tabs
        .iter_mut()
        .find(|t| t.id == id)
    {
        t.suspended = true;
        t.loading = false;
    }
    emit(app);
    persist(app)
}
