//! Only compiled into CI/development test builds with the `smoke-tests` feature.
//! Exercises the actual native renderer without exposing test IPC to websites.
use crate::{browser, AppState};
use std::{
    io::{Read, Write},
    time::{Duration, Instant},
};
use tauri::{AppHandle, Manager};
#[track_caller]
fn wait(mut condition: impl FnMut() -> bool) -> Result<(), String> {
    let caller = std::panic::Location::caller();
    let until = Instant::now() + Duration::from_secs(25);
    while Instant::now() < until {
        if condition() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Err(format!(
        "Native test condition timed out at {}:{}",
        caller.file(),
        caller.line()
    ))
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
fn shell_bool(app: &AppHandle, script: &str) -> Result<bool, String> {
    let shell = app.get_webview("shell").ok_or("Missing shell")?;
    let (tx, rx) = std::sync::mpsc::channel();
    shell
        .eval_with_callback(script, move |value| {
            let _ = tx.send(value);
        })
        .map_err(|e| e.to_string())?;
    let value = rx
        .recv_timeout(Duration::from_secs(5))
        .map_err(|e| e.to_string())?;
    Ok(value == "true")
}
fn sidebar_test(app: &AppHandle, remote: &str) -> Result<(), String> {
    let shell = app.get_webview("shell").ok_or("Missing shell")?;
    wait(|| {
        shell_bool(
            app,
            "!!document.querySelector('[aria-label=\"Utvid sidemeny\"]')",
        )
        .unwrap_or(false)
    })?;
    shell
        .eval("document.querySelector('[aria-label=\"Utvid sidemeny\"]').click()")
        .map_err(|e| e.to_string())?;
    wait(|| {
        app.state::<AppState>()
            .store
            .lock()
            .unwrap()
            .settings()
            .appearance
            .sidebar
            == "expanded"
    })?;
    wait(|| app.state::<AppState>().bounds.lock().unwrap().x >= 160.)?;
    shell
        .eval("document.querySelector('[aria-label=\"Vis eller skjul sidemeny\"]').click()")
        .map_err(|e| e.to_string())?;
    wait(|| {
        app.state::<AppState>()
            .store
            .lock()
            .unwrap()
            .settings()
            .appearance
            .sidebar
            == "hidden"
    })?;
    wait(|| app.state::<AppState>().bounds.lock().unwrap().x < 1.)?;
    shell
        .eval("document.querySelector('[aria-label=\"Vis eller skjul sidemeny\"]').click()")
        .map_err(|e| e.to_string())?;
    wait(|| {
        app.state::<AppState>()
            .store
            .lock()
            .unwrap()
            .settings()
            .appearance
            .sidebar
            == "icons"
    })?;
    wait(|| {
        let x = app.state::<AppState>().bounds.lock().unwrap().x;
        x >= 50. && x <= 65.
    })?;
    shell
        .eval("document.querySelector('.rail [aria-label=\"Innstillinger\"]').click()")
        .map_err(|e| e.to_string())?;
    wait(|| shell_bool(app, "!!document.querySelector('.settings-nav')").unwrap_or(false))?;
    shell.eval("Array.from(document.querySelectorAll('.settings-nav button')).find(b=>b.textContent==='Oppdateringer').click()").map_err(|e| e.to_string())?;
    wait(|| {
        shell_bool(app, "Array.from(document.querySelectorAll('.settings-content h2')).some(h=>h.textContent==='Oppdateringer' && h.getClientRects().length>0) && document.querySelector('.appearance-settings').getClientRects().length===0").unwrap_or(false)
    })?;
    browser::navigate(app, remote, "elix://start")?;
    Ok(())
}
pub fn start(app: AppHandle) {
    std::thread::spawn(move || {
        let result = exercise(&app);
        let code = if result.is_ok() { 0 } else { 1 };
        let message=match result{Ok(())=>"PASS: sidebar expand/hide/restore, native content bounds and settings categories; native minimize/maximize/restore/fullscreen; native HTTP navigation, title/status, tab switching, back/forward, DevTools, download bytes/failure, suspension, profile storage isolation, active-session restoration, remote IPC denied".to_owned(),Err(e)=>format!("FAIL: {e}\n{}", serde_json::json!({"tabs":app.state::<AppState>().session.lock().unwrap().clone(),"downloads":app.state::<AppState>().downloads.lock().unwrap().clone(),"logs":app.state::<AppState>().logs.list()}))};
        println!("{message}");
        if let Some(path) = std::env::var_os("ELIX_SMOKE_RESULT") {
            let _ = std::fs::write(path, &message);
        }
        if code == 0 {
            // Exercise the same persisted close path as the visible close button.
            if crate::window::perform(&app, crate::window::Action::Close).is_err() {
                app.exit(1);
            }
        } else {
            app.exit(code);
        }
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
                let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 1000000\r\nContent-Type: application/octet-stream\r\nContent-Disposition: attachment; filename=broken.txt\r\nConnection: close\r\n\r\n");
                // Commit the download before disconnecting mid-transfer. An already
                // truncated response can be retried as a failed navigation instead.
                let _ = stream.flush();
                std::thread::sleep(Duration::from_millis(500));
                let _ = stream.write_all(b"partial");
                let _ = stream.flush();
                std::thread::sleep(Duration::from_millis(500));
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
    let main = app.get_window("main").ok_or("Missing main window")?;
    crate::window::perform(app, crate::window::Action::Minimize)?;
    wait(|| main.is_minimized().unwrap_or(false))?;
    main.unminimize().map_err(|e| e.to_string())?;
    wait(|| !main.is_minimized().unwrap_or(true))?;
    main.unmaximize().map_err(|e| e.to_string())?;
    wait(|| !main.is_maximized().unwrap_or(true))?;
    crate::window::perform(app, crate::window::Action::ToggleMaximize)?;
    wait(|| main.is_maximized().unwrap_or(false))?;
    crate::window::perform(app, crate::window::Action::ToggleMaximize)?;
    wait(|| !main.is_maximized().unwrap_or(true))?;
    crate::window::perform(app, crate::window::Action::ToggleFullscreen)?;
    wait(|| main.is_fullscreen().unwrap_or(false))?;
    crate::window::perform(app, crate::window::Action::ExitFullscreen)?;
    wait(|| !main.is_fullscreen().unwrap_or(true))?;
    main.maximize().map_err(|e| e.to_string())?;
    let first =
        tauri::async_runtime::block_on(browser::create(app.clone(), format!("{origin}/one")))?;
    wait(|| title(app, &first, "ELIX native one"))?;
    sidebar_test(app, &first)?;
    browser::navigate(app, &first, &format!("{origin}/one"))?;
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
    if crate::authorize(&view).is_ok() {
        return Err("Remote native view passed IPC authorization".into());
    }
    // An inaccessible IPC transport may never settle its Promise. Distinguish that
    // from an accepted call; the backend authorization is independently asserted above.
    view.eval("(async()=>{document.title='ELIX IPC pending';if(!window.__TAURI_INTERNALS__?.invoke){document.title='ELIX IPC denied';return}const result=await Promise.race([window.__TAURI_INTERNALS__.invoke('dispatch',{request:{action:'snapshot'}}).then(()=> 'allowed',()=> 'denied'),new Promise(resolve=>setTimeout(()=>resolve('unreachable'),2000))]);document.title='ELIX IPC '+result})()")
        .map_err(|e| e.to_string())?;
    wait(|| title(app, &first, "ELIX IPC denied") || title(app, &first, "ELIX IPC unreachable"))?;
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
