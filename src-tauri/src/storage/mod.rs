use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const SITE_PERMISSIONS: &[&str] = &[
    "camera", "microphone", "location", "notifications", "multiple_downloads",
];

#[derive(Clone, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Entry {
    pub id: i64,
    pub url: String,
    pub title: String,
    pub time: String,
    #[serde(default)]
    pub favicon: Option<String>,
    #[serde(default)]
    pub group: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub developer: bool,
    pub gaming: bool,
    pub channel: String,
    pub suspend_minutes: u32,
    pub appearance: Appearance,
    pub password_autosave: bool,
    pub autofill: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            developer: false,
            gaming: false,
            channel: "stable".into(),
            suspend_minutes: 0,
            appearance: Appearance::default(),
            password_autosave: false,
            autofill: false,
        }
    }
}
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
#[serde(default)]
pub struct Appearance {
    pub accent: String,
    pub background: String,
    pub density: String,
    pub radius: u8,
    pub reduce_motion: bool,
    pub gradient: bool,
    pub sidebar: String,
    pub bookmark_bar: String,
}
impl Default for Appearance {
    fn default() -> Self {
        Self {
            accent: "#43d3e8".into(),
            background: "#080b12".into(),
            density: "comfortable".into(),
            radius: 12,
            reduce_motion: false,
            gradient: true,
            sidebar: "icons".into(),
            bookmark_bar: "toolbar".into(),
        }
    }
}
impl Settings {
    pub fn valid(&self) -> bool {
        let color = |s: &str| {
            s.len() == 7
                && s.starts_with('#')
                && s.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)
        };
        ["stable", "beta", "developer"].contains(&self.channel.as_str())
            && self.suspend_minutes <= 1440
            && color(&self.appearance.accent)
            && color(&self.appearance.background)
            && ["comfortable", "compact"].contains(&self.appearance.density.as_str())
            && self.appearance.radius <= 20
            && ["icons", "expanded", "hidden"].contains(&self.appearance.sidebar.as_str())
            && ["hidden", "sidebar", "toolbar"].contains(&self.appearance.bookmark_bar.as_str())
    }
}
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum TabSide { Left, Right }

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Debug)]
pub struct TabLayout {
    pub vertical: bool,
    pub side: TabSide,
    pub width: u16,
}

impl Default for TabLayout {
    fn default() -> Self {
        Self { vertical: false, side: TabSide::Left, width: 240 }
    }
}

mod saved_sessions;

pub struct Store {
    pub conn: Connection,
}
impl Store {
    pub fn open(path: &Path) -> Result<Self, String> {
        let mut conn = Connection::open(path).map_err(|e| e.to_string())?;
        conn.busy_timeout(std::time::Duration::from_secs(5))
            .map_err(|e| e.to_string())?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
            .map_err(|e| e.to_string())?;
        let version: i64 = conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .map_err(|e| e.to_string())?;
        if version > 2 {
            return Err("Databasen er fra en nyere ELIX-versjon. Nedgradering er blokkert.".into());
        }
        if version < 1 {
            if path.exists()
                && conn
                    .query_row(
                        "SELECT count(*) FROM sqlite_master WHERE type='table'",
                        [],
                        |r| r.get::<_, i64>(0),
                    )
                    .unwrap_or(0)
                    > 0
            {
                conn.backup(
                    "main",
                    path.with_extension(format!("v{version}.backup.sqlite")),
                    None,
                )
                .map_err(|e| e.to_string())?;
            }
            let tx = conn
                .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                .map_err(|e| e.to_string())?;
            if tx
                .pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
                .map_err(|e| e.to_string())?
                < 1
            {
                tx.execute_batch("CREATE TABLE IF NOT EXISTS profiles(id TEXT PRIMARY KEY, name TEXT NOT NULL); CREATE TABLE IF NOT EXISTS settings(key TEXT PRIMARY KEY, value TEXT NOT NULL); CREATE TABLE IF NOT EXISTS bookmarks(id INTEGER PRIMARY KEY, profile TEXT NOT NULL REFERENCES profiles(id), url TEXT NOT NULL, title TEXT NOT NULL, time TEXT NOT NULL, UNIQUE(profile,url)); CREATE TABLE IF NOT EXISTS history(id INTEGER PRIMARY KEY, profile TEXT NOT NULL REFERENCES profiles(id), url TEXT NOT NULL, title TEXT NOT NULL, time TEXT NOT NULL); CREATE INDEX IF NOT EXISTS history_profile_time ON history(profile,time); CREATE TABLE IF NOT EXISTS sessions(profile TEXT PRIMARY KEY REFERENCES profiles(id), data TEXT NOT NULL); PRAGMA user_version=1;").map_err(|e|e.to_string())?;
            }
            tx.commit().map_err(|e| e.to_string())?;
        }
        if version < 2 {
            if version == 1 && !path.with_extension("v1.backup.sqlite").exists() {
                conn.backup("main", path.with_extension("v1.backup.sqlite"), None)
                    .map_err(|e| e.to_string())?;
            }
            let tx = conn
                .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                .map_err(|e| e.to_string())?;
            if tx
                .pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
                .map_err(|e| e.to_string())?
                < 2
            {
                tx.execute_batch("ALTER TABLE bookmarks ADD COLUMN favicon TEXT;
                CREATE TABLE site_permissions(profile TEXT NOT NULL REFERENCES profiles(id), origin TEXT NOT NULL, kind TEXT NOT NULL, decision TEXT NOT NULL, PRIMARY KEY(profile,origin,kind));
                CREATE TABLE session_backups(id INTEGER PRIMARY KEY, profile TEXT NOT NULL REFERENCES profiles(id), data TEXT NOT NULL);
                PRAGMA user_version=2;").map_err(|e| e.to_string())?;
            }
            tx.commit().map_err(|e| e.to_string())?;
        }
        // Additive tables deliberately keep schema 2 readable by Alpha 6/7.
        // Older clients ignore groups; their bookmark deletions still cascade.
        let groups_exist: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='bookmark_groups')",
            [], |r| r.get(0),
        ).map_err(|e| e.to_string())?;
        if !groups_exist && version == 2 && !path.with_extension("v2.bookmark-groups.backup.sqlite").exists() {
            conn.backup("main", path.with_extension("v2.bookmark-groups.backup.sqlite"), None)
                .map_err(|e| e.to_string())?;
        }
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(|e| e.to_string())?;
        tx.execute_batch("CREATE TABLE IF NOT EXISTS bookmark_groups(
            profile TEXT NOT NULL REFERENCES profiles(id), name TEXT NOT NULL,
            PRIMARY KEY(profile,name));
            CREATE UNIQUE INDEX IF NOT EXISTS bookmark_id_profile ON bookmarks(id,profile);
            CREATE TABLE IF NOT EXISTS bookmark_group_entries(
                bookmark INTEGER PRIMARY KEY, profile TEXT NOT NULL, name TEXT NOT NULL,
                FOREIGN KEY(bookmark,profile) REFERENCES bookmarks(id,profile) ON DELETE CASCADE,
                FOREIGN KEY(profile,name) REFERENCES bookmark_groups(profile,name) ON UPDATE CASCADE ON DELETE CASCADE);")
            .map_err(|e| e.to_string())?;
        tx.execute_batch("CREATE TABLE IF NOT EXISTS workspace_profiles(
            preset TEXT PRIMARY KEY, profile TEXT NOT NULL UNIQUE REFERENCES profiles(id) ON DELETE CASCADE);")
            .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        let store = Self { conn };
        store.init_named_sessions()?;
        if store.profiles()?.is_empty() {
            let p = store.create_profile("Personlig")?;
            store.set("active_profile", &p.id)?;
        }
        Ok(store)
    }
    pub fn get(&self, key: &str) -> Option<String> {
        self.conn
            .query_row("SELECT value FROM settings WHERE key=?1", [key], |r| {
                r.get(0)
            })
            .ok()
    }
    pub fn set(&self, key: &str, value: &str) -> Result<(), String> {
        self.conn.execute("INSERT INTO settings(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",params![key,value]).map(|_|()).map_err(|e|e.to_string())
    }
    pub fn profiles(&self) -> Result<Vec<Profile>, String> {
        let mut q = self
            .conn
            .prepare("SELECT p.id,p.name,w.preset FROM profiles p
                LEFT JOIN workspace_profiles w ON w.profile=p.id ORDER BY p.rowid")
            .map_err(|e| e.to_string())?;
        let items = q
            .query_map([], |r| {
                Ok(Profile {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    workspace: r.get(2)?,
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        Ok(items)
    }
    pub fn create_profile(&self, name: &str) -> Result<Profile, String> {
        Self::insert_profile(&self.conn, name)
    }

    fn insert_profile(conn: &Connection, name: &str) -> Result<Profile, String> {
        let name = name.trim();
        if name.is_empty() || name.len() > 60 {
            return Err("Profilnavnet må være mellom 1 og 60 tegn".into());
        }
        let p = Profile {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.into(),
            workspace: None,
        };
        conn
            .execute("INSERT INTO profiles VALUES(?1,?2)", params![p.id, p.name])
            .map_err(|e| e.to_string())?;
        Ok(p)
    }

    pub fn workspace_profile(&mut self, preset: &str) -> Result<Profile, String> {
        let name = match preset {
            "work" => "Jobb",
            "personal" => "Privat",
            "gaming" => "Gaming",
            _ => return Err("Ukjent arbeidsområdeprofil".into()),
        };
        let tx = self.conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(|e| e.to_string())?;
        let existing = tx.query_row("SELECT p.id,p.name FROM workspace_profiles w
            JOIN profiles p ON p.id=w.profile WHERE w.preset=?1", [preset], |row| {
            Ok(Profile { id: row.get(0)?, name: row.get(1)?, workspace: Some(preset.into()) })
        }).optional().map_err(|e| e.to_string())?;
        if let Some(profile) = existing {
            return Ok(profile);
        }
        let mut profile = Self::insert_profile(&tx, name)?;
        tx.execute("INSERT INTO workspace_profiles(preset,profile) VALUES(?1,?2)", params![preset,profile.id])
            .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        profile.workspace = Some(preset.into());
        Ok(profile)
    }
    pub fn tab_layout(&self, profile: &str) -> TabLayout {
        self.get(&format!("tab_layout:{profile}"))
            .and_then(|json| serde_json::from_str::<TabLayout>(&json).ok())
            .filter(|layout| (160..=420).contains(&layout.width))
            .unwrap_or_default()
    }

    pub fn save_tab_layout(&self, profile: &str, layout: TabLayout) -> Result<(), String> {
        if !(160..=420).contains(&layout.width) {
            return Err("Fanebredden må være mellom 160 og 420 piksler.".into());
        }
        self.set(&format!("tab_layout:{profile}"), &serde_json::to_string(&layout).map_err(|e| e.to_string())?)
    }

    pub fn settings(&self) -> Settings {
        self.get("preferences")
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }
    pub fn entries(&self, profile: &str, kind: &str) -> Result<Vec<Entry>, String> {
        let sql=match kind{"bookmarks"=>"SELECT b.id,b.url,b.title,b.time,b.favicon,COALESCE(g.name,'') FROM bookmarks b LEFT JOIN bookmark_group_entries g ON g.bookmark=b.id AND g.profile=b.profile WHERE b.profile=?1 ORDER BY b.id DESC LIMIT 5001", "history"=>"SELECT id,url,title,time,NULL,'' FROM history WHERE profile=?1 ORDER BY id DESC LIMIT 1000",_=>return Err("Ukjent liste".into())};
        let mut q = self.conn.prepare(sql).map_err(|e| e.to_string())?;
        let rows = q
            .query_map([profile], |r| {
                Ok(Entry {
                    id: r.get(0)?,
                    url: r.get(1)?,
                    title: r.get(2)?,
                    time: r.get(3)?,
                    favicon: r.get(4)?,
                    group: r.get(5)?,
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        Ok(rows)
    }
    pub fn visit(&self, p: &str, url: &str, title: &str) -> Result<(), String> {
        self.conn
            .execute(
                "INSERT INTO history(profile,url,title,time) VALUES(?1,?2,?3,?4)",
                params![p, url, title, chrono::Utc::now().to_rfc3339()],
            )
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
    pub fn bookmark(&self, p: &str, url: &str, title: &str) -> Result<(), String> {
        self.conn.execute("INSERT INTO bookmarks(profile,url,title,time) VALUES(?1,?2,?3,?4) ON CONFLICT(profile,url) DO UPDATE SET title=excluded.title",params![p,url,title,chrono::Utc::now().to_rfc3339()]).map(|_|()).map_err(|e|e.to_string())
    }
    pub fn remove_bookmark(&self, p: &str, id: i64) -> Result<(), String> {
        self.conn
            .execute(
                "DELETE FROM bookmarks WHERE id=?1 AND profile=?2",
                params![id, p],
            )
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
    pub fn clear_history(&self, p: &str) -> Result<(), String> {
        self.conn
            .execute("DELETE FROM history WHERE profile=?1", [p])
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
    pub fn bookmark_groups(&self, p: &str) -> Result<Vec<String>, String> {
        let mut q = self.conn.prepare("SELECT name FROM bookmark_groups WHERE profile=?1 ORDER BY name COLLATE NOCASE,name")
            .map_err(|e| e.to_string())?;
        let rows = q.query_map([p], |r| r.get(0)).map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
        Ok(rows)
    }
    fn valid_group(name: &str) -> Result<&str, String> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 80 || name.chars().any(char::is_control) {
            return Err("Gruppenavnet må være mellom 1 og 80 tegn".into());
        }
        Ok(name)
    }
    fn ensure_group(conn: &Connection, p: &str, name: &str) -> Result<(), String> {
        let name = Self::valid_group(name)?;
        let exists: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM bookmark_groups WHERE profile=?1 AND name=?2)",params![p,name],|r|r.get(0)).map_err(|e| e.to_string())?;
        if exists { return Ok(()); }
        let count: i64 = conn.query_row("SELECT count(*) FROM bookmark_groups WHERE profile=?1",[p],|r|r.get(0)).map_err(|e| e.to_string())?;
        if count >= 250 { return Err("Maksimalt 250 bokmerkegrupper per profil".into()); }
        conn.execute("INSERT INTO bookmark_groups(profile,name) VALUES(?1,?2)",params![p,name]).map(|_|()).map_err(|e|e.to_string())
    }
    pub fn create_bookmark_group(&self, p: &str, name: &str) -> Result<(), String> {
        Self::ensure_group(&self.conn,p,name)
    }
    pub fn rename_bookmark_group(&self, p: &str, old: &str, name: &str) -> Result<(), String> {
        let name = Self::valid_group(name)?;
        let changed = self.conn.execute("UPDATE bookmark_groups SET name=?3 WHERE profile=?1 AND name=?2",params![p,old,name])
            .map_err(|_|"Gruppen finnes allerede eller kunne ikke endres".to_string())?;
        if changed == 0 { return Err("Fant ikke bokmerkegruppen i denne profilen".into()); }
        Ok(())
    }
    pub fn remove_bookmark_group(&self, p: &str, name: &str) -> Result<(), String> {
        // ON DELETE CASCADE removes memberships, never the bookmarks themselves.
        self.conn.execute("DELETE FROM bookmark_groups WHERE profile=?1 AND name=?2",params![p,name])
            .map(|_|()).map_err(|e|e.to_string())
    }
    pub fn move_bookmark(&self, p: &str, id: i64, group: &str) -> Result<(), String> {
        let exists: bool = self.conn.query_row("SELECT EXISTS(SELECT 1 FROM bookmarks WHERE id=?1 AND profile=?2)",params![id,p],|r|r.get(0)).map_err(|e|e.to_string())?;
        if !exists { return Err("Fant ikke bokmerket i denne profilen".into()); }
        if group.is_empty() {
            return self.conn.execute("DELETE FROM bookmark_group_entries WHERE bookmark=?1 AND profile=?2",params![id,p])
                .map(|_|()).map_err(|e|e.to_string());
        }
        Self::valid_group(group)?;
        self.conn.execute("INSERT INTO bookmark_group_entries(bookmark,profile,name) VALUES(?1,?2,?3) ON CONFLICT(bookmark) DO UPDATE SET name=excluded.name",params![id,p,group])
            .map(|_|()).map_err(|_|"Fant ikke bokmerkegruppen i denne profilen".into())
    }
    pub fn edit_bookmark(&self, p: &str, id: i64, title: &str, address: &str) -> Result<(), String> {
        let url = Self::valid_bookmark(address,title)?;
        let changed = self.conn.execute("UPDATE bookmarks SET title=?3,url=?4,favicon=CASE WHEN url=?4 THEN favicon ELSE NULL END WHERE id=?1 AND profile=?2",params![id,p,title.trim(),url.as_str()])
            .map_err(|_|"Adressen er allerede lagret eller bokmerket kunne ikke endres".to_string())?;
        if changed == 0 { return Err("Fant ikke bokmerket i denne profilen".into()); }
        Ok(())
    }
    fn valid_bookmark(address: &str, title: &str) -> Result<url::Url, String> {
        let url = url::Url::parse(address).map_err(|_| "Ugyldig bokmerkeadresse")?;
        if !crate::security::remote_allowed(&url) || !url.username().is_empty() || url.password().is_some()
            || address.len() > 8192 || title.len() > 1000 {
            return Err("Bokmerket må ha en gyldig HTTP- eller HTTPS-adresse".into());
        }
        Ok(url)
    }
    pub fn import_bookmarks(&self, p: &str, items: Vec<Entry>) -> Result<usize, String> {
        self.import_bookmark_collection(p,items,Vec::new())
    }
    pub fn import_bookmark_collection(&self, p: &str, items: Vec<Entry>, groups: Vec<String>) -> Result<usize, String> {
        if items.len() > 5000 || groups.len() > 250 {
            return Err("Maksimalt 5000 bokmerker og 250 grupper om gangen".into());
        }
        let tx = self.conn.unchecked_transaction().map_err(|e| e.to_string())?;
        for group in groups { Self::ensure_group(&tx,p,&group)?; }
        let mut count = 0;
        for item in items {
            let url = Self::valid_bookmark(&item.url,&item.title)?;
            if !item.group.is_empty() { Self::ensure_group(&tx,p,&item.group)?; }
            let inserted = tx.execute("INSERT OR IGNORE INTO bookmarks(profile,url,title,time) VALUES(?1,?2,?3,?4)",
                params![p,url.as_str(),item.title,chrono::Utc::now().to_rfc3339()]).map_err(|e|e.to_string())?;
            count += inserted;
            // Merge leaves local titles and existing memberships intact, as before.
            if inserted > 0 && !item.group.is_empty() {
                tx.execute("INSERT INTO bookmark_group_entries(bookmark,profile,name) VALUES(?1,?2,?3)",
                    params![tx.last_insert_rowid(),p,item.group.trim()]).map_err(|e|e.to_string())?;
            }
        }
        tx.commit().map_err(|e| e.to_string())?;
        Ok(count)
    }
    pub fn bookmark_icon(&self, p: &str, url: &str, icon: Option<&str>) -> Result<(), String> {
        if icon.is_some_and(|i| !i.starts_with("data:image/png;base64,") || i.len() > 700_000) {
            return Err("Ugyldig bokmerkeikon".into());
        }
        self.conn
            .execute(
                "UPDATE bookmarks SET favicon=?3 WHERE profile=?1 AND url=?2",
                params![p, url, icon],
            )
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
    pub fn permission(&self, p: &str, origin: &str, kind: &str) -> String {
        self.conn
            .query_row(
                "SELECT decision FROM site_permissions WHERE profile=?1 AND origin=?2 AND kind=?3",
                params![p, origin, kind],
                |r| r.get(0),
            )
            .unwrap_or_else(|_| "ask".into())
    }
    pub fn set_permission(
        &self,
        p: &str,
        origin: &str,
        kind: &str,
        decision: &str,
    ) -> Result<(), String> {
        let url = url::Url::parse(origin).map_err(|_| "Ugyldig nettsted")?;
        if !matches!(url.scheme(), "https" | "http")
            || url.origin().ascii_serialization() != origin
            || !SITE_PERMISSIONS.contains(&kind)
            || !["ask", "allow", "deny"].contains(&decision)
        {
            return Err("Ugyldig nettstedstillatelse".into());
        }
        self.conn.execute("INSERT INTO site_permissions VALUES(?1,?2,?3,?4) ON CONFLICT(profile,origin,kind) DO UPDATE SET decision=excluded.decision",params![p,origin,kind,decision]).map(|_|()).map_err(|e|e.to_string())
    }
    pub fn save_session(&self, p: &str, data: &str) -> Result<(), String> {
        // One SQLite transaction keeps the current snapshot and its fallback together.
        let tx = self
            .conn
            .unchecked_transaction()
            .map_err(|e| e.to_string())?;
        tx.execute("INSERT INTO session_backups(profile,data) SELECT profile,data FROM sessions WHERE profile=?1 AND data<>?2",params![p,data]).map_err(|e|e.to_string())?;
        tx.execute("DELETE FROM session_backups WHERE profile=?1 AND id NOT IN (SELECT id FROM session_backups WHERE profile=?1 ORDER BY id DESC LIMIT 3)",[p]).map_err(|e|e.to_string())?;
        tx.execute("INSERT INTO sessions VALUES(?1,?2) ON CONFLICT(profile) DO UPDATE SET data=excluded.data",params![p,data]).map_err(|e|e.to_string())?;
        tx.commit().map_err(|e| e.to_string())
    }
    pub fn session_candidates(&self, p: &str) -> Vec<String> {
        let mut result = self.load_session(p).into_iter().collect::<Vec<_>>();
        if let Ok(mut q) = self
            .conn
            .prepare("SELECT data FROM session_backups WHERE profile=?1 ORDER BY id DESC")
        {
            if let Ok(rows) = q.query_map([p], |r| r.get::<_, String>(0)) {
                result.extend(rows.flatten());
            }
        }
        result
    }
    pub fn load_session(&self, p: &str) -> Option<String> {
        self.conn
            .query_row("SELECT data FROM sessions WHERE profile=?1", [p], |r| {
                r.get(0)
            })
            .ok()
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vertical_layout_survives_reopen_and_is_scoped_to_the_profile() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("browser.sqlite");
        let db = Store::open(&path).unwrap();
        let right = TabLayout { vertical: true, side: TabSide::Right, width: 320 };
        let hidden = TabLayout { vertical: false, side: TabSide::Left, width: 185 };
        db.save_tab_layout("personal", right).unwrap();
        db.save_tab_layout("work", hidden).unwrap();
        assert!(db.save_tab_layout("personal", TabLayout { width: 0, ..right }).is_err());
        drop(db);
        let db = Store::open(&path).unwrap();
        assert_eq!(db.tab_layout("personal"), right);
        assert_eq!(db.tab_layout("work"), hidden);
        assert_eq!(db.tab_layout("legacy"), TabLayout::default());
    }

    #[test]
    fn workspace_identities_and_site_permissions_survive_reopen() {
        let temp = tempfile::tempdir().unwrap();
        let db = temp.path().join("profiles.db");
        let origin = "https://example.com";
        let mut store = Store::open(&db).unwrap();
        let previous = store.create_profile("Jobb").unwrap();
        let presets = [("work","allow"), ("personal","deny"), ("gaming","ask")];
        let mut ids = Vec::new();
        for (preset, decision) in presets {
            let p = store.workspace_profile(preset).unwrap();
            assert!(uuid::Uuid::parse_str(&p.id).is_ok());
            assert_ne!(p.id, previous.id);
            assert!(!ids.contains(&p.id));
            assert_eq!(store.permission(&p.id, origin, "camera"), "ask");
            store.set_permission(&p.id, origin, "camera", decision).unwrap();
            store.save_session(&p.id, preset).unwrap();
            ids.push(p.id);
        }
        assert!(store.workspace_profile("unknown").is_err());
        store.set("active_profile", &ids[2]).unwrap();
        drop(store);
        let mut reopened = Store::open(&db).unwrap();
        assert_eq!(reopened.get("active_profile").as_ref(), Some(&ids[2]));
        let profiles = reopened.profiles().unwrap();
        assert_eq!(profiles.len(), 5);
        for ((preset, decision), id) in presets.into_iter().zip(ids) {
            assert_eq!(reopened.workspace_profile(preset).unwrap().id, id);
            assert_eq!(profiles.iter().find(|p| p.id == id).unwrap().workspace.as_deref(), Some(preset));
            assert_eq!(reopened.permission(&id, origin, "camera"), decision);
            assert_eq!(reopened.load_session(&id).as_deref(), Some(preset));
        }
        assert_eq!(reopened.permission(&previous.id, origin, "camera"), "ask");
        assert_eq!(reopened.load_session(&previous.id), None);
    }
    #[test]
    fn upgrade_preserves_and_isolates_data() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("browser.sqlite");
        let p;
        {
            let s = Store::open(&path).unwrap();
            p = s.profiles().unwrap()[0].id.clone();
            s.bookmark(&p, "https://example.com/", "Example").unwrap();
            s.visit(&p, "https://example.com/", "Example").unwrap();
            s.set(
                "preferences",
                &serde_json::to_string(&Settings {
                    developer: true,
                    ..Default::default()
                })
                .unwrap(),
            )
            .unwrap();
            let other = s.create_profile("Jobb").unwrap();
            assert!(s.entries(&other.id, "history").unwrap().is_empty());
        }
        let s = Store::open(&path).unwrap();
        assert_eq!(s.entries(&p, "bookmarks").unwrap().len(), 1);
        assert_eq!(s.entries(&p, "history").unwrap().len(), 1);
        assert!(s.settings().developer);
    }
    #[test]
    fn alpha_one_preferences_gain_theme_defaults_without_losing_values() {
        let old: Settings = serde_json::from_str(
            r##"{"developer":true,"gaming":true,"channel":"beta","suspend_minutes":30}"##,
        )
        .unwrap();
        assert!(old.developer && old.gaming);
        assert_eq!(old.channel, "beta");
        assert_eq!(old.suspend_minutes, 30);
        assert_eq!(old.appearance, Appearance::default());
        assert!(old.valid());
    }
    #[test]
    fn appearance_survives_reopen_and_rejects_css_injection() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("theme.sqlite");
        let mut settings = Settings::default();
        settings.appearance.background = "#f4f7fb".into();
        settings.appearance.accent = "#1768cf".into();
        settings.appearance.radius = 0;
        settings.appearance.density = "compact".into();
        settings.appearance.sidebar = "expanded".into();
        {
            let store = Store::open(&path).unwrap();
            store
                .set("preferences", &serde_json::to_string(&settings).unwrap())
                .unwrap();
        }
        assert_eq!(
            Store::open(&path).unwrap().settings().appearance,
            settings.appearance
        );
        settings.appearance.accent = "red; background:url(https://example.com)".into();
        assert!(!settings.valid());
        settings.appearance.accent = "#ffffff".into();
        settings.appearance.radius = 21;
        assert!(!settings.valid());
    }
    #[test]
    fn alpha_three_preferences_preserve_custom_theme_and_gain_sidebar() {
        let settings: Settings = serde_json::from_str(r##"{"channel":"developer","appearance":{"accent":"#1768cf","background":"#f4f7fb","density":"compact","radius":3,"gradient":false,"reduce_motion":true}}"##).unwrap();
        assert_eq!(settings.channel, "developer");
        assert_eq!(settings.appearance.accent, "#1768cf");
        assert_eq!(settings.appearance.sidebar, "icons");
        assert!(settings.appearance.reduce_motion && !settings.appearance.gradient);
        assert!(settings.valid());
        let mut invalid = settings;
        invalid.appearance.sidebar = "invalid".into();
        assert!(!invalid.valid());
    }
    #[test]
    fn alpha_four_settings_keep_defaults_and_bookmark_placement() {
        let mut settings: Settings =
            serde_json::from_str(r#"{"appearance":{"sidebar":"expanded"},"developer":true}"#)
                .unwrap();
        assert_eq!(settings.appearance.bookmark_bar, "toolbar");
        assert!(!settings.password_autosave && !settings.autofill);
        assert!(settings.valid());
        for placement in ["hidden", "sidebar", "toolbar"] {
            settings.appearance.bookmark_bar = placement.into();
            assert!(settings.valid());
        }
        settings.appearance.bookmark_bar = "outside".into();
        assert!(!settings.valid());
    }
    #[test]
    fn import_is_atomic_deduplicated_and_profile_scoped() {
        let d = tempfile::tempdir().unwrap();
        let s = Store::open(&d.path().join("test.sqlite")).unwrap();
        let p = s.profiles().unwrap()[0].id.clone();
        let item = |url: &str| Entry {
            id: 0,
            url: url.into(),
            title: "Imported".into(),
            time: String::new(),
            favicon: None,
            group: String::new(),
        };
        assert!(s
            .import_bookmarks(
                &p,
                vec![item("https://example.com/"), item("javascript:alert(1)")]
            )
            .is_err());
        assert!(s.entries(&p, "bookmarks").unwrap().is_empty());
        assert_eq!(
            s.import_bookmarks(
                &p,
                vec![item("https://example.com"), item("https://example.com/")]
            )
            .unwrap(),
            1
        );
        let other = s.create_profile("Other").unwrap();
        assert!(s.entries(&other.id, "bookmarks").unwrap().is_empty());
        s.bookmark_icon(
            &p,
            "https://example.com/",
            Some("data:image/png;base64,iVBORw0KGgo="),
        )
        .unwrap();
        assert!(s.entries(&p, "bookmarks").unwrap()[0].favicon.is_some());
        assert!(s
            .bookmark_icon(
                &p,
                "https://example.com/",
                Some("https://tracker.example/icon")
            )
            .is_err());
    }
    #[test]
    fn checkpoints_are_bounded_and_permission_choices_are_isolated() {
        let d = tempfile::tempdir().unwrap();
        let s = Store::open(&d.path().join("test.sqlite")).unwrap();
        let p = s.profiles().unwrap()[0].id.clone();
        for n in 0..10 {
            s.save_session(&p, &format!("session-{n}")).unwrap();
        }
        assert_eq!(
            s.session_candidates(&p),
            vec!["session-9", "session-8", "session-7", "session-6"]
        );
        s.save_session(&p, "session-9").unwrap();
        assert_eq!(s.session_candidates(&p).len(), 4);
        s.set_permission(&p, "https://example.com", "camera", "allow")
            .unwrap();
        let other = s.create_profile("Other").unwrap();
        assert_eq!(
            s.permission(&other.id, "https://example.com", "camera"),
            "ask"
        );
        assert_eq!(s.permission(&p, "https://example.com", "camera"), "allow");
        s.set_permission(&p, "https://example.com", "multiple_downloads", "allow").unwrap();
        assert_eq!(s.permission(&other.id, "https://example.com", "multiple_downloads"), "ask");
        assert_eq!(s.permission(&p, "https://elsewhere.example", "multiple_downloads"), "ask");
        assert!(s
            .set_permission(&p, "https://example.com/path", "camera", "allow")
            .is_err());
    }
    #[test]
    fn schema_one_migration_creates_backup_and_preserves_bookmarks() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("test.sqlite");
        {
            let c = Connection::open(&path).unwrap();
            c.execute_batch("CREATE TABLE profiles(id TEXT PRIMARY KEY,name TEXT NOT NULL);CREATE TABLE settings(key TEXT PRIMARY KEY,value TEXT NOT NULL);CREATE TABLE bookmarks(id INTEGER PRIMARY KEY,profile TEXT NOT NULL REFERENCES profiles(id),url TEXT NOT NULL,title TEXT NOT NULL,time TEXT NOT NULL,UNIQUE(profile,url));CREATE TABLE history(id INTEGER PRIMARY KEY,profile TEXT,url TEXT,title TEXT,time TEXT);CREATE TABLE sessions(profile TEXT PRIMARY KEY,data TEXT);INSERT INTO profiles VALUES('fixture','Original');INSERT INTO bookmarks VALUES(1,'fixture','https://example.com/','Original','old');PRAGMA user_version=1;").unwrap();
        }
        let s = Store::open(&path).unwrap();
        assert_eq!(
            s.entries("fixture", "bookmarks").unwrap()[0].title,
            "Original"
        );
        assert!(path.with_extension("v1.backup.sqlite").exists());
        assert_eq!(
            s.conn
                .pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
                .unwrap(),
            2
        );
    }
    #[test]
    fn newer_database_is_not_modified() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("new.sqlite");
        let s = Store::open(&p).unwrap();
        s.conn.pragma_update(None, "user_version", 99).unwrap();
        drop(s);
        assert!(Store::open(&p).is_err());
    }
    #[test]
    fn bookmark_groups_survive_reopen_and_are_profile_isolated() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("groups.sqlite");
        let p;
        let id;
        {
            let store = Store::open(&path).unwrap();
            p = store.profiles().unwrap()[0].id.clone();
            let other = store.create_profile("Jobb").unwrap();
            store.bookmark(&p,"https://example.com/","Example").unwrap();
            id = store.entries(&p,"bookmarks").unwrap()[0].id;
            store.create_bookmark_group(&p,"Gaming").unwrap();
            store.create_bookmark_group(&p,"Tom gruppe").unwrap();
            store.move_bookmark(&p,id,"Gaming").unwrap();
            assert!(store.move_bookmark(&other.id,id,"Gaming").is_err());
            assert!(store.edit_bookmark(&other.id,id,"Wrong","https://other.example/").is_err());
            assert!(store.bookmark_groups(&other.id).unwrap().is_empty());
            store.remove_bookmark_group(&other.id,"Gaming").unwrap();
            assert_eq!(store.entries(&p,"bookmarks").unwrap()[0].group,"Gaming");
        }
        let store = Store::open(&path).unwrap();
        assert_eq!(store.bookmark_groups(&p).unwrap(),vec!["Gaming","Tom gruppe"]);
        store.rename_bookmark_group(&p,"Gaming","Spill").unwrap();
        assert_eq!(store.entries(&p,"bookmarks").unwrap()[0].group,"Spill");
        assert!(store.rename_bookmark_group(&p,"Spill","Tom gruppe").is_err());
        store.edit_bookmark(&p,id,"New title","https://example.com/new").unwrap();
        assert_eq!(store.entries(&p,"bookmarks").unwrap()[0].group,"Spill");
        store.remove_bookmark_group(&p,"Spill").unwrap();
        let items=store.entries(&p,"bookmarks").unwrap();
        assert_eq!(items.len(),1);
        assert_eq!(items[0].title,"New title");
        assert!(items[0].group.is_empty());
    }
    #[test]
    fn additive_groups_keep_schema_two_and_old_client_deletes_working() {
        let d = tempfile::tempdir().unwrap();
        let path=d.path().join("old.sqlite");
        let p;
        {
            let store=Store::open(&path).unwrap();
            p=store.profiles().unwrap()[0].id.clone();
            store.bookmark(&p,"https://example.com/","Original").unwrap();
            store.conn.execute_batch("DROP TABLE bookmark_group_entries; DROP TABLE bookmark_groups;").unwrap();
        }
        let store=Store::open(&path).unwrap();
        assert!(path.with_extension("v2.bookmark-groups.backup.sqlite").exists());
        assert_eq!(store.conn.pragma_query_value(None,"user_version",|r|r.get::<_,i64>(0)).unwrap(),2);
        let id=store.entries(&p,"bookmarks").unwrap()[0].id;
        store.create_bookmark_group(&p,"Retained").unwrap();
        store.move_bookmark(&p,id,"Retained").unwrap();
        // The previous client's star action updates only the title.
        store.bookmark(&p,"https://example.com/","Updated by old client").unwrap();
        assert_eq!(store.entries(&p,"bookmarks").unwrap()[0].group,"Retained");
        store.remove_bookmark(&p,id).unwrap();
        assert_eq!(store.conn.query_row("SELECT count(*) FROM bookmark_group_entries",[],|r|r.get::<_,i64>(0)).unwrap(),0);
        assert_eq!(store.bookmark_groups(&p).unwrap(),vec!["Retained"]);
    }
    #[test]
    fn bookmark_group_import_is_atomic_and_accepts_older_sync_payloads() {
        let d=tempfile::tempdir().unwrap();
        let store=Store::open(&d.path().join("sync.sqlite")).unwrap();
        let p=store.profiles().unwrap()[0].id.clone();
        let old:Entry=serde_json::from_str(r#"{"id":1,"url":"https://example.com/","title":"Old","time":""}"#).unwrap();
        assert!(old.group.is_empty());
        store.import_bookmarks(&p,vec![old.clone()]).unwrap();
        let mut grouped=old.clone();grouped.url="https://example.org/".into();grouped.group="ELIX".into();
        let mut invalid=old;invalid.url="javascript:alert(1)".into();
        assert!(store.import_bookmark_collection(&p,vec![grouped.clone(),invalid],vec!["Empty".into()]).is_err());
        assert_eq!(store.entries(&p,"bookmarks").unwrap().len(),1);
        assert!(store.bookmark_groups(&p).unwrap().is_empty());
        store.import_bookmark_collection(&p,vec![grouped],vec!["Empty".into()]).unwrap();
        assert_eq!(store.bookmark_groups(&p).unwrap(),vec!["ELIX","Empty"]);
        let entries=store.entries(&p,"bookmarks").unwrap();
        assert_eq!(entries.iter().find(|item|item.url=="https://example.org/").unwrap().group,"ELIX");
        store.import_bookmark_collection(&p,entries,vec![]).unwrap();
        assert_eq!(store.entries(&p,"bookmarks").unwrap().len(),2);
    }
    #[test]
    fn bookmark_edits_validate_urls_duplicates_and_group_names() {
        let d=tempfile::tempdir().unwrap();
        let store=Store::open(&d.path().join("edit.sqlite")).unwrap();
        let p=store.profiles().unwrap()[0].id.clone();
        store.bookmark(&p,"https://example.com/","First").unwrap();
        store.bookmark(&p,"https://example.org/","Second").unwrap();
        let id=store.entries(&p,"bookmarks").unwrap().iter().find(|e|e.title=="First").unwrap().id;
        assert!(store.edit_bookmark(&p,id,"Unsafe","javascript:alert(1)").is_err());
        assert!(store.edit_bookmark(&p,id,"Duplicate","https://example.org/").is_err());
        assert!(store.create_bookmark_group(&p,"  ").is_err());
        assert!(store.create_bookmark_group(&p,"Bad\nname").is_err());
        assert!(store.move_bookmark(&p,id,"Missing").is_err());
        assert_eq!(store.entries(&p,"bookmarks").unwrap().iter().find(|e|e.id==id).unwrap().title,"First");
    }

}
