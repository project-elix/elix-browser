//! Only compiled into CI/development test builds with the `smoke-tests` feature.
//! Exercises the actual native renderer without exposing test IPC to websites.
use crate::{browser, AppState};
use std::{
    io::{Read, Write},
    time::{Duration, Instant},
};
use tauri::{AppHandle, Manager};
fn wait(mut condition: impl FnMut() -> bool) -> Result<(), String> {
    let until = Instant::now() + Duration::from_secs(25);
    while Instant::now() < until {
        if condition() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Err("Native test condition timed out".into())
}
fn title(app: &AppHandle, id: &str, expected: &str) -> bool {
    app.state::<AppState>()
        .session
        .lock()
        .unwrap()
        .tabs
        .iter()
        .any(|t| t.id == id && t.title == expected && !t.loading)
}
pub fn start(app: AppHandle) {
    std::thread::spawn(move || {
        let result = exercise(&app);
        let code = if result.is_ok() { 0 } else { 1 };
        let message=match result{Ok(())=>"PASS: native HTTP navigation, title/status, tab switching, back/forward, DevTools, download bytes/failure, suspension, profile storage isolation, active-session restoration, remote IPC denied".to_owned(),Err(e)=>format!("FAIL: {e}")};
        println!("{message}");
        if let Some(path) = std::env::var_os("ELIX_SMOKE_RESULT") {
            let _ = std::fs::write(path, &message);
        }
        app.exit(code);
    });
}
fn exercise(app: &AppHandle) -> Result<(), String> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    let origin = format!(
        "http://{}",
        listener.local_addr().map_err(|e| e.to_string())?
    );
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
            let mut request = [0; 8192];
            let size = stream.read(&mut request).unwrap_or(0);
            let request = String::from_utf8_lossy(&request[..size]);
            if request.starts_with("GET /broken ") {
                let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 1000000\r\nContent-Type: application/octet-stream\r\nContent-Disposition: attachment; filename=broken.txt\r\nConnection: close\r\n\r\npartial");
                continue;
            }
            let download = request.starts_with("GET /download ");
            let body = if download {
                "elix-native-download".to_owned()
            } else {
                format!("<!doctype html><html><head><title>{}</title></head><body>Native WebView2 fixture</body></html>",if request.starts_with("GET /two "){"ELIX native two"}else{"ELIX native one"})
            };
            let extra = if download {
                "Content-Type: application/octet-stream\r\nContent-Disposition: attachment; filename=elix-smoke.txt\r\n"
            } else {
                "Content-Type: text/html; charset=utf-8\r\n"
            };
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n{}Connection: close\r\n\r\n{}",
                body.len(),
                extra,
                body
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });
    std::thread::sleep(Duration::from_secs(2));
    let first =
        tauri::async_runtime::block_on(browser::create(app.clone(), format!("{origin}/one")))?;
    wait(|| title(app, &first, "ELIX native one"))?;
    let view = app.get_webview(&first).ok_or("Missing native view")?;
    view.reload().map_err(|e| e.to_string())?;
    wait(|| {
        app.state::<AppState>()
            .session
            .lock()
            .unwrap()
            .tabs
            .iter()
            .any(|t| t.id == first && t.http_status == Some(200) && t.load_ms.is_some())
    })?;
    browser::navigate(app, &first, &format!("{origin}/two"))?;
    wait(|| title(app, &first, "ELIX native two"))?;
    tauri::async_runtime::block_on(browser::engine::action(view.clone(), "back".into(), false))?;
    wait(|| title(app, &first, "ELIX native one"))?;
    tauri::async_runtime::block_on(browser::engine::action(
        view.clone(),
        "forward".into(),
        false,
    ))?;
    wait(|| title(app, &first, "ELIX native two"))?;
    tauri::async_runtime::block_on(browser::engine::action(
        view.clone(),
        "set_developer".into(),
        true,
    ))?;
    // Wry cannot query/close DevTools on Windows. Verify native COM acceptance; visual QA is separate.
    tauri::async_runtime::block_on(browser::engine::action(
        view.clone(),
        "devtools".into(),
        true,
    ))?;
    view.eval("(async()=>{if(!window.__TAURI_INTERNALS__?.invoke){document.title='ELIX IPC denied';return}try{await window.__TAURI_INTERNALS__.invoke('dispatch',{request:{action:'snapshot'}});document.title='ELIX IPC allowed'}catch{document.title='ELIX IPC denied'}})()")
        .map_err(|e| e.to_string())?;
    wait(|| title(app, &first, "ELIX IPC denied"))?;
    let second =
        tauri::async_runtime::block_on(browser::create(app.clone(), format!("{origin}/one")))?;
    wait(|| title(app, &second, "ELIX native one"))?;
    browser::navigate(app, &second, &format!("{origin}/download"))?;
    wait(|| {
        app.state::<AppState>()
            .downloads
            .lock()
            .unwrap()
            .iter()
            .any(|d| d.status == "complete")
    })?;
    let path = app
        .state::<AppState>()
        .downloads
        .lock()
        .unwrap()
        .iter()
        .find(|d| d.status == "complete")
        .unwrap()
        .path
        .clone();
    if std::fs::read_to_string(&path).map_err(|e| e.to_string())? != "elix-native-download" {
        return Err("Downloaded bytes differ".into());
    }
    browser::navigate(app, &second, &format!("{origin}/broken"))?;
    wait(|| {
        app.state::<AppState>()
            .downloads
            .lock()
            .unwrap()
            .iter()
            .any(|d| d.status == "failed")
    })?;
    browser::activate(app, &first)?;
    browser::suspend(app, &second)?;
    if app.get_webview(&second).is_some() {
        return Err("Suspended view remains alive".into());
    }
    {
        let state = app.state::<AppState>();
        let p = state.profile.lock().unwrap().clone();
        let store = state.store.lock().unwrap();
        store.bookmark(&p, &format!("{origin}/one"), "Fixture")?;
        let other = store.create_profile("Isolated test")?;
        if !store.entries(&other.id, "bookmarks")?.is_empty() {
            return Err("Profile bookmark leak".into());
        }
        if store.entries(&p, "bookmarks")?.is_empty() {
            return Err("Bookmark not persisted".into());
        }
    }
    browser::persist(app)?;
    browser::close(app, &second)?;
    for (label, view) in app.webviews() {
        if label.starts_with("tab-") {
            view.close().map_err(|e| e.to_string())?;
        }
    }
    browser::restore(app);
    let state = app.state::<AppState>();
    let session = state.session.lock().unwrap();
    if !session
        .tabs
        .iter()
        .any(|t| t.id == session.active && t.url == format!("{origin}/two"))
    {
        return Err("Active tab was not restored".into());
    }
    Ok(())
}
