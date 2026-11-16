//! Disposable Windows CI only. No test entrypoint exists in production builds.
use crate::{storage::Settings, AppState};
use tauri::{AppHandle, Manager};
use tauri_plugin_updater::UpdaterExt;
fn stage(app: &AppHandle, value: &str) {
    use std::io::Write;
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(app.state::<AppState>().root.join("progress.log"))
    {
        let _ = writeln!(file, "{} {value}", chrono::Utc::now());
    }
}

pub fn start(app: AppHandle) {
    std::thread::spawn(move || {
        stage(
            &app,
            "Application started; waiting for shell initialization",
        );
        // Let the shell finish its normal initial update check first.
        std::thread::sleep(std::time::Duration::from_secs(12));
        let result = tauri::async_runtime::block_on(exercise(&app));
        let (code, message) = match result {
            Ok(()) => (0, "PASS: signed 0.1.0 -> 0.1.1 install; profiles, bookmarks, history, settings, cookies and profile files preserved; tampered/replayed artifacts rejected".to_string()),
            Err(e) => (1, format!("FAIL: {e}")),
        };
        let root = &app.state::<AppState>().root;
        let _ = std::fs::write(root.join("result.txt"), message);
        app.exit(code);
    });
}

async fn exercise(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    stage(app, "Beginning version/data verification");
    let version = app.package_info().version.to_string();
    if version == "0.1.1" {
        {
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
                    || std::fs::read_to_string(
                        crate::profiles::directory(&state.root, &p.id)?
                            .join("elix-preserve-test.txt"),
                    )
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
        }
        stage(app, "Checking persisted cookies after upgrade");
        check_cookies(app, false).await?;
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
            let dir = crate::profiles::directory(&state.root, &p.id)?;
            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            std::fs::write(dir.join("elix-preserve-test.txt"), &p.name)
                .map_err(|e| e.to_string())?;
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
    stage(app, "Seeding real profile cookies");
    check_cookies(app, true).await?;
    let base = option_env!("ELIX_UPDATE_BASE").ok_or("Missing HTTPS fixture")?;
    if base != "https://127.0.0.1:44335" {
        return Err("Upgrade self-test requires loopback fixture".into());
    }
    // Real Tauri verification must reject both modified bytes and a valid OLD signature
    // whose unsigned metadata lies about the version. No installer is called here.
    for case in ["corrupt", "replay"] {
        stage(app, &format!("Rejecting {case} artifact"));
        let candidate = app
            .updater_builder()
            .pubkey(option_env!("ELIX_UPDATE_PUBLIC_KEY").unwrap_or_default())
            .endpoints(vec![
                url::Url::parse(&format!("{base}/{case}.json")).unwrap()
            ])
            .map_err(|e| e.to_string())?
            .timeout(std::time::Duration::from_secs(20))
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
    stage(app, "Checking real signed update");
    let offered = crate::updater::check(app).await?;
    if offered["version"] != "0.1.1" {
        return Err("0.1.1 not detected".into());
    }
    stage(app, "Downloading and verifying signed update");
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
    stage(app, "Starting signed NSIS installation");
    crate::updater::install(app)?;
    Err("Installer unexpectedly returned without restarting".into())
}

async fn check_cookies(app: &AppHandle, seed: bool) -> Result<(), String> {
    let profiles = app.state::<AppState>().store.lock().unwrap().profiles()?;
    for p in profiles {
        let shell = app.get_webview("shell").ok_or("Missing shell")?;
        crate::dispatch(
            app.clone(),
            shell,
            crate::Request::SwitchProfile { id: p.id.clone() },
        )
        .await?;
        let url = if seed {
            format!("https://127.0.0.1:44335/cookie.html?seed={}", p.id)
        } else {
            "https://127.0.0.1:44335/cookie.html".to_owned()
        };
        let id = crate::browser::create(app.clone(), url).await?;
        let until = std::time::Instant::now() + std::time::Duration::from_secs(25);
        loop {
            let found = app
                .state::<AppState>()
                .session
                .lock()
                .unwrap()
                .tabs
                .iter()
                .any(|t| t.id == id && t.title == format!("cookie:{}", p.id) && !t.loading);
            if found {
                break;
            }
            if std::time::Instant::now() > until {
                return Err(format!(
                    "Cookie persistence/isolation failed for profile {}",
                    p.name
                ));
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        crate::browser::close(app, &id)?;
    }
    // Allow the real browser process to flush persistent cookies before installation exits it.
    std::thread::sleep(std::time::Duration::from_secs(2));
    Ok(())
}
