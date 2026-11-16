pub mod detach;
pub mod engine;
pub mod extensions;
#[cfg(windows)]
mod favicon;
pub mod organization;
pub mod private;
pub mod site_data;
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
    pub private: bool,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default = "organization::default_workspace")]
    pub workspace: String,
    #[serde(default)]
    pub group: String,
    #[serde(default)]
    pub muted: bool,
    #[serde(skip)]
    pub crashed: bool,
    #[serde(default, skip_deserializing)]
    pub favicon: Option<String>,
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
            private: false,
            pinned: false,
            workspace: organization::default_workspace(),
            group: String::new(),
            muted: false,
            crashed: false,
            favicon: None,
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
    #[serde(default)]
    pub closed: Vec<Tab>,
    #[serde(default = "organization::default_workspaces")]
    pub workspaces: Vec<String>,
    #[serde(default = "organization::default_workspace")]
    pub workspace: String,
    #[serde(skip)]
    pub split: Option<[String; 2]>,
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
    let s = state.session.lock().unwrap().persistent();
    let profile = state.profile.lock().unwrap().clone();
    let data = serde_json::to_string(&s).map_err(|e| e.to_string())?;
    let store = state.store.lock().unwrap();
    let result = if state.window_id == "main" {
        store.save_session(&profile, &data)
    } else {
        store.set(
            &format!("window_session:{}:{profile}", state.window_id),
            &data,
        )
    };
    result
}
pub fn restore(app: &AppHandle) {
    let state = app.state::<AppState>();
    let p = state.profile.lock().unwrap().clone();
    let candidates = {
        let store = state.store.lock().unwrap();
        if state.window_id == "main" {
            store.session_candidates(&p)
        } else {
            store
                .get(&format!("window_session:{}:{p}", state.window_id))
                .into_iter()
                .collect()
        }
    };
    let mut s = candidates
        .into_iter()
        .find_map(|v| serde_json::from_str::<Session>(&v).ok())
        .unwrap_or_default();
    s.tabs
        .retain(|t| !t.private && security::destination(&t.url).is_ok());
    s.tabs.truncate(100);
    if s.workspaces.is_empty() {
        s.workspaces = organization::default_workspaces();
    }
    s.closed
        .retain(|t| !t.private && security::destination(&t.url).is_ok());
    s.closed.truncate(20);
    for tab in &s.tabs {
        if !s.workspaces.contains(&tab.workspace) {
            s.workspaces.push(tab.workspace.clone());
        }
    }
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
    s.workspace = s.tabs[active_index.min(s.tabs.len() - 1)].workspace.clone();
    *state.session.lock().unwrap() = s;
}
pub async fn create(app: AppHandle, input: String) -> Result<String, String> {
    create_with_mode(app, input, false).await
}
pub async fn create_with_mode(
    app: AppHandle,
    input: String,
    private: bool,
) -> Result<String, String> {
    let url = security::destination(&input)?;
    let mut tab = Tab::new(url);
    tab.private = private;
    let id = tab.id.clone();
    {
        let state = app.state::<AppState>();
        let mut s = state.session.lock().unwrap();
        if s.tabs.len() >= 100 {
            return Err("Maksimalt 100 faner".into());
        }
        tab.workspace = s.workspace.clone();
        s.tabs.push(tab);
    }
    if let Err(error) = activate(&app, &id) {
        // Keep the tab visible with an actionable failure instead of a blank view.
        failed(&app, &id, &error);
        let _ = persist(&app);
        return Err(error);
    }
    persist(&app)?;
    Ok(id)
}
pub fn activate(app: &AppHandle, id: &str) -> Result<(), String> {
    let state = app.state::<AppState>();
    #[cfg(windows)]
    if state.session.lock().unwrap().active != id {
        crate::security::permissions::cancel_all(app);
    }
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
        s.workspace = t.workspace.clone();
        if s.split
            .as_ref()
            .is_some_and(|pair| !pair.iter().any(|x| x == id))
        {
            s.split = None;
        }
        t
    };
    for (label, view) in app.webviews() {
        if label.starts_with("tab-") {
            view.hide().map_err(|e| e.to_string())?;
        }
    }
    if !tab.url.starts_with("elix:") {
        if app.get_webview(id).is_none() {
            {
                let mut session = state.session.lock().unwrap();
                if let Some(t) = session.tabs.iter_mut().find(|t| t.id == id) {
                    t.loading = true;
                    t.navigation_error = None;
                    t.started = Some(std::time::Instant::now());
                }
            }
            emit(app);
            if let Err(error) = engine::create(app, &tab) {
                failed(app, id, &error);
                return Err(error);
            }
        }
        let view = app
            .get_webview(id)
            .ok_or("Nettvisningen kunne ikke opprettes")?;
        organization::layout(app)?;
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
    organization::layout(app)?;
    emit(app);
    Ok(())
}
pub fn navigate(app: &AppHandle, id: &str, input: &str) -> Result<(), String> {
    let url = security::destination(input)?;
    // Internal panels must not close the native view that owns a live transfer.
    if url.starts_with("elix:") {
        let state = app.state::<AppState>();
        let transfer_active = state.downloads.lock().unwrap().iter().any(|d| {
            d.view_id == id
                && (d.can_resume || matches!(d.status.as_str(), "downloading" | "paused"))
        });
        if transfer_active {
            let next = {
                let mut session = state.session.lock().unwrap();
                let source = session
                    .tabs
                    .iter()
                    .find(|t| t.id == id)
                    .ok_or("Fant ikke fanen")?;
                let private = source.private;
                if session.tabs.len() >= 100 {
                    return Err("Maksimalt 100 faner. Lukk en fane og prøv igjen.".into());
                }
                let mut tab = Tab::new(url);
                tab.private = private;
                let next = tab.id.clone();
                session.tabs.push(tab);
                next
            };
            activate(app, &next)?;
            return persist(app);
        }
    }

    #[cfg(windows)]
    crate::security::permissions::cancel(app, id);
    let private;
    {
        let state = app.state::<AppState>();
        let mut s = state.session.lock().unwrap();
        let tab = s
            .tabs
            .iter_mut()
            .find(|t| t.id == id)
            .ok_or("Fant ikke fanen")?;
        private = tab.private;
        if tab.url != url {
            tab.favicon = None;
        }
        tab.url = url.clone();
        tab.loading = !url.starts_with("elix:");
        tab.started = tab.loading.then(std::time::Instant::now);
        tab.http_status = None;
        tab.navigation_error = None;
        if url.starts_with("elix:") {
            s.split = None;
        }
    }
    emit(app);
    if url.starts_with("elix:") {
        if let Some(v) = app.get_webview(id) {
            v.close().map_err(|e| e.to_string())?;
        }
    } else if let Some(v) = app.get_webview(id) {
        if private {
            private::launch(&v, &url)?;
        } else {
            v.navigate(url.parse().map_err(|_| "Ugyldig URL")?)
                .map_err(|e| e.to_string())?;
        }
    }
    activate(app, id)?;
    persist(app)
}
pub fn close(app: &AppHandle, id: &str) -> Result<(), String> {
    if app
        .state::<AppState>()
        .downloads
        .lock()
        .unwrap()
        .iter()
        .any(|d| {
            d.view_id == id
                && (d.can_resume || matches!(d.status.as_str(), "downloading" | "paused"))
        })
    {
        return Err("Fanen har en aktiv nedlasting. Fullfør eller avbryt den i Nedlastinger (Ctrl+J) før du lukker fanen.".into());
    }

    #[cfg(windows)]
    crate::security::permissions::cancel(app, id);
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
        let closed = s.tabs.remove(index);
        if !closed.private {
            s.closed.insert(0, closed);
            s.closed.truncate(20);
        }
        if s.split
            .as_ref()
            .is_some_and(|pair| pair.iter().any(|x| x == id))
        {
            s.split = None;
        }
        if s.tabs.is_empty() {
            s.tabs.push(Tab::new("elix://start".into()));
        }
        if s.active == id {
            s.active = s.tabs[index.min(s.tabs.len() - 1)].id.clone();
        }
        s.active.clone()
    };
    private::cleanup_if_empty(app);
    activate(app, &next)?;
    persist(app)
}
pub fn suspend(app: &AppHandle, id: &str) -> Result<(), String> {
    let state = app.state::<AppState>();
    {
        let s = state.session.lock().unwrap();
        if s.active == id || s.split.as_ref().is_some_and(|p| p.iter().any(|x| x == id)) {
            return Err("Bytt fane før du setter den i dvale".into());
        }
        let tab = s
            .tabs
            .iter()
            .find(|t| t.id == id)
            .ok_or("Fant ikke fanen")?;
        if tab.private {
            return Err("Private faner holdes aktive til de lukkes.".into());
        }
        if tab.pinned {
            return Err("Festede faner holdes aktive. Løsne fanen for å sette den i dvale.".into());
        }
    }
    if state.downloads.lock().unwrap().iter().any(|d| {
        d.view_id == id && (d.can_resume || matches!(d.status.as_str(), "downloading" | "paused"))
    }) {
        return Err("Fanen har en nedlasting. Fullfør eller avbryt nedlastingen først.".into());
    }
    #[cfg(windows)]
    crate::security::permissions::cancel(app, id);
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

/// A snapshot suitable for disk; private URLs, titles and favicons never leave memory.
impl Session {
    pub fn persistent(&self) -> Self {
        let mut tabs: Vec<_> = self.tabs.iter().filter(|t| !t.private).cloned().collect();
        for tab in &mut tabs {
            tab.favicon = None;
        }
        let active = if tabs.iter().any(|t| t.id == self.active) {
            self.active.clone()
        } else {
            tabs.first().map(|t| t.id.clone()).unwrap_or_default()
        };
        let mut closed: Vec<_> = self
            .closed
            .iter()
            .filter(|t| !t.private)
            .take(20)
            .cloned()
            .collect();
        for tab in &mut closed {
            tab.favicon = None;
        }
        Self {
            tabs,
            active,
            closed,
            workspaces: self.workspaces.clone(),
            workspace: self.workspace.clone(),
            split: None,
        }
    }
}
pub fn pin(app: &AppHandle, id: &str, pinned: bool) -> Result<(), String> {
    {
        let state = app.state::<AppState>();
        let mut session = state.session.lock().unwrap();
        session
            .tabs
            .iter_mut()
            .find(|t| t.id == id)
            .ok_or("Fant ikke fanen")?
            .pinned = pinned;
        session.tabs.sort_by_key(|t| !t.pinned);
    }
    emit(app);
    persist(app)
}
pub fn failed(app: &AppHandle, id: &str, message: &str) {
    {
        let state = app.state::<AppState>();
        let mut session = state.session.lock().unwrap();
        if let Some(tab) = session.tabs.iter_mut().find(|t| t.id == id) {
            tab.loading = false;
            tab.started = None;
            tab.navigation_error = Some(message.into());
        }
    }
    emit(app);
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn private_tabs_and_favicons_never_reach_session_storage() {
        let mut regular = Tab::new("https://example.com/".into());
        regular.pinned = true;
        regular.favicon = Some("data:image/png;base64,secret".into());
        let mut private = Tab::new("https://private.example/secret".into());
        private.private = true;
        let session = Session {
            active: private.id.clone(),
            tabs: vec![regular.clone(), private],
            ..Default::default()
        };
        let persisted = session.persistent();
        assert_eq!(persisted.tabs.len(), 1);
        assert_eq!(persisted.active, regular.id);
        assert!(persisted.tabs[0].pinned);
        let json = serde_json::to_string(&persisted).unwrap();
        assert!(!json.contains("secret"));
        assert!(!json.contains("private.example"));
    }
}
