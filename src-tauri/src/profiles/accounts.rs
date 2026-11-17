//! Provider secrets live on the account service. Desktop bearer tokens use Windows DPAPI.
use crate::AppState;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

pub fn configured() -> bool {
    base().is_ok()
}
fn base() -> Result<url::Url, String> {
    let value = option_env!("ELIX_ACCOUNTS_URL").unwrap_or("");
    let url =
        url::Url::parse(value).map_err(|_| "Kontotjenesten er ikke aktivert i dette bygget")?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.path() != "/"
    {
        return Err("Ugyldig kontotjeneste".into());
    }
    Ok(url)
}
fn path(app: &AppHandle, profile: &str) -> std::path::PathBuf {
    app.state::<AppState>()
        .root
        .join("profiles")
        .join(profile)
        .join("account.dat")
}

#[cfg(windows)]
fn protect(bytes: &[u8], encrypt: bool) -> Result<Vec<u8>, String> {
    use windows::{
        core::PCWSTR,
        Win32::{
            Foundation::{LocalFree, HLOCAL},
            Security::Cryptography::{
                CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
            },
        },
    };
    let input = CRYPT_INTEGER_BLOB {
        cbData: bytes.len() as u32,
        pbData: bytes.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB::default();
    unsafe {
        let result = if encrypt {
            CryptProtectData(
                &input,
                PCWSTR::null(),
                None,
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        } else {
            CryptUnprotectData(
                &input,
                None,
                None,
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        };
        result.map_err(|_| "Windows kunne ikke åpne den sikre kontolagringen")?;
        let data = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        LocalFree(Some(HLOCAL(output.pbData.cast())));
        Ok(data)
    }
}
#[cfg(not(windows))]
fn protect(_: &[u8], _: bool) -> Result<Vec<u8>, String> {
    Err("Sikker kontolagring krever Windows".into())
}
fn token(app: &AppHandle, profile: &str, base: &url::Url) -> Result<Option<String>, String> {
    read_token(&path(app, profile), profile, base)
}
fn read_token(
    path: &std::path::Path,
    profile: &str,
    base: &url::Url,
) -> Result<Option<String>, String> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err("Kunne ikke lese kontoen".into()),
    };
    if bytes.len() > 16000 {
        return Ok(None);
    }
    // A damaged credential or one copied from another Windows user must not block signing in again.
    // Leave other profile data untouched; the next successful login replaces only account.dat.
    let Some(value) = protect(&bytes, false)
        .ok()
        .and_then(|plain| serde_json::from_slice::<Value>(&plain).ok())
    else {
        return Ok(None);
    };
    if value["service"] != base.as_str() || value["profile"] != profile {
        return Ok(None);
    }
    Ok(value["token"]
        .as_str()
        .filter(|token| (32..=128).contains(&token.len()))
        .map(String::from))
}
fn write_token(path: &std::path::Path, data: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let parent = path.parent().ok_or("Ugyldig kontolagring")?;
    std::fs::create_dir_all(parent).map_err(|_| "Kunne ikke lagre kontoen")?;
    let temporary = parent.join(format!("account-{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| -> std::io::Result<()> {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(data)?;
        file.sync_all()?;
        drop(file);
        // Same-directory rename atomically replaces an existing file, including on Windows.
        std::fs::rename(&temporary, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result.map_err(|_| "Kunne ikke lagre kontoen".into())
}
fn save_token(app: &AppHandle, profile: &str, base: &url::Url, token: &str) -> Result<(), String> {
    if token.len() > 128 || token.len() < 32 {
        return Err("Ugyldig kontosesjon".into());
    }
    let data = protect(
        &serde_json::to_vec(&json!({"service":base.as_str(),"profile":profile,"token":token}))
            .unwrap(),
        true,
    )?;
    write_token(&path(app, profile), &data)
}

fn take_login_token(result: &mut Value) -> Result<String, String> {
    result
        .get_mut("token")
        .map(Value::take)
        .and_then(|v| v.as_str().map(String::from))
        .filter(|token| {
            (32..=128).contains(&token.len())
                && token.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        })
        .ok_or_else(|| "Kontotjenesten returnerte en ugyldig innlogging. Prøv igjen.".into())
}

fn remove_token(app: &AppHandle, profile: &str) -> Result<(), String> {
    match std::fs::remove_file(path(app, profile)) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err("Kunne ikke fjerne lokal innlogging".into()),
    }
}

pub async fn request(
    app: AppHandle,
    profile: String,
    operation: String,
    payload: Value,
) -> Result<Value, String> {
    let state = app.state::<AppState>();
    if !crate::security::valid_profile(&profile) || *state.profile.lock().unwrap() != profile {
        return Err("Profilen ble endret. Prøv på nytt.".into());
    }
    if operation == "status" && !configured() {
        return Ok(json!({"configured":false,"account":null}));
    }
    let base = base()?;
    let saved = token(&app, &profile, &base)?;
    let (method, endpoint) = match operation.as_str() {
        "status" if saved.is_none() => return Ok(json!({"configured":true,"account":null})),
        "status" => (reqwest::Method::GET, "v1/account"),
        "email_start" => (reqwest::Method::POST, "v1/auth/email/start"),
        "email_verify" => (reqwest::Method::POST, "v1/auth/email/verify"),
        "logout" => (reqwest::Method::POST, "v1/auth/logout"),
        "sync_get" => (reqwest::Method::GET, "v1/sync"),
        "sync_put" => (reqwest::Method::PUT, "v1/sync"),
        "sync_delete" => (reqwest::Method::DELETE, "v1/sync"),
        _ => return Err("Ukjent kontohandling".into()),
    };
    if payload.to_string().len() > 5_500_000 {
        return Err("For mye synkroniseringsdata".into());
    }
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|_| "Kunne ikke kontakte kontotjenesten")?;
    let mut request = client.request(method, base.join(endpoint).unwrap());
    if let Some(token) = saved {
        request = request.bearer_auth(token);
    }
    let mut response = request
        .json(&payload)
        .send()
        .await
        .map_err(|_| "Kontotjenesten svarte ikke. Prøv igjen.")?;
    let status = response.status();
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "Ufullstendig svar fra kontotjenesten")?
    {
        if bytes.len() + chunk.len() > 5_500_000 {
            return Err("For stort svar fra kontotjenesten".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    let mut result: Value =
        serde_json::from_slice(&bytes).map_err(|_| "Ugyldig svar fra kontotjenesten")?;
    // Keep profile selection stable while replacing/removing this profile's credential.
    // No network await occurs while this lock is held.
    let current_profile = state.profile.lock().unwrap();
    if *current_profile != profile {
        return Err("Profilen ble endret. Prøv innloggingen igjen i riktig profil.".into());
    }
    if !status.is_success() {
        if status.as_u16() == 401
            && matches!(operation.as_str(), "status" | "logout" | "sync_get" | "sync_put" | "sync_delete")
        {
            remove_token(&app, &profile)?;
            if operation == "status" || operation == "logout" {
                return Ok(json!({"configured":true,"account":null}));
            }
            return Err("Kontosesjonen er utløpt. Logg inn på nytt.".into());
        }
        return Err(result["error"]
            .as_str()
            .unwrap_or("Kontohandlingen mislyktes")
            .chars()
            .take(200)
            .collect());
    }
    if operation == "email_verify" {
        let token = take_login_token(&mut result)?;
        save_token(&app, &profile, &base, &token)?;
    }
    if operation == "logout" {
        remove_token(&app, &profile)?;
    }
    result
        .as_object_mut()
        .ok_or("Ugyldig kontosvar")?
        .insert("configured".into(), json!(true));
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_login_requires_a_valid_token_and_never_exposes_it_to_the_shell() {
        let token = "a".repeat(43);
        let mut result = json!({"token":token,"account":{"name":"person@example.com"}});
        assert_eq!(take_login_token(&mut result).unwrap(), token);
        assert!(result["token"].is_null());
        assert_eq!(result["account"]["name"], "person@example.com");
        for value in [Value::Null, json!("short"), json!("x".repeat(129)), json!(" ".repeat(43))] {
            let mut result = json!({"token":value});
            assert!(take_login_token(&mut result).is_err());
            assert!(result["token"].is_null());
        }
        assert!(take_login_token(&mut json!({"account":{}})).is_err());
    }

    #[test]
    fn atomic_credential_save_replaces_existing_file_and_keeps_profile_data() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("account.dat");
        let profile_data = directory.path().join("profile-data");
        std::fs::write(&profile_data, b"keep me").unwrap();
        write_token(&path, b"old credential").unwrap();
        write_token(&path, b"new credential").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"new credential");
        assert_eq!(std::fs::read(profile_data).unwrap(), b"keep me");
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 2);
    }

    #[test]
    fn invalid_credentials_allow_a_fresh_login() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("account.dat");
        let base = url::Url::parse("https://accounts.example/").unwrap();
        for bytes in [Vec::new(), b"truncated DPAPI data".to_vec(), vec![0; 16001]] {
            std::fs::write(&path, bytes).unwrap();
            assert!(read_token(&path, "profile", &base).unwrap().is_none());
        }
    }

    #[cfg(windows)]
    #[test]
    fn protected_credential_can_replace_corrupt_store() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("account.dat");
        let base = url::Url::parse("https://accounts.example/").unwrap();
        std::fs::write(&path, b"corrupt").unwrap();
        assert!(read_token(&path, "profile", &base).unwrap().is_none());
        let token = "a".repeat(43);
        let data =
            serde_json::to_vec(&json!({"service":base.as_str(),"profile":"profile","token":token}))
                .unwrap();
        write_token(&path, &protect(&data, true).unwrap()).unwrap();
        assert_eq!(read_token(&path, "profile", &base).unwrap(), Some(token));
        assert!(read_token(&path, "other-profile", &base).unwrap().is_none());
    }
}
