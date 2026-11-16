use serde::Serialize;
use std::path::PathBuf;
#[derive(Clone, Serialize)]
pub struct Download {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    pub status: String,
    pub time: String,
}
pub fn destination(
    root: PathBuf,
    suggested: &std::path::Path,
) -> Result<(String, PathBuf), std::io::Error> {
    let id = uuid::Uuid::new_v4().to_string();
    let dir = root.join("ELIX").join(&id);
    std::fs::create_dir_all(&dir)?;
    let name = suggested
        .file_name()
        .and_then(|v| v.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("download");
    Ok((id, dir.join(name)))
}
