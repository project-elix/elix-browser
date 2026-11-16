//! Disposable Windows CI only. No test entrypoint exists in production builds.
use crate::{storage::Settings, AppState};
use tauri::{AppHandle, Manager};
use tauri_plugin_updater::UpdaterExt;

pub fn start(app: AppHandle) {
    std::thread::spawn(move || {
        // Let the shell finish its normal initial update check first.
        std::thread::sleep(std::time::Duration::from_secs(8));
        let result = tauri::async_runtime::block_on(exercise(&app));
        let (code, message) = match result {
            Ok(()) => (0, "PASS: signed 0.1.0 -> 0.1.1 install; profiles, bookmarks, history, settings and profile files preserved; tampered/replayed artifacts rejected".to_string()),
            Err(e) => (1, format!("FAIL: {e}")),
        };
        let root = &app.state::<AppState>().root;
        let _ = std::fs::write(root.join("result.txt"), message);
        app.exit(code);
    });
}

async fn exercise(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let version = app.package_info().version.to_string();
    if version == "0.1.1" {
        let store = state.store.lock().unwrap();
        let profiles = store.profiles()?;
        if profiles.len() != 2 || !store.settings().gaming || !store.settings().developer {
            return Err("Profiles/preferences did not survive upgrade".into());
        }
        for p in &profiles {
            if store
                .entries(&p.id, "bookmarks")?
                .iter()
                .all(|e| e.title != p.name)
                || store
                    .entries(&p.id, "history")?
                    .iter()
                    .all(|e| e.title != p.name)
                || std::fs::read_to_string(state.root.join(&p.id).join("preserve.txt"))
                    .ok()
                    .as_deref()
                    != Some(&p.name)
            {
                return Err("Profile data missing after upgrade".into());
            }
        }
        if store.get("upgrade_seed_version").as_deref() != Some("0.1.0") {
            return Err("Missing original version marker".into());
        }
        return Ok(());
    }
    if version != "0.1.0" {
        return Err(format!("Unexpected test version {version}"));
    }
    {
        let store = state.store.lock().unwrap();
        if store.get("upgrade_seed_version").is_some() {
            return Err("Old executable restarted instead of upgraded executable".into());
        }
        store.create_profile("Upgrade secondary")?;
        for p in store.profiles()? {
            store.bookmark(&p.id, "https://example.com/upgrade", &p.name)?;
            store.visit(&p.id, "https://example.com/upgrade", &p.name)?;
            let dir = state.root.join(&p.id);
            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            std::fs::write(dir.join("preserve.txt"), &p.name).map_err(|e| e.to_string())?;
        }
        store.set(
            "preferences",
            &serde_json::to_string(&Settings {
                developer: true,
                gaming: true,
                ..Default::default()
            })
            .unwrap(),
        )?;
        store.set("upgrade_seed_version", "0.1.0")?;
    }
    let base = option_env!("ELIX_UPDATE_BASE").ok_or("Missing HTTPS fixture")?;
    if base != "https://127.0.0.1:44335" {
        return Err("Upgrade self-test requires loopback fixture".into());
    }
    // Real Tauri verification must reject both modified bytes and a valid OLD signature
    // whose unsigned metadata lies about the version. No installer is called here.
    for case in ["corrupt", "replay"] {
        let candidate = app
            .updater_builder()
            .pubkey(option_env!("ELIX_UPDATE_PUBLIC_KEY").unwrap_or_default())
            .endpoints(vec![
                url::Url::parse(&format!("{base}/{case}.json")).unwrap()
            ])
            .map_err(|e| e.to_string())?
            .build()
            .map_err(|e| e.to_string())?
            .check()
            .await
            .map_err(|e| format!("{case} manifest: {e}"))?
            .ok_or(format!("{case} fixture not offered"))?;
        if candidate.download(|_, _| {}, || {}).await.is_ok() {
            return Err(format!("Security failure: {case} artifact accepted"));
        }
    }
    let offered = crate::updater::check(app).await?;
    if offered["version"] != "0.1.1" {
        return Err("0.1.1 not detected".into());
    }
    let ready = crate::updater::download(app).await?;
    if ready["state"] != "ready" {
        return Err("Signed download not ready".into());
    }
    std::fs::write(
        state.root.join("verified.txt"),
        "0.1.1 signature verified; corrupt/replay rejected",
    )
    .map_err(|e| e.to_string())?;
    // The real NSIS updater exits this process and restarts with the original argument.
    crate::updater::install(app)?;
    Err("Installer unexpectedly returned without restarting".into())
}
