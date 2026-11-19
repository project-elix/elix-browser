//! Profile-wide rules are keyed by workspace. A tab's explicit override travels
//! with the tab through save, reopen, and transfer to another window.
use super::{Session,Tab};
use crate::{storage::Store,AppState};
use serde::{Deserialize,Serialize};
use std::collections::BTreeMap;
use tauri::{AppHandle,Manager};

#[derive(Default,Clone,Serialize,Deserialize)]
pub struct WorkspaceRules {
    pub domains:BTreeMap<String,String>,
    pub colors:BTreeMap<String,String>,
}
pub type GroupRules=BTreeMap<String,WorkspaceRules>;

pub fn load(store:&Store,profile:&str)->GroupRules {
    store.get(&format!("tab_group_rules:{profile}")).and_then(|s|serde_json::from_str(&s).ok()).unwrap_or_default()
}
// Other windows of this profile can edit a workspace's rules.
pub fn refresh(app:&AppHandle) {
    let state=app.state::<AppState>();
    let profile=state.profile.lock().unwrap().clone();
    let rules=load(&state.store.lock().unwrap(),&profile);
    state.session.lock().unwrap().group_rules=rules;
}
fn domain_name(raw:&str)->Result<String,String>{
    let value=raw.trim().trim_end_matches('.');
    let domain=match url::Host::parse(value) {
        Ok(url::Host::Domain(domain))=>domain,
        _=>return Err("Skriv et domenenavn, for eksempel example.com.".into()),
    };
    if domain.len()>253 || domain.split('.').any(|p|p.is_empty()||p.len()>63||p.starts_with('-')||p.ends_with('-')||
        !p.bytes().all(|b|b.is_ascii_alphanumeric()||b==b'-')) {
        return Err("Ugyldig domenenavn.".into());
    }
    Ok(domain)
}
impl WorkspaceRules{
    fn target(&self,url:&str)->Option<&String>{
        let url=url::Url::parse(url).ok()?;
        if !matches!(url.scheme(),"http"|"https"){return None;}
        let mut host=url.domain()?.trim_end_matches('.');
        loop {
            if let Some(name)=self.domains.get(host){return Some(name);}
            host=host.split_once('.')?.1;
        }
    }
    fn save(&mut self,domain:&str,group:&str,color:&str)->Result<(),String>{
        let group=group.trim();
        if group.is_empty()||group.chars().count()>40||group.chars().any(char::is_control){return Err("Gruppenavnet må være 1–40 tegn.".into());}
        if color.len()!=7||!color.starts_with('#')||!color[1..].bytes().all(|b|b.is_ascii_hexdigit()){return Err("Ugyldig gruppefarge.".into());}
        let domain=if domain.trim().is_empty(){None}else{Some(domain_name(domain)?)};
        if self.colors.len()>=100&&!self.colors.contains_key(group){return Err("Maksimalt 100 navngitte grupper per arbeidsområde.".into());}
        if let Some(host)=domain {
            if self.domains.len()>=100&&!self.domains.contains_key(&host){return Err("Maksimalt 100 regler per arbeidsområde.".into());}
            self.domains.insert(host,group.into());
        }
        self.colors.insert(group.into(),color.to_ascii_lowercase());
        Ok(())
    }
}

pub fn manual(tab:&Tab)->bool{
    // All memberships created before this field existed were manual.
    tab.group_manual.unwrap_or(!tab.group.is_empty())
}
pub fn apply(session:&mut Session,id:&str){
    let Some(tab)=session.tabs.iter().find(|t|t.id==id) else{return};
    if manual(tab){return;}
    let workspace=tab.workspace.clone();
    let group=session.group_rules.get(&workspace).and_then(|r|r.target(&tab.url)).cloned().unwrap_or_default();
    if tab.group==group{return;}
    // The workspace is unchanged, so this only clusters members in their own partition.
    let _=super::organization::update_group(session,id,&group,&workspace);
    if let Some(tab)=session.tabs.iter_mut().find(|t|t.id==id){tab.group_manual=Some(false);}
}
pub fn apply_all(session:&mut Session){
    let workspaces=session.workspaces.clone();
    for workspace in workspaces{apply_workspace(session,&workspace);}
}
fn apply_workspace(session:&mut Session,workspace:&str){
    let ids:Vec<_>=session.tabs.iter().filter(|t|t.workspace==workspace).map(|t|t.id.clone()).collect();
    for id in ids{apply(session,&id);}
}
pub fn configure(app:&AppHandle,profile:&str,workspace:&str,operation:&str,domain:&str,group:&str,color:&str,tab_id:&str)->Result<(),String>{
    let state=app.state::<AppState>();
    let _transition=state.session_transition.lock().unwrap();
    if *state.profile.lock().unwrap()!=profile{return Err("Profilen ble endret.".into());}
    if !state.session.lock().unwrap().workspaces.iter().any(|w|w==workspace){return Err("Ukjent arbeidsområde.".into());}
    if operation=="save"||operation=="remove"{
        let mut store=state.store.lock().unwrap();
        let transaction=store.conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate).map_err(|e|e.to_string())?;
        let key=format!("tab_group_rules:{profile}");
        use rusqlite::OptionalExtension;
        let old:Option<String>=transaction.query_row("SELECT value FROM settings WHERE key=?1",[&key],|r|r.get(0)).optional().map_err(|e|e.to_string())?;
        let mut rules:GroupRules=old.map(|raw|serde_json::from_str(&raw)).transpose().map_err(|_|"Ugyldige lagrede grupperegler.")?.unwrap_or_default();
        let scope=rules.entry(workspace.into()).or_default();
        if operation=="save"{scope.save(domain,group,color)?;}else{scope.domains.remove(&domain_name(domain)?);}
        let encoded=serde_json::to_string(&rules).map_err(|e|e.to_string())?;
        transaction.execute("INSERT INTO settings(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",rusqlite::params![key,encoded]).map_err(|e|e.to_string())?;
        transaction.commit().map_err(|e|e.to_string())?;
    }else if operation!="apply"&&operation!="resume"{return Err("Ukjent grupperegelhandling.".into());}
    refresh(app);
    {
        let mut session=state.session.lock().unwrap();
        if operation=="resume"{
            let tab=session.tabs.iter_mut().find(|t|t.id==tab_id&&t.workspace==workspace).ok_or("Ukjent fane.")?;
            tab.group_manual=Some(false);
            apply(&mut session,tab_id);
        }else{apply_workspace(&mut session,workspace);}
    }
    super::emit(app);
    super::persist(app)
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn rules_follow_navigation_and_keep_workspace_colors_and_manual_moves_on_restore(){
        let mut session=Session::default();
        session.workspaces=vec!["Jobb".into(),"Personlig".into()];
        let rules=session.group_rules.entry("Jobb".into()).or_default();
        rules.save("example.org","Lesing","#aabbcc").unwrap();
        rules.save("other.test","Prosjekt","#334455").unwrap();
        for url in ["https://example.org/a","https://sub.example.org/b","https://other.test/"] {
            let mut tab=Tab::new(url.into());tab.workspace="Jobb".into();session.tabs.push(tab);
        }
        let ids:Vec<_>=session.tabs.iter().map(|t|t.id.clone()).collect();
        apply_workspace(&mut session,"Jobb");
        assert_eq!(session.tabs.iter().filter(|t|t.group=="Lesing").count(),2);
        let manual_tab=&ids[0];
        super::super::organization::update_group(&mut session,manual_tab,"Privat leseliste","Jobb").unwrap();
        let moving=&ids[1];
        session.tabs.iter_mut().find(|t|t.id==*moving).unwrap().url="https://other.test/new".into();
        apply(&mut session,moving);
        apply_workspace(&mut session,"Jobb");
        assert_eq!(session.tabs.iter().filter(|t|t.group=="Prosjekt").count(),2);
        session.tabs.push(Tab::new("https://other.test/home".into()));
        apply_workspace(&mut session,"Personlig");
        let raw=serde_json::to_string(&session.persistent()).unwrap();
        let mut restored:Session=serde_json::from_str(&raw).unwrap();
        restored.prepare_restore(1000);apply_workspace(&mut restored,"Jobb");
        assert!(restored.tabs.iter().any(|t|t.group=="Privat leseliste"&&manual(t)));
        assert!(restored.tabs.iter().any(|t|t.workspace=="Personlig"&&t.group.is_empty()));
        assert_eq!(restored.group_rules["Jobb"].colors["Prosjekt"],"#334455");
    }
}
