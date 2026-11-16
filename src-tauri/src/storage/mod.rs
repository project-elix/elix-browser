use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Entry {
    pub id: i64,
    pub url: String,
    pub title: String,
    pub time: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub developer: bool,
    pub gaming: bool,
    pub channel: String,
    pub suspend_minutes: u32,
    pub appearance: Appearance,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            developer: false,
            gaming: false,
            channel: "stable".into(),
            suspend_minutes: 0,
            appearance: Appearance::default(),
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
    }
}
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
        if version > 1 {
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
            let tx = conn.transaction().map_err(|e| e.to_string())?;
            tx.execute_batch("CREATE TABLE IF NOT EXISTS profiles(id TEXT PRIMARY KEY, name TEXT NOT NULL); CREATE TABLE IF NOT EXISTS settings(key TEXT PRIMARY KEY, value TEXT NOT NULL); CREATE TABLE IF NOT EXISTS bookmarks(id INTEGER PRIMARY KEY, profile TEXT NOT NULL REFERENCES profiles(id), url TEXT NOT NULL, title TEXT NOT NULL, time TEXT NOT NULL, UNIQUE(profile,url)); CREATE TABLE IF NOT EXISTS history(id INTEGER PRIMARY KEY, profile TEXT NOT NULL REFERENCES profiles(id), url TEXT NOT NULL, title TEXT NOT NULL, time TEXT NOT NULL); CREATE INDEX IF NOT EXISTS history_profile_time ON history(profile,time); CREATE TABLE IF NOT EXISTS sessions(profile TEXT PRIMARY KEY REFERENCES profiles(id), data TEXT NOT NULL); PRAGMA user_version=1;").map_err(|e|e.to_string())?;
            tx.commit().map_err(|e| e.to_string())?;
        }
        let store = Self { conn };
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
            .prepare("SELECT id,name FROM profiles ORDER BY rowid")
            .map_err(|e| e.to_string())?;
        let items = q
            .query_map([], |r| {
                Ok(Profile {
                    id: r.get(0)?,
                    name: r.get(1)?,
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        Ok(items)
    }
    pub fn create_profile(&self, name: &str) -> Result<Profile, String> {
        let name = name.trim();
        if name.is_empty() || name.len() > 60 {
            return Err("Profilnavnet må være mellom 1 og 60 tegn".into());
        }
        let p = Profile {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.into(),
        };
        self.conn
            .execute("INSERT INTO profiles VALUES(?1,?2)", params![p.id, p.name])
            .map_err(|e| e.to_string())?;
        Ok(p)
    }
    pub fn settings(&self) -> Settings {
        self.get("preferences")
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }
    pub fn entries(&self, profile: &str, kind: &str) -> Result<Vec<Entry>, String> {
        let sql=match kind{"bookmarks"=>"SELECT id,url,title,time FROM bookmarks WHERE profile=?1 ORDER BY id DESC LIMIT 1000", "history"=>"SELECT id,url,title,time FROM history WHERE profile=?1 ORDER BY id DESC LIMIT 1000",_=>return Err("Ukjent liste".into())};
        let mut q = self.conn.prepare(sql).map_err(|e| e.to_string())?;
        let rows = q
            .query_map([profile], |r| {
                Ok(Entry {
                    id: r.get(0)?,
                    url: r.get(1)?,
                    title: r.get(2)?,
                    time: r.get(3)?,
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
    pub fn save_session(&self, p: &str, data: &str) -> Result<(), String> {
        self.conn.execute("INSERT INTO sessions VALUES(?1,?2) ON CONFLICT(profile) DO UPDATE SET data=excluded.data",params![p,data]).map(|_|()).map_err(|e|e.to_string())
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
    fn newer_database_is_not_modified() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("new.sqlite");
        let s = Store::open(&p).unwrap();
        s.conn.pragma_update(None, "user_version", 99).unwrap();
        drop(s);
        assert!(Store::open(&p).is_err());
    }
}
