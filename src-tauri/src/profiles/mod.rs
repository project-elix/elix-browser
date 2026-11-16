pub mod accounts;
use std::path::{Path, PathBuf};
pub fn directory(root: &Path, id: &str) -> Result<PathBuf, String> {
    if !crate::security::valid_profile(id) {
        return Err("Ugyldig profil-ID".into());
    }
    let path = root.join("profiles").join(id).join("webview2");
    std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    Ok(path)
}
