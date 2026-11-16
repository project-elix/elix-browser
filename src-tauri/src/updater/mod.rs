use crate::AppState;
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};
#[derive(Default)]
pub struct Updates {
    pub pending: Mutex<Option<Update>>,
    pub bytes: Mutex<Option<Vec<u8>>>,
    pub busy: AtomicBool,
}
struct Busy<'a>(&'a AtomicBool);
impl Drop for Busy<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}
pub fn configured() -> bool {
    option_env!("ELIX_UPDATE_BASE").is_some_and(|s| s.starts_with("https://"))
        && option_env!("ELIX_UPDATE_PUBLIC_KEY").is_some_and(|s| !s.is_empty())
}
pub fn channel_version(channel: &str, version: &semver::Version) -> bool {
    match channel {
        "stable" => version.pre.is_empty(),
        "beta" => version.pre.is_empty() || version.pre.as_str().starts_with("beta."),
        "developer" => true,
        _ => false,
    }
}
pub async fn check(app: &AppHandle) -> Result<Value, String> {
    if !configured() {
        return Ok(
            json!({"state":"unconfigured","message":"Denne utviklingsbyggingen har ingen aktivert oppdateringsserver."}),
        );
    }
    let state = app.state::<AppState>();
    if state.updates.busy.swap(true, Ordering::SeqCst) {
        return Err("En oppdateringsoperasjon pågår".into());
    }
    let _guard = Busy(&state.updates.busy);
    let channel = state.store.lock().unwrap().settings().channel;
    let endpoint = format!(
        "{}/{channel}.json",
        option_env!("ELIX_UPDATE_BASE")
            .unwrap_or_default()
            .trim_end_matches('/')
    );
    let url = url::Url::parse(&endpoint).map_err(|_| "Ugyldig oppdateringsadresse")?;
    if url.scheme() != "https" {
        return Err("Oppdateringer krever HTTPS".into());
    }
    state.logs.add("info", "update_check_started");
    let result = app
        .updater_builder()
        .pubkey(option_env!("ELIX_UPDATE_PUBLIC_KEY").unwrap_or_default())
        .endpoints(vec![url])
        .map_err(|e| e.to_string())?
        .timeout(std::time::Duration::from_secs(20))
        // Enforce HTTPS on redirected requests as well as the initial URLs.
        .configure_client(|client| client.https_only(true))
        .build()
        .map_err(|e| e.to_string())?
        .check()
        .await;
    let update = match result {
        Ok(v) => v,
        Err(_) => {
            state.logs.add("warn", "update_check_failed");
            return Err("Kunne ikke sjekke oppdateringer. Prøv igjen senere.".into());
        }
    };
    *state.updates.bytes.lock().unwrap() = None;
    if let Some(update) = update {
        let version = semver::Version::parse(&update.version).map_err(|_| "Ugyldig versjon")?;
        if !channel_version(&channel, &version) {
            return Err("Versjonen passer ikke valgt kanal".into());
        }
        if update.download_url.scheme() != "https" || update.signature.trim().is_empty() {
            return Err("Usikker eller usignert oppdatering ble avvist".into());
        }
        let result = json!({"state":"available","version":update.version,"notes":update.body,"date":update.date.map(|d|d.to_string())});
        *state.updates.pending.lock().unwrap() = Some(update);
        state.logs.add("info", "update_available");
        Ok(result)
    } else {
        *state.updates.pending.lock().unwrap() = None;
        Ok(json!({"state":"current"}))
    }
}
pub async fn download(app: &AppHandle) -> Result<Value, String> {
    let state = app.state::<AppState>();
    if state.updates.busy.swap(true, Ordering::SeqCst) {
        return Err("En oppdateringsoperasjon pågår".into());
    }
    let _guard = Busy(&state.updates.busy);
    let update = state
        .updates
        .pending
        .lock()
        .unwrap()
        .clone()
        .ok_or("Sjekk etter oppdateringer først")?;
    let mut received = 0u64;
    let result = update
        .download(
            |size, total| {
                received += size as u64;
                let _ = app.emit_to(
                    "shell",
                    "update-progress",
                    json!({"received":received,"total":total}),
                );
            },
            || {},
        )
        .await;
    match result {
        Ok(bytes) => {
            *state.updates.bytes.lock().unwrap() = Some(bytes);
            state.logs.add("info", "update_signature_verified");
            Ok(json!({"state":"ready","version":update.version}))
        }
        Err(_) => {
            state
                .logs
                .add("error", "update_download_or_signature_failed");
            *state.updates.bytes.lock().unwrap() = None;
            Err(
                "Nedlasting eller signaturkontroll mislyktes. Ingen oppdatering er installert."
                    .into(),
            )
        }
    }
}
pub fn install(app: &AppHandle) -> Result<Value, String> {
    let state = app.state::<AppState>();
    if state.updates.busy.swap(true, Ordering::SeqCst) {
        return Err("En oppdateringsoperasjon pågår".into());
    }
    let _guard = Busy(&state.updates.busy);
    let update = state
        .updates
        .pending
        .lock()
        .unwrap()
        .clone()
        .ok_or("Ingen oppdatering")?;
    let bytes = state
        .updates
        .bytes
        .lock()
        .unwrap()
        .take()
        .ok_or("Last ned og verifiser oppdateringen først")?;
    crate::browser::persist(app)?;
    state.logs.add("info", "update_install_started");
    update
        .install(bytes)
        .map_err(|_| "Installasjonen mislyktes. Last ned på nytt og prøv igjen.".to_string())?;
    Ok(json!({"state":"installed"}))
}
