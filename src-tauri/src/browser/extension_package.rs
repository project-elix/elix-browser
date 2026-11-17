//! Publisher-source ZIP import. CRX containers are deliberately not unpacked:
//! WebView2's folder API cannot preserve Chrome Web Store signature verification.
use serde_json::Value;
use std::{collections::HashSet, fs, io::{Cursor, Read, Write}, path::{Path, PathBuf}};

const MAX_ARCHIVE: u64 = 64 * 1024 * 1024;
const MAX_EXPANDED: u64 = 256 * 1024 * 1024;
const MAX_FILE: u64 = 32 * 1024 * 1024;
const MAX_FILES: usize = 10_000;
const MAX_MANIFEST: u64 = 256 * 1024;

#[derive(Debug)]
pub struct ManifestReview {
    pub name: String,
    pub version: String,
    pub permissions: Vec<String>,
    pub optional_permissions: Vec<String>,
}

pub struct PreparedExtension {
    pub path: PathBuf,
    pub review: ManifestReview,
    cleanup: Option<PathBuf>,
}
impl PreparedExtension {
    // Once native installation starts, a timeout does not prove it failed. Keep
    // its source intact even if the asynchronous completion arrives later.
    pub fn retain_source(&mut self) { self.cleanup = None; }
}
impl Drop for PreparedExtension {
    fn drop(&mut self) {
        if let Some(path) = &self.cleanup { let _ = fs::remove_dir_all(path); }
    }
}

fn display_text(text: &str) -> String {
    text.chars().filter(|c| !c.is_control() && !matches!(*c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')).collect()
}

fn strings(value: Option<&Value>) -> Result<Vec<String>, String> {
    let Some(value) = value else { return Ok(Vec::new()); };
    let list = value.as_array().ok_or("Ugyldig tillatelsesliste i manifest.json")?;
    if list.len() > 128 { return Err("Utvidelsen oppgir for mange tillatelser til lokal import.".into()); }
    list.iter().map(|entry| {
        let text = entry.as_str().filter(|v| !v.is_empty() && v.len() <= 512)
            .ok_or("Ugyldig tillatelse i manifest.json")?;
        Ok(display_text(text))
    }).collect()
}

pub fn review_manifest(path: &Path) -> Result<ManifestReview, String> {
    let manifest_path = path.join("manifest.json");
    let metadata = fs::symlink_metadata(&manifest_path).map_err(|_| "Velg en utvidelse med manifest.json i rotmappen.")?;
    if !metadata.is_file() || metadata.len() > MAX_MANIFEST { return Err("manifest.json må være en vanlig fil på høyst 256 KiB.".into()); }
    let file = fs::File::open(&manifest_path).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new(); file.take(MAX_MANIFEST + 1).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_MANIFEST { return Err("manifest.json er for stor.".into()); }
    let manifest: Value = serde_json::from_slice(&bytes).map_err(|_| "manifest.json er ikke gyldig JSON.")?;
    if !matches!(manifest["manifest_version"].as_u64(), Some(2 | 3)) || manifest.get("app").is_some() || manifest.get("theme").is_some() {
        return Err("Velg en nettleserutvidelse med manifestversjon 2 eller 3, ikke en app eller et tema.".into());
    }
    let field = |name: &str| manifest[name].as_str().filter(|s| !s.trim().is_empty() && s.len() <= 256)
        .map(display_text).ok_or_else(|| format!("Manifestet mangler gyldig {name}."));
    let mut permissions = strings(manifest.get("permissions"))?;
    permissions.extend(strings(manifest.get("host_permissions"))?);
    if let Some(scripts) = manifest.get("content_scripts") {
        let scripts = scripts.as_array().filter(|v| v.len() <= 128).ok_or("Ugyldige innholdsskript i manifestet.")?;
        for script in scripts { permissions.extend(strings(script.get("matches"))?); }
    }
    permissions.sort(); permissions.dedup();
    let mut optional_permissions = strings(manifest.get("optional_permissions"))?;
    optional_permissions.extend(strings(manifest.get("optional_host_permissions"))?);
    optional_permissions.sort(); optional_permissions.dedup();
    if permissions.len() + optional_permissions.len() > 128 { return Err("Utvidelsen oppgir for mange tillatelser til lokal import.".into()); }
    Ok(ManifestReview { name: field("name")?, version: field("version")?, permissions, optional_permissions })
}

pub fn prepare_folder(path: PathBuf) -> Result<PreparedExtension, String> {
    let path = path.canonicalize().map_err(|e| e.to_string())?;
    let review = review_manifest(&path)?;
    Ok(PreparedExtension { path, review, cleanup: None })
}

fn safe_name(name: &str, directory: bool) -> Result<PathBuf, String> {
    let name = if directory { name.strip_suffix('/').unwrap_or(name) } else { name };
    if name.is_empty() || name.len() > 1024 || name.contains(['\\', ':']) || name.starts_with('/') {
        return Err("ZIP-filen inneholder en ugyldig filbane.".into());
    }
    for part in name.split('/') {
        if part.is_empty() || part == "." || part == ".." || part.ends_with(['.', ' ']) || part.chars().any(|c| c.is_control() || "<>\"|?*".contains(c)) {
            return Err("ZIP-filen inneholder en utrygg filbane.".into());
        }
        let stem = part.split('.').next().unwrap_or("").to_ascii_uppercase();
        let port_suffix = stem.strip_prefix("COM").or_else(|| stem.strip_prefix("LPT"));
        if ["CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$"].contains(&stem.as_str())
            || port_suffix.is_some_and(|suffix| matches!(suffix, "0" | "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³")) {
            return Err("ZIP-filen bruker et reservert Windows-filnavn.".into());
        }
    }
    Ok(PathBuf::from(name))
}

fn extract(bytes: &[u8], destination: &Path) -> Result<PathBuf, String> {
    if bytes.starts_with(b"Cr24") { return Err("CRX-filer krever butikkens signaturkontroll. Bruk en ZIP med kildefiler fra utgiveren.".into()); }
    if !bytes.starts_with(b"PK\x03\x04") { return Err("Velg en vanlig ZIP-fil med utvidelsens kildefiler.".into()); }
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|_| "ZIP-filen kunne ikke leses.")?;
    if archive.len() > MAX_FILES || archive.has_duplicate_names() || archive.offset() != 0
        || archive.has_overlapping_entries().map_err(|_| "ZIP-strukturen er ugyldig.")? {
        return Err("ZIP-filen har for mange eller overlappende filer.".into());
    }
    let mut names = HashSet::new();
    let mut plan = Vec::new();
    let mut expanded = 0u64;
    for index in 0..archive.len() {
        let file = archive.by_index(index).map_err(|_| "ZIP-filen er kryptert, skadet eller bruker en komprimering som ikke støttes.")?;
        if file.encrypted() || file.is_symlink() || (!file.is_file() && !file.is_dir()) { return Err("ZIP-filen kan ikke inneholde lenker eller krypterte filer.".into()); }
        let name = file.name().map_err(|_| "ZIP-filen har ugyldige filnavn.")?;
        let relative = safe_name(&name, file.is_dir())?;
        if !names.insert(relative.to_string_lossy().to_lowercase()) { return Err("ZIP-filen har filnavn som kolliderer i Windows.".into()); }
        expanded = expanded.checked_add(file.size()).ok_or("ZIP-filen er for stor.")?;
        if file.size() > MAX_FILE || expanded > MAX_EXPANDED { return Err("ZIP-filen overskrider grensen på 32 MiB per fil eller 256 MiB utpakket.".into()); }
        plan.push((relative, file.is_dir(), file.size()));
    }
    // Accept either a root manifest or one enclosing publisher folder. Never
    // search recursively and silently select one extension from a bundle.
    let root_manifest = plan.iter().any(|(p, dir, _)| !dir && p == Path::new("manifest.json"));
    let prefix = if root_manifest { PathBuf::new() } else {
        let mut roots = plan.iter().filter(|(_, dir, _)| !dir).map(|(p, _, _)| p.components().next().unwrap().as_os_str());
        let first = roots.next().ok_or("ZIP-filen er tom.")?;
        if roots.any(|root| root != first) { return Err("ZIP-filen må inneholde én utvidelse med manifest.json i rotmappen.".into()); }
        let prefix = PathBuf::from(first);
        if !plan.iter().any(|(p, dir, _)| !dir && *p == prefix.join("manifest.json")) { return Err("Fant ikke manifest.json i utvidelsesmappen.".into()); }
        prefix
    };
    for (index, (relative, directory, size)) in plan.into_iter().enumerate() {
        let output = destination.join(relative);
        if directory { fs::create_dir_all(&output).map_err(|e| e.to_string())?; continue; }
        fs::create_dir_all(output.parent().unwrap()).map_err(|e| e.to_string())?;
        let mut target = fs::OpenOptions::new().write(true).create_new(true).open(output).map_err(|e| e.to_string())?;
        let file = archive.by_index(index).map_err(|_| "ZIP-filen kunne ikke pakkes ut.")?;
        let actual = std::io::copy(&mut file.take(size + 1), &mut target).map_err(|_| "ZIP-filen er skadet eller feilet kontrollsummen.")?;
        if actual != size { return Err("ZIP-filen oppgir feil utpakket størrelse.".into()); }
        target.flush().map_err(|e| e.to_string())?;
    }
    Ok(destination.join(prefix))
}

pub fn prepare_zip(source: &Path, managed_parent: &Path) -> Result<PreparedExtension, String> {
    let metadata = fs::symlink_metadata(source).map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.len() > MAX_ARCHIVE { return Err("Velg en vanlig ZIP-fil på høyst 64 MiB.".into()); }
    let mut bytes = Vec::new(); fs::File::open(source).map_err(|e| e.to_string())?.take(MAX_ARCHIVE + 1).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_ARCHIVE { return Err("ZIP-filen er for stor.".into()); }
    fs::create_dir_all(managed_parent).map_err(|e| e.to_string())?;
    let destination = managed_parent.join(uuid::Uuid::new_v4().to_string());
    fs::create_dir(&destination).map_err(|e| e.to_string())?;
    let prepared = (|| {
        let path = extract(&bytes, &destination)?;
        let review = review_manifest(&path)?;
        Ok(PreparedExtension { path, review, cleanup: Some(destination.clone()) })
    })();
    if prepared.is_err() { let _ = fs::remove_dir_all(&destination); }
    prepared
}

#[cfg(test)]
mod tests {
    use super::*;
    fn archive(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, bytes) in entries {
            zip.start_file(*name, zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated)).unwrap();
            zip.write_all(bytes).unwrap();
        }
        zip.finish().unwrap().into_inner()
    }
    const MANIFEST: &[u8] = br#"{"manifest_version":3,"name":"Publisher test","version":"1.0","permissions":["storage"],"host_permissions":["https://example.com/*"],"content_scripts":[{"matches":["https://other.example/*"],"js":["content.js"]}],"optional_permissions":["tabs"]}"#;
    #[test]
    fn imports_deflated_publisher_folder_and_retains_permissions() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source.zip");
        fs::write(&source, archive(&[("publisher/manifest.json", MANIFEST), ("publisher/content.js", b"document.title='extension';")])).unwrap();
        let parent = temp.path().join("managed");
        let prepared = prepare_zip(&source, &parent).unwrap();
        assert_eq!(prepared.review.name, "Publisher test");
        assert!(prepared.review.permissions.contains(&"https://other.example/*".to_string()));
        assert_eq!(prepared.review.optional_permissions, vec!["tabs"]);
        assert!(prepared.path.join("content.js").is_file());
        drop(prepared);
        assert_eq!(fs::read_dir(&parent).unwrap().count(), 0);
        assert!(source.is_file());
    }
    #[test]
    fn rejects_traversal_windows_aliases_and_case_collisions_without_outside_writes() {
        for bad in ["../escape.js", "x/../../escape.js", "/escape.js", "C:/escape.js", "x\\escape.js", "NUL.txt", "LPT1", "COM¹.txt", "folder./file.js", "file.js:stream"] {
            assert!(safe_name(bad, false).is_err(), "{bad}");
        }
        let temp = tempfile::tempdir().unwrap();
        for entries in [vec![("manifest.json", MANIFEST), ("../escape.js", b"bad".as_slice())], vec![("manifest.json", MANIFEST), ("MANIFEST.JSON", MANIFEST)]] {
            let bytes = archive(&entries);
            assert!(extract(&bytes, temp.path()).is_err());
            assert!(!temp.path().join("manifest.json").exists());
        }
    }
    #[test]
    fn rejects_crx_containers_instead_of_stripping_signature_headers() {
        let mut bytes = b"Cr24\x03\x00\x00\x00".to_vec();
        bytes.extend(archive(&[("manifest.json", MANIFEST)]));
        let temp = tempfile::tempdir().unwrap();
        assert!(extract(&bytes, temp.path()).unwrap_err().contains("CRX"));
        assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 0);
    }
    #[test]
    fn rejects_ambiguous_bundles_and_manifestless_archives() {
        let temp = tempfile::tempdir().unwrap();
        for entries in [vec![("a/manifest.json", MANIFEST), ("b/manifest.json", MANIFEST)], vec![("folder/other.json", MANIFEST)]] {
            assert!(extract(&archive(&entries), temp.path()).is_err());
        }
    }
    #[test]
    fn rejects_links_before_extracting_any_file() {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        zip.start_file("manifest.json", zip::write::SimpleFileOptions::default()).unwrap();
        zip.write_all(MANIFEST).unwrap();
        zip.add_symlink("escape", "../outside", zip::write::SimpleFileOptions::default()).unwrap();
        let bytes = zip.finish().unwrap().into_inner();
        let temp = tempfile::tempdir().unwrap();
        assert!(extract(&bytes, temp.path()).is_err());
        assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 0);
    }
    #[test]
    fn rejects_expansion_bomb_and_cleans_failed_import() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("oversized.zip");
        let large = vec![0u8; MAX_FILE as usize + 1];
        let bytes = archive(&[("manifest.json", MANIFEST), ("large.js", &large)]);
        assert!(bytes.len() < 1024 * 1024);
        fs::write(&source, bytes).unwrap();
        let parent = temp.path().join("managed");
        assert!(prepare_zip(&source, &parent).is_err());
        assert_eq!(fs::read_dir(parent).unwrap().count(), 0);
    }
    #[test]
    fn retains_approved_source_after_preparation_scope_ends() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source.zip");
        fs::write(&source, archive(&[("manifest.json", MANIFEST)])).unwrap();
        let mut prepared = prepare_zip(&source, &temp.path().join("managed")).unwrap();
        let path = prepared.path.clone();
        prepared.retain_source();
        drop(prepared);
        assert_eq!(fs::read(path.join("manifest.json")).unwrap(), MANIFEST);
    }
}
