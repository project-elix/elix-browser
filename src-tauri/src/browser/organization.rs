use super::{engine, Bounds, Tab};
use crate::{security, AppState};
use tauri::{AppHandle, Manager};

pub fn default_workspace() -> String {
    "Personlig".into()
}
pub fn default_workspaces() -> Vec<String> {
    vec![default_workspace()]
}

pub fn layout(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let s = state.session.lock().unwrap().clone();
    let b = *state.bounds.lock().unwrap();
    for (id, view) in app.webviews() {
        if !id.starts_with("tab-") {
            continue;
        }
        if state
            .permissions_visible
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            view.hide().map_err(|e| e.to_string())?;
            continue;
        }
        if let Some(pair) = &s.split {
            if let Some(index) = pair.iter().position(|x| x == &id) {
                let width = (b.width - 8.).max(2.) / 2.;
                engine::bounds(
                    &view,
                    Bounds {
                        x: b.x + index as f64 * (width + 8.),
                        width,
                        ..b
                    },
                )?;
                view.show().map_err(|e| e.to_string())?;
                continue;
            }
        } else if id == s.active {
            engine::bounds(&view, b)?;
            view.show().map_err(|e| e.to_string())?;
            continue;
        }
        view.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn reopen(app: &AppHandle) -> Result<(), String> {
    let id = {
        let state = app.state::<AppState>();
        let mut s = state.session.lock().unwrap();
        if s.tabs.len() >= 100 {
            return Err("Maksimalt 100 faner".into());
        }
        if s.closed.is_empty() {
            return Err("Ingen lukkede faner å gjenåpne".into());
        }
        let mut tab = s.closed.remove(0);
        tab.id = format!("tab-{}", uuid::Uuid::new_v4());
        tab.crashed = false;
        tab.loading = false;
        let id = tab.id.clone();
        s.tabs.push(tab);
        id
    };
    super::activate(app, &id)?;
    super::persist(app)
}

pub fn workspace(app: &AppHandle, name: &str) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 40 {
        return Err("Navnet må være 1–40 tegn".into());
    }
    let id = {
        let state = app.state::<AppState>();
        let mut s = state.session.lock().unwrap();
        if !s.workspaces.iter().any(|w| w == name) {
            if s.workspaces.len() >= 20 {
                return Err("Maksimalt 20 arbeidsområder".into());
            }
            s.workspaces.push(name.into());
        }
        s.workspace = name.into();
        s.split = None;
        if let Some(t) = s.tabs.iter().find(|t| t.workspace == name) {
            t.id.clone()
        } else {
            if s.tabs.len() >= 100 {
                return Err("Maksimalt 100 faner".into());
            }
            let mut tab = Tab::new("elix://start".into());
            tab.workspace = name.into();
            let id = tab.id.clone();
            s.tabs.push(tab);
            id
        }
    };
    super::activate(app, &id)?;
    super::persist(app)
}

pub fn group(app: &AppHandle, id: &str, group: &str, workspace: &str) -> Result<(), String> {
    if group.chars().count() > 40 {
        return Err("Gruppenavnet er for langt".into());
    }
    {
        let state = app.state::<AppState>();
        let mut s = state.session.lock().unwrap();
        if !s.workspaces.iter().any(|w| w == workspace) {
            return Err("Ukjent arbeidsområde".into());
        }
        let tab = s
            .tabs
            .iter_mut()
            .find(|t| t.id == id)
            .ok_or("Ukjent fane")?;
        tab.group = group.trim().into();
        tab.workspace = workspace.into();
    }
    super::activate(app, id)?;
    super::persist(app)
}

pub fn reorder(app: &AppHandle, id: &str, before: &str) -> Result<(), String> {
    {
        let state = app.state::<AppState>();
        let mut s = state.session.lock().unwrap();
        let source = s.tabs.iter().find(|t| t.id == id).ok_or("Ukjent fane")?;
        let target = s
            .tabs
            .iter()
            .find(|t| t.id == before)
            .ok_or("Ukjent fane")?;
        if source.workspace != target.workspace || source.pinned != target.pinned {
            return Err("Flytt innenfor samme arbeidsområde og festede/vanlige faner".into());
        }
        if id == before {
            return Ok(());
        }
        let from = s.tabs.iter().position(|t| t.id == id).unwrap();
        let to = s.tabs.iter().position(|t| t.id == before).unwrap();
        let tab = s.tabs.remove(from);
        s.tabs.insert(to, tab);
    }
    super::emit(app);
    super::persist(app)
}

pub fn split(app: &AppHandle, other: Option<String>) -> Result<(), String> {
    let state = app.state::<AppState>();
    if let Some(id) = other {
        let (active, tab) = {
            let s = state.session.lock().unwrap();
            let a = s
                .tabs
                .iter()
                .find(|t| t.id == s.active)
                .ok_or("Ukjent fane")?;
            let b = s.tabs.iter().find(|t| t.id == id).ok_or("Ukjent fane")?;
            if a.id == b.id
                || a.workspace != b.workspace
                || a.private != b.private
                || a.url.starts_with("elix:")
                || b.url.starts_with("elix:")
            {
                return Err("Velg to nettsider i samme arbeidsområde og samme privatmodus".into());
            }
            (a.id.clone(), b.clone())
        };
        if app.get_webview(&id).is_none() {
            engine::create(app, &tab)?;
        }
        state.session.lock().unwrap().split = Some([active, id]);
    } else {
        state.session.lock().unwrap().split = None;
    }
    layout(app)?;
    super::emit(app);
    Ok(())
}

pub async fn mute(app: &AppHandle, id: &str, muted: bool) -> Result<(), String> {
    if let Some(view) = app.get_webview(id) {
        engine::action(view, "mute".into(), muted).await?;
    }
    {
        let state = app.state::<AppState>();
        let mut s = state.session.lock().unwrap();
        s.tabs
            .iter_mut()
            .find(|t| t.id == id)
            .ok_or("Ukjent fane")?
            .muted = muted;
    }
    super::emit(app);
    super::persist(app)
}

pub fn recover(app: &AppHandle, id: &str) -> Result<(), String> {
    let state = app.state::<AppState>();
    if state.downloads.lock().unwrap().iter().any(|d| {
        d.view_id == id && (d.can_resume || matches!(d.status.as_str(), "downloading" | "paused"))
    }) {
        return Err(
            "Fullfør eller avbryt nedlastingen før nettvisningen opprettes på nytt.".into(),
        );
    }
    if !state
        .session
        .lock()
        .unwrap()
        .tabs
        .iter()
        .any(|t| t.id == id && security::destination(&t.url).is_ok())
    {
        return Err("Ukjent fane".into());
    }
    if let Some(view) = app.get_webview(id) {
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
        t.crashed = false;
    }
    super::activate(app, id)
}
