pub mod detach;
#[cfg(windows)]
pub mod window_transfer;
mod dormancy;
pub mod engine;
pub mod extensions;
pub mod media;
#[cfg(windows)]
mod favicon;
#[cfg(windows)]
pub mod tab_preview;
pub mod organization;
pub mod group_rules;
pub mod saved_sessions;
pub mod private;
pub mod site_data;
#[cfg(feature = "smoke-tests")]
pub mod smoke;
#[cfg(windows)]
mod windows;
use crate::{security, AppState};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
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
    pub group_manual: Option<bool>,
    #[serde(default)]
    pub muted: bool,
    #[serde(skip)]
    pub crashed: bool,
    #[serde(default, skip_deserializing)]
    pub favicon: Option<String>,
    #[serde(default)]
    pub loading: bool,
    #[serde(default, skip_deserializing)]
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
            group_manual: None,
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

    fn reset_runtime_state(&mut self, now: i64) {
        self.loading = false;
        self.suspended = false;
        self.crashed = false;
        self.favicon = None;
        self.load_ms = None;
        self.http_status = None;
        self.navigation_error = None;
        self.started = None;
        self.last_active = now;
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
    #[serde(default)]
    pub group_rules: group_rules::GroupRules,
    #[serde(default = "organization::default_workspace")]
    pub workspace: String,
    #[serde(default)]
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
    // Serialize in database-write order so an older callback cannot overwrite a newer layout.
    let store = state.store.lock().unwrap();
    let profile = state.profile.lock().unwrap().clone();
    let data = saved_sessions::encode(&state.session.lock().unwrap(), false)?;
    let result = if state.window_id == "main" {
        store.save_session(&profile, &data)
    } else {
        store.save_window_session(&state.window_id, &profile, &data)
    };
    result
}
pub fn restore(app: &AppHandle) {
    let profile = app.state::<AppState>().profile.lock().unwrap().clone();
    let _ = restore_profile(app, &profile, false);
}

pub fn restore_profile(app: &AppHandle, profile: &str, select: bool) -> Result<(),String> {
    let state = app.state::<AppState>();
    // Snapshot reads and writes also take the store first. No callback can save the
    // previous profile's tabs under the incoming profile's ID during this transition.
    let store = state.store.lock().unwrap();
    let candidates = if state.window_id == "main" {
        store.session_candidates(profile)
    } else {
        store.window_session_candidates(&state.window_id, profile)
    };
    let mut session = candidates.iter().find_map(|v| saved_sessions::decode(v)).unwrap_or_else(|| {
        let mut empty = Session::default();
        empty.prepare_restore(chrono::Utc::now().timestamp());
        empty
    });
    session.group_rules = group_rules::load(&store, profile);
    group_rules::apply_all(&mut session);
    if select { store.set("active_profile",profile)?; }
    *state.profile.lock().unwrap() = profile.into();
    *state.session.lock().unwrap() = session;
    Ok(())
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
    group_rules::refresh(&app);
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
        group_rules::apply(&mut s,&id);
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
    media::before_activate(app, id)?;
    let state = app.state::<AppState>();
    #[cfg(windows)]
    let previous = state.session.lock().unwrap().active.clone();
    let tab = state
        .session
        .lock()
        .unwrap()
        .select_tab(id, chrono::Utc::now().timestamp())?;
    #[cfg(windows)]
    if previous != id {
        crate::security::permissions::cancel_all(app);
    }
    for (label, view) in app.webviews() {
        if label.starts_with("tab-") && !media::is_player(app, &label) {
            view.hide().map_err(|e| e.to_string())?;
        }
    }
    if !tab.url.starts_with("elix:") && !media::is_player(app, id) {
        if app.get_webview(id).is_none() {
            {
                let mut session = state.session.lock().unwrap();
                if let Some(t) = session.tabs.iter_mut().find(|t| t.id == id) {
                    t.loading = true;
                    t.suspended = false;
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
        // A restored split has fresh IDs and no native views yet. Wake both panes.
        let peer = {
            let s = state.session.lock().unwrap();
            s.split.as_ref().and_then(|pair| pair.iter().find(|other| other.as_str() != id))
                .and_then(|other| s.tabs.iter().find(|t| &t.id == other)).cloned()
        };
        if let Some(peer) = peer {
            if app.get_webview(&peer.id).is_none() { engine::create(app, &peer)?; }
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
    media::before_navigation(app, id)?;
    group_rules::refresh(app);
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
                let workspace=source.workspace.clone();
                if session.tabs.len() >= 100 {
                    return Err("Maksimalt 100 faner. Lukk en fane og prøv igjen.".into());
                }
                let mut tab = Tab::new(url);
                tab.private = private;
                tab.workspace=workspace;
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
        group_rules::apply(&mut s,id);
        if url.starts_with("elix:") {
            s.clear_split(chrono::Utc::now().timestamp());
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
    close_inner(app, id, true)
}
pub fn close_transferred(app: &AppHandle, id: &str) -> Result<(), String> {
    close_inner(app, id, false)
}
fn close_inner(app: &AppHandle, id: &str, remember: bool) -> Result<(), String> {
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
    media::release(app, id)?;
    if let Some(v) = app.get_webview(id) {
        v.close().map_err(|e| e.to_string())?;
    }
    let (next, ended_split) = {
        let state = app.state::<AppState>();
        let mut s = state.session.lock().unwrap();
        let index = s
            .tabs
            .iter()
            .position(|t| t.id == id)
            .ok_or("Fant ikke fanen")?;
        let closed = s.tabs.remove(index);
        if remember && !closed.private {
            s.closed.insert(0, closed);
            s.closed.truncate(20);
        }
        let ended_split = s.split
            .as_ref()
            .is_some_and(|pair| pair.iter().any(|x| x == id));
        if ended_split {
            s.clear_split(chrono::Utc::now().timestamp());
        }
        if s.tabs.is_empty() {
            s.tabs.push(Tab::new("elix://start".into()));
        }
        let next = if s.active == id {
            s.active = s.tabs[index.min(s.tabs.len() - 1)].id.clone();
            Some(s.active.clone())
        } else {
            None
        };
        (next, ended_split)
    };
    private::cleanup_if_empty(app);
    if let Some(next) = next {
        activate(app, &next)?;
    } else {
        if ended_split {
            organization::layout(app)?;
        }
        emit(app);
    }
    persist(app)
}
pub fn suspend(app: &AppHandle, id: &str) -> Result<(), String> {
    park_background(app, id, None)
}

pub fn park_background(app: &AppHandle, id: &str, automatic_minutes: Option<u32>) -> Result<(), String> {
    let view = app.get_webview(id);
    if let Some(view) = &view {
        if !dormancy::idle(view)? {
            return Err("Fanen holdes våken på grunn av avspilling, et møte eller aktivitet som ikke kan kontrolleres.".into());
        }
    } else if automatic_minutes.is_some() {
        return Ok(());
    }
    // Recheck eligibility after the renderer replies. The user may have
    // selected/pinned the tab or begun a download while the probe was pending.
    if media::is_player(app, id) {
        return Err("Minispilleren holdes våken. Flytt den tilbake til nettleseren før dvale.".into());
    }
    let state = app.state::<AppState>();
    if automatic_minutes.is_some_and(|minutes| state.store.lock().unwrap().settings().suspend_minutes != minutes) {
        return Ok(());
    }
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
        if let Some(minutes) = automatic_minutes {
            if minutes == 0 || chrono::Utc::now().timestamp().saturating_sub(tab.last_active) < i64::from(minutes) * 60 {
                return Ok(());
            }
        }
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
    if let Some(view) = view {
        view.close().map_err(|e| e.to_string())?;
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
        t.started = None;
    }
    emit(app);
    persist(app)
}

impl Session {
    /// Restored pages stay unloaded until selected, but are not sleeping views.
    pub fn prepare_restore(&mut self, now: i64) {
        self.tabs
            .retain(|t| !t.private && security::stored_destination_allowed(&t.url));
        self.tabs.truncate(100);
        if self.tabs.is_empty() {
            self.tabs.push(Tab::new("elix://start".into()));
        }
        if self.workspaces.is_empty() {
            self.workspaces = organization::default_workspaces();
        }
        self.closed
            .retain(|t| !t.private && security::stored_destination_allowed(&t.url));
        self.closed.truncate(20);
        for tab in &self.tabs {
            if !self.workspaces.contains(&tab.workspace) {
                self.workspaces.push(tab.workspace.clone());
            }
        }
        let active_index = self
            .tabs
            .iter()
            .position(|t| t.id == self.active)
            .unwrap_or(0);
        let split_indices = saved_sessions::split_indices(self);
        for tab in &mut self.tabs {
            tab.id = format!("tab-{}", uuid::Uuid::new_v4());
            tab.reset_runtime_state(now);
        }
        for tab in &mut self.closed {
            tab.reset_runtime_state(now);
        }
        self.active = self.tabs[active_index].id.clone();
        self.workspace = self.tabs[active_index].workspace.clone();
        self.split = split_indices.map(|[a,b]| [self.tabs[a].id.clone(), self.tabs[b].id.clone()]);
    }

    /// Start the idle period when a visible tab moves into the background.
    pub fn select_tab(&mut self, id: &str, now: i64) -> Result<Tab, String> {
        let index = self
            .tabs
            .iter()
            .position(|t| t.id == id)
            .ok_or("Fant ikke fanen")?;
        let leaving_split = self
            .split
            .as_ref()
            .is_some_and(|pair| !pair.iter().any(|x| x == id));
        for tab in &mut self.tabs {
            if tab.id == self.active || tab.id == id {
                tab.last_active = now;
            }
        }
        let tab = self.tabs[index].clone();
        self.active = id.into();
        self.workspace = tab.workspace.clone();
        if leaving_split {
            self.clear_split(now);
        }
        Ok(tab)
    }

    pub fn clear_split(&mut self, now: i64) {
        if let Some(pair) = self.split.take() {
            for tab in &mut self.tabs {
                if pair.contains(&tab.id) {
                    tab.last_active = now;
                }
            }
        }
    }

    /// Install a validated pair after its native views have been created.
    pub fn replace_split(&mut self, pair: [String; 2], now: i64) {
        self.clear_split(now);
        self.split = Some(pair);
    }

    pub fn automatic_suspend_ids(
        &self,
        minutes: u32,
        now: i64,
        loaded: &HashSet<String>,
        transfers: &HashSet<String>,
    ) -> Vec<String> {
        if minutes == 0 {
            return Vec::new();
        }
        let threshold = i64::from(minutes) * 60;
        self.tabs
            .iter()
            .filter(|tab| {
                loaded.contains(&tab.id)
                    && tab.id != self.active
                    && !self
                        .split
                        .as_ref()
                        .is_some_and(|pair| pair.contains(&tab.id))
                    && !tab.private
                    && !tab.pinned
                    && !tab.suspended
                    && !tab.url.starts_with("elix:")
                    && !transfers.contains(&tab.id)
                    && now.saturating_sub(tab.last_active) >= threshold
            })
            .map(|tab| tab.id.clone())
            .collect()
    }

    /// A snapshot suitable for disk; private content and dormancy stay in memory.
    pub fn persistent(&self) -> Self {
        let eligible = |t: &&Tab| !t.private && security::stored_destination_allowed(&t.url);
        let mut tabs: Vec<_> = self.tabs.iter().filter(eligible).cloned().collect();
        for tab in &mut tabs {
            tab.reset_runtime_state(0);
        }
        let active = if tabs.iter().any(|t| t.id == self.active) {
            self.active.clone()
        } else {
            tabs.first().map(|t| t.id.clone()).unwrap_or_default()
        };
        let mut closed: Vec<_> = self
            .closed
            .iter()
            .filter(eligible)
            .take(20)
            .cloned()
            .collect();
        for tab in &mut closed {
            tab.reset_runtime_state(0);
        }
        let workspace = tabs.iter().find(|t| t.id == active).map(|t| t.workspace.clone())
            .unwrap_or_else(organization::default_workspace);
        let workspaces:Vec<_> = self.workspaces.iter().filter(|name|
            tabs.iter().any(|t| &t.workspace == *name) || !self.tabs.iter().any(|t| &t.workspace == *name && t.private)
        ).cloned().collect();
        let split = saved_sessions::split_indices(self)
            .filter(|[a,b]| tabs.iter().any(|t| t.id == self.tabs[*a].id) && tabs.iter().any(|t| t.id == self.tabs[*b].id))
            .and(self.split.clone());
        Self {
            tabs,
            active,
            closed,
            group_rules:self.group_rules.iter().filter(|(name,_)|workspaces.contains(name)).map(|(k,v)|(k.clone(),v.clone())).collect(),
            workspaces,
            workspace,
            split,
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
            tab.suspended = false;
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
    fn restoring_old_sleep_state_keeps_tabs_lazy_without_marking_them_dormant() {
        let mut first = Tab::new("https://example.com/first".into());
        first.loading = true;
        first.suspended = true;
        first.last_active = 1;
        first.load_ms = Some(42);
        first.http_status = Some(503);
        first.navigation_error = Some("old failure".into());
        let mut selected = first.clone();
        selected.id = "selected-before-restart".into();
        selected.url = "https://example.com/selected".into();
        selected.workspace = "Work".into();
        selected.pinned = true;
        let old_ids = HashSet::from([first.id.clone(), selected.id.clone()]);
        let session = Session {
            active: selected.id.clone(),
            tabs: vec![first.clone(), selected],
            closed: vec![first],
            ..Default::default()
        };
        // Old saved sessions contained the runtime flag; the shell still needs it.
        let old_snapshot = serde_json::to_value(&session).unwrap();
        assert_eq!(old_snapshot["tabs"][0]["suspended"], true);
        let mut restored: Session = serde_json::from_value(old_snapshot).unwrap();
        assert!(!restored.tabs[0].suspended);
        restored.tabs[0].crashed = true;
        restored.tabs[0].started = Some(std::time::Instant::now());
        restored.prepare_restore(10_000);
        let selected = restored
            .tabs
            .iter()
            .find(|tab| tab.id == restored.active)
            .unwrap();
        assert_eq!(selected.url, "https://example.com/selected");
        assert!(selected.pinned);
        assert_eq!(restored.workspace, "Work");
        assert!(restored.workspaces.contains(&"Work".to_string()));
        assert!(restored.tabs.iter().all(|tab| !old_ids.contains(&tab.id)));
        for tab in restored.tabs.iter().chain(&restored.closed) {
            assert!(!tab.loading && !tab.suspended && !tab.crashed);
            assert!(tab.load_ms.is_none() && tab.http_status.is_none());
            assert!(tab.navigation_error.is_none() && tab.started.is_none());
            assert_eq!(tab.last_active, 10_000);
        }
        // Background restoration does not create native views or make them candidates.
        assert!(restored
            .automatic_suspend_ids(15, 20_000, &HashSet::new(), &HashSet::new())
            .is_empty());
    }

    #[test]
    fn persistence_does_not_turn_runtime_dormancy_into_a_saved_preference() {
        let mut tab = Tab::new("https://example.com/".into());
        tab.suspended = true;
        let session = Session {
            active: tab.id.clone(),
            tabs: vec![tab.clone()],
            closed: vec![tab],
            ..Default::default()
        };
        let saved = session.persistent();
        assert!(!saved.tabs[0].suspended && !saved.closed[0].suspended);
        assert!(session.tabs[0].suspended && session.closed[0].suspended);
    }

    #[test]
    fn idle_time_starts_when_a_tab_moves_to_the_background() {
        let mut first = Tab::new("https://example.com/first".into());
        let mut second = Tab::new("https://example.com/second".into());
        first.last_active = 0;
        second.last_active = 0;
        let first_id = first.id.clone();
        let second_id = second.id.clone();
        let loaded = HashSet::from([first_id.clone(), second_id.clone()]);
        let mut session = Session {
            active: first_id.clone(),
            tabs: vec![first, second],
            ..Default::default()
        };
        let selected = session.select_tab(&second_id, 3_600).unwrap();
        assert_eq!(selected.id, second_id);
        assert!(session.tabs.iter().all(|tab| tab.last_active == 3_600));
        assert!(session
            .automatic_suspend_ids(15, 4_499, &loaded, &HashSet::new())
            .is_empty());
        assert_eq!(
            session.automatic_suspend_ids(15, 4_500, &loaded, &HashSet::new()),
            vec![first_id]
        );
        // A failed selection must not change the selected tab or its idle clock.
        let before = serde_json::to_value(&session).unwrap();
        assert!(session.select_tab("missing", 9_999).is_err());
        assert_eq!(serde_json::to_value(&session).unwrap(), before);
    }

    #[test]
    fn leaving_split_view_starts_the_idle_period_for_both_visible_panes() {
        let mut tabs: Vec<_> = ["left", "right", "other"]
            .into_iter()
            .map(|name| Tab::new(format!("https://example.com/{name}")))
            .collect();
        for tab in &mut tabs {
            tab.last_active = 0;
        }
        let ids: Vec<_> = tabs.iter().map(|tab| tab.id.clone()).collect();
        let loaded = ids.iter().cloned().collect();
        let pair = [ids[0].clone(), ids[1].clone()];
        let mut session = Session {
            active: ids[0].clone(),
            tabs,
            split: Some(pair.clone()),
            ..Default::default()
        };
        session.select_tab(&ids[1], 1_000).unwrap();
        assert_eq!(session.split, Some(pair));
        session.select_tab(&ids[2], 2_000).unwrap();
        assert!(session.split.is_none());
        assert_eq!(session.tabs[0].last_active, 2_000);
        assert_eq!(session.tabs[1].last_active, 2_000);
        assert!(session
            .automatic_suspend_ids(15, 2_899, &loaded, &HashSet::new())
            .is_empty());
        assert_eq!(
            session.automatic_suspend_ids(15, 2_900, &loaded, &HashSet::new()),
            ids[..2].to_vec()
        );
    }

    #[test]
    fn replacing_split_starts_only_the_departing_peers_idle_period() {
        let tabs: Vec<_> = ["active", "old-peer", "new-peer", "background"]
            .into_iter()
            .map(|name| {
                let mut tab = Tab::new(format!("https://example.com/{name}"));
                tab.id = name.into();
                tab.last_active = 0;
                tab
            })
            .collect();
        let loaded = tabs.iter().map(|tab| tab.id.clone()).collect();
        let mut session = Session {
            active: "active".into(),
            tabs,
            split: Some(["active".into(), "old-peer".into()]),
            ..Default::default()
        };
        session.replace_split(["active".into(), "new-peer".into()], 3_600);
        assert_eq!(session.tabs[1].last_active, 3_600);
        assert_eq!(session.tabs[3].last_active, 0);
        assert_eq!(
            session.automatic_suspend_ids(10, 4_199, &loaded, &HashSet::new()),
            vec!["background".to_string()]
        );
        assert_eq!(
            session.automatic_suspend_ids(10, 4_200, &loaded, &HashSet::new()),
            vec!["old-peer".to_string(), "background".to_string()]
        );
    }

    #[test]
    fn automatic_dormancy_only_selects_loaded_unprotected_idle_tabs() {
        let mut tabs: Vec<_> = [
            "eligible", "active", "split", "private", "pinned", "sleeping", "transfer",
            "unloaded", "internal", "recent", "future",
        ]
        .into_iter()
        .map(|name| {
            let mut tab = Tab::new(format!("https://example.com/{name}"));
            tab.id = name.into();
            tab.last_active = 0;
            tab
        })
        .collect();
        tabs[3].private = true;
        tabs[4].pinned = true;
        tabs[5].suspended = true;
        tabs[8].url = "elix://settings".into();
        tabs[9].last_active = 1;
        tabs[10].last_active = 10_000;
        let loaded = tabs
            .iter()
            .filter(|tab| tab.id != "unloaded")
            .map(|tab| tab.id.clone())
            .collect();
        let transfers = HashSet::from(["transfer".into()]);
        let session = Session {
            active: "active".into(),
            tabs,
            split: Some(["active".into(), "split".into()]),
            ..Default::default()
        };
        assert!(session
            .automatic_suspend_ids(0, 900, &loaded, &transfers)
            .is_empty());
        assert_eq!(
            session.automatic_suspend_ids(15, 900, &loaded, &transfers),
            vec!["eligible".to_string()]
        );
    }

    #[test]
    fn ending_split_starts_peer_idle_time_without_resetting_unrelated_tabs() {
        let tabs: Vec<_> = ["left", "right", "background"]
            .into_iter()
            .map(|id| {
                let mut tab = Tab::new(format!("https://example.com/{id}"));
                tab.id = id.into();
                tab.last_active = 0;
                tab
            })
            .collect();
        let loaded = tabs.iter().map(|tab| tab.id.clone()).collect();
        let mut session = Session {
            active: "left".into(),
            tabs,
            split: Some(["left".into(), "right".into()]),
            ..Default::default()
        };
        session.clear_split(2_000);
        assert!(session.split.is_none());
        assert_eq!(session.tabs[0].last_active, 2_000);
        assert_eq!(session.tabs[1].last_active, 2_000);
        assert_eq!(session.tabs[2].last_active, 0);
        assert_eq!(
            session.automatic_suspend_ids(15, 2_899, &loaded, &HashSet::new()),
            vec!["background".to_string()]
        );
        assert_eq!(
            session.automatic_suspend_ids(15, 2_900, &loaded, &HashSet::new()),
            vec!["right".to_string(), "background".to_string()]
        );
        // Repeating the no-op must not keep an already hidden peer awake.
        session.clear_split(3_000);
        assert_eq!(session.tabs[1].last_active, 2_000);
    }

    #[test]
    fn restored_split_targets_the_same_pages_with_new_native_ids() {
        let left = Tab::new("https://example.com/left".into());
        let right = Tab::new("https://example.com/right".into());
        let pair = [left.id.clone(), right.id.clone()];
        let session = Session {
            active: left.id.clone(),
            tabs: vec![left, right],
            split: Some(pair.clone()),
            ..Default::default()
        };
        let snapshot = serde_json::to_value(&session).unwrap();
        assert_eq!(snapshot["split"], serde_json::json!(pair));
        let persisted = serde_json::to_value(session.persistent()).unwrap();
        assert_eq!(persisted["split"], serde_json::json!(pair));
        let restored = saved_sessions::decode(&saved_sessions::encode(&session, false).unwrap()).unwrap();
        assert_ne!(restored.split, Some(pair));
        assert_eq!(restored.split, Some([restored.tabs[0].id.clone(),restored.tabs[1].id.clone()]));
        assert_eq!(restored.active, restored.tabs[0].id);
    }
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
