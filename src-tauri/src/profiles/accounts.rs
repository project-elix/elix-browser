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
    let path = path(app, profile);
    if !path.exists() {
        return Ok(None);
    }
    let bytes = std::fs::read(path).map_err(|_| "Kunne ikke lese kontoen")?;
    if bytes.len() > 16000 {
        return Err("Ugyldig kontolagring".into());
    }
    let value: Value =
        serde_json::from_slice(&protect(&bytes, false)?).map_err(|_| "Ugyldig kontolagring")?;
    if value["service"] != base.as_str() || value["profile"] != profile {
        return Ok(None);
    }
    Ok(value["token"].as_str().map(String::from))
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
    let path = path(app, profile);
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|_| "Kunne ikke lagre kontoen")?;
    std::fs::write(path, data).map_err(|_| "Kunne ikke lagre kontoen".into())
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
        "discord_start" => (reqwest::Method::POST, "v1/auth/discord/start"),
        "discord_complete" => (reqwest::Method::POST, "v1/auth/discord/complete"),
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
    if !status.is_success() {
        if status.as_u16() == 401 && operation == "status" {
            let _ = std::fs::remove_file(path(&app, &profile));
            return Ok(json!({"configured":true,"account":null}));
        }
        return Err(result["error"]
            .as_str()
            .unwrap_or("Kontohandlingen mislyktes")
            .chars()
            .take(200)
            .collect());
    }
    if *state.profile.lock().unwrap() != profile {
        return Err("Profilen ble endret. Prøv innloggingen igjen i riktig profil.".into());
    }
    if matches!(operation.as_str(), "email_verify" | "discord_complete") {
        if let Some(token) = result
            .get_mut("token")
            .map(Value::take)
            .and_then(|v| v.as_str().map(String::from))
        {
            save_token(&app, &profile, &base, &token)?;
        }
    }
    if operation == "logout" {
        let p = path(&app, &profile);
        if p.exists() {
            std::fs::remove_file(p).map_err(|_| "Kunne ikke fjerne lokal innlogging")?;
        }
    }
    if operation == "discord_start" {
        let target = result["authorize_url"]
            .as_str()
            .ok_or("Mangler Discord-adresse")?;
        let url = url::Url::parse(target).map_err(|_| "Ugyldig Discord-adresse")?;
        if url.origin() != base.origin() || url.path() != "/v1/auth/discord/authorize" {
            return Err("Ugyldig Discord-adresse".into());
        }
        crate::browser::create(app.clone(), url.to_string()).await?;
    }
    result
        .as_object_mut()
        .ok_or("Ugyldig kontosvar")?
        .insert("configured".into(), json!(true));
    Ok(result)
}
