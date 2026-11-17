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
    if super::media::layout(app)? { return Ok(()); }
    let state = app.state::<AppState>();
    let s = state.session.lock().unwrap().clone();
    let b = *state.bounds.lock().unwrap();
    for (id, view) in app.webviews() {
        if !id.starts_with("tab-") {
            continue;
        }
        if super::media::is_player(app, &id) { continue; }
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
        s.clear_split(chrono::Utc::now().timestamp());
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
    let moved = {
        let state = app.state::<AppState>();
        let mut s = state.session.lock().unwrap();
        update_group(&mut s, id, group, workspace)?
    };
    // Saving a group name on blur can arrive after another tab was selected.
    // Only an explicit workspace move changes the user's current selection.
    if moved { super::activate(app, id)?; } else { super::emit(app); }
    super::persist(app)
}

fn update_group(
    s: &mut super::Session,
    id: &str,
    group: &str,
    workspace: &str,
) -> Result<bool, String> {
    if group.chars().count() > 40 {
        return Err("Gruppenavnet er for langt".into());
    }
    if !s.workspaces.iter().any(|w| w == workspace) {
        return Err("Ukjent arbeidsområde".into());
    }
    let tab = s
        .tabs
        .iter_mut()
        .find(|t| t.id == id)
        .ok_or("Ukjent fane")?;
    let moved = tab.workspace != workspace;
    tab.group = group.trim().into();
    tab.workspace = workspace.into();
    if moved
        && s.split
            .as_ref()
            .is_some_and(|pair| pair.iter().any(|member| member == id))
    {
        s.clear_split(chrono::Utc::now().timestamp());
    }
    Ok(moved)
}

fn reorder_tabs(tabs: &mut Vec<Tab>, id: &str, before: Option<&str>) -> Result<(), String> {
    let from = tabs.iter().position(|t| t.id == id).ok_or("Ukjent fane")?;
    let source = &tabs[from];
    if let Some(before) = before {
        let target = tabs.iter().find(|t| t.id == before).ok_or("Ukjent fane")?;
        if source.workspace != target.workspace || source.pinned != target.pinned {
            return Err("Flytt innenfor samme arbeidsområde og festede/vanlige faner".into());
        }
        if id == before {
            return Ok(());
        }
    }
    let workspace = source.workspace.clone();
    let pinned = source.pinned;
    let tab = tabs.remove(from);
    // Resolve the insertion point after removal: earlier -> later must still
    // insert before the target. A null target appends to this partition only.
    let to = match before {
        Some(target) => tabs.iter().position(|t| t.id == target).unwrap(),
        None => tabs
            .iter()
            .rposition(|t| t.workspace == workspace && t.pinned == pinned)
            .map_or(from.min(tabs.len()), |index| index + 1),
    };
    tabs.insert(to, tab);
    Ok(())
}

pub fn reorder(app: &AppHandle, id: &str, before: Option<&str>) -> Result<(), String> {
    {
        let state = app.state::<AppState>();
        let mut s = state.session.lock().unwrap();
        reorder_tabs(&mut s.tabs, id, before)?;
    }
    super::emit(app);
    super::persist(app)
}

pub fn split(app: &AppHandle, other: Option<String>) -> Result<(), String> {
    let state = app.state::<AppState>();
    if let Some(id) = other {
        let active = state.session.lock().unwrap().active.clone();
        if super::media::is_player(app, &id) || super::media::is_player(app, &active) {
            return Err("Flytt minispilleren tilbake før du bruker fanen i delt visning.".into());
        }
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
        state
            .session
            .lock()
            .unwrap()
            .replace_split([active, id], chrono::Utc::now().timestamp());
    } else {
        state
            .session
            .lock()
            .unwrap()
            .clear_split(chrono::Utc::now().timestamp());
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
    super::media::release(app, id)?;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn ordered_tab(id: &str, workspace: &str, pinned: bool) -> Tab {
        let mut tab = Tab::new(format!("https://example.com/{id}"));
        tab.id = id.into();
        tab.workspace = workspace.into();
        tab.pinned = pinned;
        tab
    }

    fn ids(tabs: &[Tab]) -> Vec<&str> {
        tabs.iter().map(|tab| tab.id.as_str()).collect()
    }

    #[test]
    fn reorder_inserts_before_target_in_both_directions() {
        let mut tabs = vec![ordered_tab("a", "Home", false), ordered_tab("b", "Home", false), ordered_tab("c", "Home", false)];
        reorder_tabs(&mut tabs, "a", Some("c")).unwrap();
        assert_eq!(ids(&tabs), ["b", "a", "c"]);
        reorder_tabs(&mut tabs, "c", Some("b")).unwrap();
        assert_eq!(ids(&tabs), ["c", "b", "a"]);
        reorder_tabs(&mut tabs, "b", Some("b")).unwrap();
        assert_eq!(ids(&tabs), ["c", "b", "a"]);
    }

    #[test]
    fn reorder_appends_within_partition_and_preserves_unrelated_order() {
        let mut tabs = vec![ordered_tab("a", "Home", false), ordered_tab("p", "Home", true), ordered_tab("b", "Home", false), ordered_tab("w", "Work", false), ordered_tab("c", "Home", false), ordered_tab("x", "Work", false)];
        reorder_tabs(&mut tabs, "a", None).unwrap();
        assert_eq!(ids(&tabs), ["p", "b", "w", "c", "a", "x"]);
        reorder_tabs(&mut tabs, "p", None).unwrap();
        assert_eq!(ids(&tabs), ["p", "b", "w", "c", "a", "x"]);
        reorder_tabs(&mut tabs, "w", None).unwrap();
        assert_eq!(ids(&tabs), ["p", "b", "c", "a", "x", "w"]);
    }

    #[test]
    fn invalid_reorder_does_not_mutate_tabs() {
        let mut tabs = vec![ordered_tab("a", "Home", false), ordered_tab("p", "Home", true), ordered_tab("w", "Work", false)];
        for (id, target) in [("missing", None), ("a", Some("missing")), ("a", Some("p")), ("a", Some("w"))] {
            assert!(reorder_tabs(&mut tabs, id, target).is_err());
            assert_eq!(ids(&tabs), ["a", "p", "w"]);
        }
    }

    #[test]
    fn moving_a_split_tab_to_another_workspace_ends_the_split() {
        let left = Tab::new("https://example.com/left".into());
        let right = Tab::new("https://example.com/right".into());
        let id = left.id.clone();
        let pair = [id.clone(), right.id.clone()];
        let mut session = super::super::Session {
            tabs: vec![left, right],
            active: id.clone(),
            workspaces: vec![default_workspace(), "Jobb".into()],
            split: Some(pair.clone()),
            ..Default::default()
        };
        assert!(!update_group(&mut session, &id, "Lesing", &default_workspace()).unwrap());
        assert_eq!(session.split, Some(pair));
        assert_eq!(session.active, id);
        assert!(update_group(&mut session, &id, "Prosjekt", "Jobb").unwrap());
        assert!(session.split.is_none());
        assert_eq!(session.tabs[0].workspace, "Jobb");
        assert_eq!(session.tabs[0].group, "Prosjekt");
        assert_eq!(session.tabs[1].workspace, default_workspace());
    }
}
