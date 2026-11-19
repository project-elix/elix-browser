use super::{Session, Tab};
use crate::{security, AppState};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use tauri::{AppHandle, Manager};

#[derive(Serialize, Deserialize)]
struct StoredSession {
    #[serde(default)]
    layout_version: u32,
    #[serde(flatten)]
    session: Session,
}

pub fn split_indices(session: &Session) -> Option<[usize; 2]> {
    let pair = session.split.as_ref()?;
    if pair[0] == pair[1] || !pair.contains(&session.active) { return None; }
    let left = session.tabs.iter().position(|tab| tab.id == pair[0])?;
    let right = session.tabs.iter().position(|tab| tab.id == pair[1])?;
    let a = &session.tabs[left];
    let b = &session.tabs[right];
    (a.workspace == b.workspace && !a.private && !b.private
        && !a.url.starts_with("elix:") && !b.url.starts_with("elix:")).then_some([left,right])
}

pub fn encode(session: &Session, named: bool) -> Result<String, String> {
    let mut saved = session.persistent();
    if named {
        if saved.tabs.is_empty() { return Err("Ingen vanlige faner å lagre".into()); }
        saved.closed.clear();
    }
    serde_json::to_string(&StoredSession { layout_version: 1, session: saved }).map_err(|e| e.to_string())
}

pub fn decode(data: &str) -> Option<Session> {
    let stored: StoredSession = serde_json::from_str(data).ok()?;
    let mut session = stored.session;
    if stored.layout_version > 1 { return None; }
    let mut ids = HashSet::new();
    if stored.layout_version == 0 {
        // Older releases tolerated stale IDs and ignored persisted split references.
        // Keep their eligible pages when upgrading to the versioned format.
        if !session.tabs.is_empty() && !session.tabs.iter().any(|t|
            !t.private && security::stored_destination_allowed(&t.url)) { return None; }
        session.split = None;
    } else if session.tabs.len() > 100 || session.workspaces.len() > 20
        || session.tabs.iter().any(|t| t.id.is_empty() || !ids.insert(&t.id) || t.private
            || !security::stored_destination_allowed(&t.url))
        || (!session.tabs.is_empty() && !ids.contains(&session.active))
        || (session.split.is_some() && split_indices(&session).is_none()) {
        return None;
    }
    session.prepare_restore(chrono::Utc::now().timestamp());
    Some(session)
}

pub fn load_named(store: &crate::storage::Store, profile: &str, name: &str) -> Result<Session,String> {
    store.named_session_candidates(profile, name)?.iter().find_map(|data| decode(data))
        .ok_or_else(|| "Fant ingen gyldig versjon av økten".into())
}

pub fn save(app: &AppHandle, profile: &str, name: &str) -> Result<(),String> {
    let state = app.state::<AppState>();
    let _transition = state.session_transition.lock().unwrap();
    if *state.profile.lock().unwrap() != profile { return Err("Profilen ble endret".into()); }
    let data = encode(&state.session.lock().unwrap(), true)?;
    let result = state.store.lock().unwrap().save_named_session(profile, name, &data);
    result
}

pub async fn open(app: AppHandle, profile: String, name: String) -> Result<(),String> {
    {
        let state = app.state::<AppState>();
        let _transition = state.session_transition.lock().unwrap();
        if *state.profile.lock().unwrap() != profile { return Err("Profilen ble endret".into()); }
        load_named(&state.store.lock().unwrap(), &profile, &name)?;
    }
    let handoff = super::detach::Handoff {
        profile, saved_session: Some(name), tab: Tab::new("elix://start".into()), x: 100, y: 100,
    };
    super::detach::launch_window(&app, handoff, false).await
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn old_sessions_restore_ordinary_tabs_even_with_a_stale_active_id() {
        let public = Tab::new("https://example.com/".into());
        let mut private = Tab::new("https://private.test/".into()); private.private = true;
        let legacy = Session { active: "stale".into(), tabs: vec![public,private], ..Default::default() };
        let restored = decode(&serde_json::to_string(&legacy).unwrap()).unwrap();
        assert_eq!(restored.tabs.len(), 1);
        assert_eq!(restored.tabs[0].url, "https://example.com/");
        assert_eq!(restored.active, restored.tabs[0].id);
    }

    #[test]
    fn two_workspaces_split_order_private_content_and_recovery() {
        let dir=tempfile::tempdir().unwrap();
        let db=dir.path().join("sessions.sqlite");
        let store=crate::storage::Store::open(&db).unwrap();
        let profile=store.profiles().unwrap()[0].id.clone();
        let mut first = Tab::new("https://example.com/first".into());
        first.group="Reading".into();
        let private_workspace="Private-only workspace";
        let mut hidden=Tab::new("https://private.test/secret".into());
        hidden.private=true; hidden.workspace=private_workspace.into(); hidden.group="Private-only group".into();
        let mut second=Tab::new("https://example.com/second".into());
        second.workspace="Jobb".into(); second.group="Meeting".into();
        let mut third=Tab::new("https://example.com/third".into());
        third.workspace="Jobb".into();
        let original=Session {
            active:third.id.clone(), split:Some([third.id.clone(),second.id.clone()]),
            workspace:"Jobb".into(), workspaces:vec!["Personlig".into(),private_workspace.into(),"Jobb".into()],
            tabs:vec![first,hidden.clone(),second,third],closed:vec![hidden],..Default::default()
        };
        let bytes=encode(&original,true).unwrap();
        for secret in ["private.test","Private-only","secret"] { assert!(!bytes.contains(secret)); }
        store.save_named_session(&profile,"Arbeidsdag",&bytes).unwrap();
        store.save_named_session(&profile,"Arbeidsdag","broken").unwrap();
        store.save_session(&profile,&bytes).unwrap();
        store.save_session(&profile,r#"{"layout_version":42,"tabs":[],"active":""}"#).unwrap();
        drop(store);
        let reopened=crate::storage::Store::open(&db).unwrap();
        let named=load_named(&reopened,&profile,"Arbeidsdag").unwrap();
        let recovered=reopened.session_candidates(&profile).iter().find_map(|raw| decode(raw)).unwrap();
        for restored in [named,recovered] {
            assert_eq!(restored.tabs.iter().map(|t| t.url.as_str()).collect::<Vec<_>>(),
                    ["https://example.com/first","https://example.com/second","https://example.com/third"]);
            assert_eq!(restored.workspaces,["Personlig","Jobb"]);
            assert_eq!(restored.tabs[0].group,"Reading"); assert_eq!(restored.tabs[1].group,"Meeting");
            assert_eq!(restored.active,restored.tabs[2].id); assert_eq!(restored.workspace,"Jobb");
            assert_eq!(restored.split,Some([restored.tabs[2].id.clone(),restored.tabs[1].id.clone()]));
        }
    }
}
