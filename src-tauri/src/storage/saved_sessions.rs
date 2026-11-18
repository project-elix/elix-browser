use super::Store;
use rusqlite::params;
use serde::Serialize;

#[derive(Serialize)]
pub struct SavedSession {
    pub name: String,
    pub revision: i64,
    pub updated: String,
}

impl Store {
    pub fn save_window_session(&self, window: &str, profile: &str, data: &str) -> Result<(),String> {
        let current = format!("window_session:{window}:{profile}");
        let backup = format!("window_session_backup:{window}:{profile}");
        let tx = self.conn.unchecked_transaction().map_err(|e| e.to_string())?;
        tx.execute("INSERT INTO settings(key,value) SELECT ?1,value FROM settings WHERE key=?2 AND value<>?3
            ON CONFLICT(key) DO UPDATE SET value=excluded.value", params![backup,current,data]).map_err(|e| e.to_string())?;
        self.set(&current, data)?;
        tx.commit().map_err(|e| e.to_string())
    }

    pub fn window_session_candidates(&self, window: &str, profile: &str) -> Vec<String> {
        [format!("window_session:{window}:{profile}"),format!("window_session_backup:{window}:{profile}")]
            .iter().filter_map(|key| self.get(key)).collect()
    }

    pub fn init_named_sessions(&self) -> Result<(), String> {
        self.conn.execute_batch("CREATE TABLE IF NOT EXISTS named_session_versions (
            profile TEXT NOT NULL REFERENCES profiles(id), name TEXT NOT NULL, revision INTEGER NOT NULL,
            updated TEXT NOT NULL, data TEXT NOT NULL, PRIMARY KEY(profile,name,revision))")
            .map_err(|e| e.to_string())
    }

    pub fn save_named_session(&self, profile: &str, name: &str, data: &str) -> Result<(), String> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 80 {
            return Err("Øktnavnet må ha 1–80 tegn".into());
        }
        let tx = self.conn.unchecked_transaction().map_err(|e| e.to_string())?;
        tx.execute("INSERT INTO named_session_versions(profile,name,revision,updated,data)
            SELECT ?1,?2,COALESCE(MAX(revision),0)+1,?3,?4 FROM named_session_versions WHERE profile=?1 AND name=?2",
            params![profile,name,chrono::Utc::now().to_rfc3339(),data]).map_err(|e| e.to_string())?;
        tx.execute("DELETE FROM named_session_versions WHERE profile=?1 AND name=?2 AND revision <
            (SELECT MAX(revision)-2 FROM named_session_versions WHERE profile=?1 AND name=?2)",
            params![profile,name]).map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())
    }

    pub fn list_named_sessions(&self, profile: &str) -> Result<Vec<SavedSession>, String> {
        let mut query = self.conn.prepare("SELECT name,revision,updated FROM named_session_versions n
            WHERE profile=?1 AND revision=(SELECT MAX(revision) FROM named_session_versions
            WHERE profile=n.profile AND name=n.name) ORDER BY updated DESC,name").map_err(|e| e.to_string())?;
        let rows = query.query_map([profile], |r| Ok(SavedSession {
            name:r.get(0)?, revision:r.get(1)?, updated:r.get(2)?,
        })).map_err(|e| e.to_string())?;
        rows.collect::<Result<_,_>>().map_err(|e| e.to_string())
    }

    pub fn named_session_candidates(&self, profile: &str, name: &str) -> Result<Vec<String>,String> {
        let mut query = self.conn.prepare("SELECT data FROM named_session_versions
            WHERE profile=?1 AND name=?2 ORDER BY revision DESC").map_err(|e| e.to_string())?;
        let rows=query.query_map(params![profile,name], |r| r.get(0)).map_err(|e| e.to_string())?;
        rows.collect::<Result<_,_>>().map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::super::Store;

    #[test]
    fn named_revisions_and_crash_backup_survive_reopening_and_isolate_profiles() {
        let dir=tempfile::tempdir().unwrap();
        let path=dir.path().join("sessions.sqlite");
        let store=Store::open(&path).unwrap();
        let main=store.profiles().unwrap()[0].id.clone();
        let other=store.create_profile("Jobb").unwrap().id;
        for n in 1..=6 { store.save_named_session(&main,"Dagens økt",&n.to_string()).unwrap(); }
        store.save_named_session(&other,"Dagens økt","private profile").unwrap();
        store.save_window_session("window",&main,"original").unwrap();
        store.save_window_session("window",&main,"new").unwrap();
        store.save_window_session("window",&main,"new").unwrap();
        assert!(store.save_named_session(&main," ","missing name").is_err());
        drop(store);
        let store=Store::open(&path).unwrap();
        assert_eq!(store.list_named_sessions(&main).unwrap()[0].revision,6);
        assert_eq!(store.named_session_candidates(&main,"Dagens økt").unwrap(),["6","5","4"]);
        assert_eq!(store.named_session_candidates(&other,"Dagens økt").unwrap(),["private profile"]);
        assert_eq!(store.window_session_candidates("window",&main),["new","original"]);
        assert!(store.window_session_candidates("window",&other).is_empty());
    }
}
