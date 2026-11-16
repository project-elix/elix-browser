//! Download metadata is kept separately from downloaded files and browser sessions.
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Download {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    pub status: String,
    pub time: String,
    #[serde(default)]
    pub profile: String,
    #[serde(default)]
    pub private: bool,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub bytes_received: u64,
    #[serde(default)]
    pub total_bytes: Option<u64>,
    #[serde(default)]
    pub can_resume: bool,
    #[serde(default)]
    pub view_id: String,
}
impl Download {
    pub fn new(
        id: String,
        path: PathBuf,
        profile: String,
        private: bool,
        url: String,
        view_id: String,
    ) -> Self {
        Self {
            id,
            name: path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into(),
            path,
            profile,
            private,
            url,
            view_id,
            status: "downloading".into(),
            time: chrono::Utc::now().to_rfc3339(),
            bytes_received: 0,
            total_bytes: None,
            can_resume: false,
        }
    }
}
fn database(root: &Path) -> Result<Connection, String> {
    let connection = Connection::open(root.join("downloads.sqlite3")).map_err(|e| e.to_string())?;
    connection
        .busy_timeout(std::time::Duration::from_secs(3))
        .map_err(|e| e.to_string())?;
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS downloads (id TEXT PRIMARY KEY, data TEXT NOT NULL)",
        )
        .map_err(|e| e.to_string())?;
    Ok(connection)
}
/// Native WebView2 operations cannot be restored after the browser process exits.
/// Keep files and their metadata, but do not offer a resume button for a dead operation.
pub fn load(root: &Path) -> Result<Vec<Download>, String> {
    let connection = database(root)?;
    let mut statement = connection
        .prepare("SELECT data FROM downloads ORDER BY rowid")
        .map_err(|e| e.to_string())?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| e.to_string())?;
    let mut result = Vec::new();
    for row in rows {
        let mut download: Download =
            serde_json::from_str(&row.map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        if matches!(download.status.as_str(), "downloading" | "paused") || download.can_resume {
            download.status = "interrupted".into();
        }
        download.can_resume = false;
        download.view_id.clear();
        if !download.private {
            result.push(download);
        }
    }
    Ok(result)
}
pub fn save(root: &Path, downloads: &[Download]) -> Result<(), String> {
    let mut connection = database(root)?;
    let transaction = connection.transaction().map_err(|e| e.to_string())?;
    for download in downloads.iter().filter(|d| !d.private) {
        let data = serde_json::to_string(download).map_err(|e| e.to_string())?;
        transaction.execute("INSERT INTO downloads(id,data) VALUES(?1,?2) ON CONFLICT(id) DO UPDATE SET data=excluded.data", params![download.id, data]).map_err(|e| e.to_string())?;
    }
    transaction.commit().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn restart_keeps_completed_files_and_interrupts_live_operations() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("retained.txt");
        std::fs::write(&path, "content").unwrap();
        let mut complete = Download::new(
            "done".into(),
            path.clone(),
            "profile-a".into(),
            false,
            "https://example.com/file".into(),
            "tab-a".into(),
        );
        complete.status = "complete".into();
        let mut paused = complete.clone();
        paused.id = "paused".into();
        paused.status = "paused".into();
        paused.can_resume = true;
        let mut private = complete.clone();
        private.id = "private".into();
        private.private = true;
        save(root.path(), &[complete, paused, private]).unwrap();
        let restored = load(root.path()).unwrap();
        assert_eq!(restored.len(), 2);
        assert_eq!(restored[0].status, "complete");
        assert_eq!(restored[1].status, "interrupted");
        assert!(!restored[1].can_resume);
        assert!(restored[1].view_id.is_empty());
        assert_eq!(std::fs::read_to_string(path).unwrap(), "content");
    }
    #[test]
    fn updates_are_atomic_and_preserve_other_profiles() {
        let root = tempfile::tempdir().unwrap();
        let mut a = Download::new(
            "a".into(),
            root.path().join("a"),
            "profile-a".into(),
            false,
            String::new(),
            String::new(),
        );
        let b = Download::new(
            "b".into(),
            root.path().join("b"),
            "profile-b".into(),
            false,
            String::new(),
            String::new(),
        );
        save(root.path(), &[a.clone(), b]).unwrap();
        a.status = "complete".into();
        a.bytes_received = 123;
        save(root.path(), &[a]).unwrap();
        let restored = load(root.path()).unwrap();
        assert_eq!(restored.len(), 2);
        assert_eq!(restored[0].bytes_received, 123);
        assert_eq!(restored[1].profile, "profile-b");
    }
}
