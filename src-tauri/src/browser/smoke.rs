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
fn wait_title_or_error(app: &AppHandle, id: &str, expected: &str) -> Result<(), String> {
    wait(|| {
        app.state::<AppState>()
            .session
            .lock()
            .unwrap()
            .tabs
            .iter()
            .any(|tab| {
                tab.id == id
                    && ((!tab.loading && tab.title == expected) || tab.navigation_error.is_some())
            })
    })?;
    let error = app
        .state::<AppState>()
        .session
        .lock()
        .unwrap()
        .tabs
        .iter()
        .find(|tab| tab.id == id)
        .and_then(|tab| tab.navigation_error.clone());
    if let Some(error) = error {
        return Err(format!("Native navigation failed: {error}"));
    }
    Ok(())
}
fn shell_bool(app: &AppHandle, script: &str) -> Result<bool, String> {
    view_bool(&app.get_webview("shell").ok_or("Missing shell")?, script)
}
fn view_bool(view: &tauri::Webview, script: &str) -> Result<bool, String> {
    let (tx, rx) = std::sync::mpsc::channel();
    view.eval_with_callback(script, move |value| {
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
    for placement in ["sidebar", "toolbar"] {
        shell.eval(format!("(()=>{{const select=document.querySelector('[aria-label=\"Bokmerkenes plassering\"]');select.value='{placement}';select.dispatchEvent(new Event('change',{{bubbles:true}}))}})()")).map_err(|e|e.to_string())?;
        shell
            .eval("document.querySelector('.appearance-actions .primary').click()")
            .map_err(|e| e.to_string())?;
        wait(|| {
            app.state::<AppState>()
                .store
                .lock()
                .unwrap()
                .settings()
                .appearance
                .bookmark_bar
                == placement
        })?;
        let label = if placement == "sidebar" {
            "Bokmerker i sidemenyen"
        } else {
            "Bokmerker under adressefeltet"
        };
        wait(|| {
            shell_bool(
                app,
                &format!(
                    "document.querySelector('[aria-label=\"{label}\"]')?.getClientRects().length>0"
                ),
            )
            .unwrap_or(false)
        })?;
    }

    shell.eval("Array.from(document.querySelectorAll('.settings-nav button')).find(b=>b.textContent==='Oppdateringer').click()").map_err(|e| e.to_string())?;
    wait(|| {
        shell_bool(app, "Array.from(document.querySelectorAll('.settings-content h2')).some(h=>h.textContent==='Oppdateringer' && h.getClientRects().length>0) && document.querySelector('.appearance-settings').getClientRects().length===0").unwrap_or(false)
    })?;
    browser::navigate(app, remote, "elix://start")?;
    if app
        .state::<AppState>()
        .session
        .lock()
        .unwrap()
        .tabs
        .iter()
        .any(|tab| tab.id == remote && tab.loading)
    {
        return Err("Internal page retained a stale loading indicator".into());
    }
    Ok(())
}
pub fn start(app: AppHandle) {
    std::thread::spawn(move || {
        let result = exercise(&app);
        let code = if result.is_ok() { 0 } else { 1 };
        let message=match result{Ok(())=>"PASS: sidebar expand/hide/restore, configurable bookmark placement, native content bounds and settings categories; native minimize/maximize/restore/fullscreen; native Ctrl+H history shortcut; native HTTP navigation, title/status/favicon and failure/recovery loading state, tab switching, back/forward, DevTools, private cookie/history/session isolation, microphone prompt/denial, native extension lifecycle/password preferences, download pause/resume bytes and persistence, distinct concurrent same-URL downloads, accurate retry-attempt records with no active phantoms after cancellation, suspension, profile storage isolation, pinned active-session restoration, remote IPC denied".to_owned(),Err(e)=>format!("FAIL: {e}\n{}", serde_json::json!({"tabs":app.state::<AppState>().session.lock().unwrap().clone(),"downloads":app.state::<AppState>().downloads.lock().unwrap().clone(),"logs":app.state::<AppState>().logs.list()}))};
        #[cfg(windows)]
        let message = format!(
            "{message}\nDownload native diagnostics: {}",
            crate::downloads::windows::debug_trace()
        );
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
#[cfg(windows)]
fn history_shortcut_test(app: &AppHandle, tab_id: &str) -> Result<(), String> {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
        VIRTUAL_KEY, VK_CONTROL,
    };
    let view = app.get_webview(tab_id).ok_or("Missing shortcut view")?;
    app.get_window("main")
        .ok_or("Missing main window")?
        .set_focus()
        .map_err(|e| e.to_string())?;
    view.set_focus().map_err(|e| e.to_string())?;
    // Tauri queues focus changes onto the UI thread. Sending input before the
    // native document actually owns focus races the dispatcher on CI machines.
    wait(|| view_bool(&view, "document.hasFocus()").unwrap_or(false))?;
    wait(|| {
        shell_bool(app, "document.querySelector('[role=tab][aria-selected=true]')?.getAttribute('aria-label')==='ELIX native one'").unwrap_or(false)
    })?;
    // Use native keyboard input, holding the modifier until the actual shortcut
    // is acknowledged. Releasing Ctrl in the same input batch can happen before
    // WebView2 marshals its accelerator event back from the renderer process.
    fn key(value: VIRTUAL_KEY, up: bool) -> INPUT {
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: value,
                    dwFlags: if up {
                        KEYEVENTF_KEYUP
                    } else {
                        KEYBD_EVENT_FLAGS(0)
                    },
                    ..Default::default()
                },
            },
        }
    }
    struct ReleaseKeys;
    impl Drop for ReleaseKeys {
        fn drop(&mut self) {
            let release = [key(VIRTUAL_KEY(72), true), key(VK_CONTROL, true)];
            unsafe {
                SendInput(&release, std::mem::size_of::<INPUT>() as i32);
            }
        }
    }
    let held_keys = ReleaseKeys;
    let pressed = [key(VK_CONTROL, false), key(VIRTUAL_KEY(72), false)];
    if unsafe { SendInput(&pressed, std::mem::size_of::<INPUT>() as i32) } != pressed.len() as u32 {
        return Err("Could not press native Ctrl+H".into());
    }
    wait(|| {
        app.state::<AppState>()
            .session
            .lock()
            .unwrap()
            .tabs
            .iter()
            .any(|tab| tab.id == tab_id && tab.url == "elix://history")
    })?;
    drop(held_keys);
    wait(|| {
        shell_bool(app, "Array.from(document.querySelectorAll('h1')).some(h=>h.textContent==='Logg.' && h.getClientRects().length>0)").unwrap_or(false)
    })?;
    Ok(())
}
fn private_test(app: &AppHandle, regular_id: &str, origin: &str) -> Result<(), String> {
    let regular = app.get_webview(regular_id).ok_or("Missing regular view")?;
    if !view_bool(&regular, "(()=>{document.cookie='elix_regular=1; Path=/';return document.cookie.includes('elix_regular=1')})()")? {
        return Err("Could not seed regular cookie".into());
    }
    let private_url = format!("{origin}/private-secret");
    let private_id = tauri::async_runtime::block_on(browser::create_with_mode(
        app.clone(),
        private_url.clone(),
        true,
    ))?;
    wait_title_or_error(app, &private_id, "ELIX private fixture")?;
    let private = app.get_webview(&private_id).ok_or("Missing private view")?;
    #[cfg(windows)]
    if !tauri::async_runtime::block_on(browser::private::is_in_private(&private))?
        || tauri::async_runtime::block_on(browser::private::is_in_private(&regular))?
    {
        return Err("WebView2 native InPrivate profile flag is incorrect".into());
    }
    #[cfg(windows)]
    tauri::async_runtime::block_on(browser::extensions::verify_private_preferences(&private))?;
    if !view_bool(&private, "(()=>{if(document.cookie.includes('elix_regular='))return false;document.cookie='elix_private=1; Path=/';return document.cookie.includes('elix_private=1')})()")? {
        return Err("Private profile reused regular cookies or cannot store session cookie".into());
    }
    if !view_bool(
        &regular,
        "document.cookie.includes('elix_regular=1') && !document.cookie.includes('elix_private=')",
    )? {
        return Err("Private cookie leaked into regular profile".into());
    }
    browser::persist(app)?;
    {
        let state = app.state::<AppState>();
        let profile = state.profile.lock().unwrap().clone();
        let store = state.store.lock().unwrap();
        if store
            .entries(&profile, "history")?
            .iter()
            .any(|entry| entry.url == private_url)
        {
            return Err("Private navigation was written to history".into());
        }
        if store
            .load_session(&profile)
            .is_some_and(|saved| saved.contains("private-secret") || saved.contains(&private_id))
        {
            return Err("Private tab was written to session restore".into());
        }
    }
    browser::close(app, &private_id)?;
    let fresh_id =
        tauri::async_runtime::block_on(browser::create_with_mode(app.clone(), private_url, true))?;
    wait_title_or_error(app, &fresh_id, "ELIX private fixture")?;
    let fresh = app
        .get_webview(&fresh_id)
        .ok_or("Missing new private view")?;
    #[cfg(windows)]
    tauri::async_runtime::block_on(browser::extensions::verify_private_preferences(&fresh))?;
    if !view_bool(
        &fresh,
        "!document.cookie.includes('elix_private=') && !document.cookie.includes('elix_regular=')",
    )? {
        return Err("Private cookies survived closing the last private tab".into());
    }
    browser::close(app, &fresh_id)?;
    browser::activate(app, regular_id)?;
    Ok(())
}
fn resumable_download_test(app: &AppHandle, tab_id: &str, origin: &str) -> Result<(), String> {
    browser::navigate(app, tab_id, &format!("{origin}/slow"))?;
    wait(|| {
        app.state::<AppState>()
            .downloads
            .lock()
            .unwrap()
            .iter()
            .any(|d| {
                d.name == "elix-resume.bin" && d.status == "downloading" && d.bytes_received > 0
            })
    })?;
    let id = app
        .state::<AppState>()
        .downloads
        .lock()
        .unwrap()
        .iter()
        .find(|d| d.name == "elix-resume.bin")
        .ok_or("Missing resumable download")?
        .id
        .clone();
    browser::navigate(app, tab_id, "elix://downloads")?;
    let manager_id = app
        .state::<AppState>()
        .session
        .lock()
        .unwrap()
        .active
        .clone();
    if manager_id == tab_id || app.get_webview(tab_id).is_none() {
        return Err("Opening Downloads closed the live transfer's owning WebView".into());
    }
    wait(|| {
        shell_bool(app, "Array.from(document.querySelectorAll('.download-entry')).some(entry=>entry.querySelector('strong')?.textContent==='elix-resume.bin' && Array.from(entry.querySelectorAll('button')).some(button=>button.textContent==='Pause' && !button.disabled))").unwrap_or(false)
    })?;
    app.get_webview("shell").ok_or("Missing shell")?.eval("Array.from(document.querySelectorAll('.download-entry')).find(entry=>entry.querySelector('strong')?.textContent==='elix-resume.bin').querySelector('.download-actions button').click()").map_err(|e|e.to_string())?;
    wait(|| {
        app.state::<AppState>()
            .downloads
            .lock()
            .unwrap()
            .iter()
            .any(|d| d.id == id && d.status == "paused" && d.can_resume)
    })?;
    if browser::suspend(app, tab_id).is_ok() {
        return Err("Suspension accepted a tab owning a paused download".into());
    }
    if browser::close(app, tab_id).is_ok() || app.get_webview(tab_id).is_none() {
        return Err("Closing the owner interrupted a paused download".into());
    }
    wait(|| {
        shell_bool(app, "Array.from(document.querySelectorAll('.download-entry')).some(entry=>entry.querySelector('strong')?.textContent==='elix-resume.bin' && Array.from(entry.querySelectorAll('button')).some(button=>button.textContent==='Fortsett' && !button.disabled))").unwrap_or(false)
    })?;
    app.get_webview("shell").ok_or("Missing shell")?.eval("Array.from(document.querySelectorAll('.download-entry')).find(entry=>entry.querySelector('strong')?.textContent==='elix-resume.bin').querySelector('.download-actions button').click()").map_err(|e|e.to_string())?;

    wait(|| {
        app.state::<AppState>()
            .downloads
            .lock()
            .unwrap()
            .iter()
            .any(|d| d.id == id && d.status == "complete")
    })?;
    let path = app
        .state::<AppState>()
        .downloads
        .lock()
        .unwrap()
        .iter()
        .find(|d| d.id == id)
        .ok_or("Resumed download missing")?
        .path
        .clone();
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    if bytes.len() != SLOW_DOWNLOAD_BYTES || bytes.iter().any(|byte| *byte != b'E') {
        return Err("Resumed download bytes differ".into());
    }
    browser::close(app, &manager_id)?;
    let state = app.state::<AppState>();
    let persisted = crate::downloads::history::load(&state.root)?;
    if !persisted.iter().any(|d| {
        d.id == id && d.status == "complete" && d.bytes_received == SLOW_DOWNLOAD_BYTES as u64
    }) {
        return Err("Complete download was not persisted for app restart".into());
    }
    Ok(())
}
fn concurrent_download_test(app: &AppHandle, origin: &str, return_tab: &str) -> Result<(), String> {
    // The retry fix must key by native operation identity, never by filename or
    // URL: two deliberate downloads of the same resource must remain distinct.
    let url = format!("{origin}/slow");
    let first = tauri::async_runtime::block_on(browser::create(app.clone(), url.clone()))?;
    let second = tauri::async_runtime::block_on(browser::create(app.clone(), url))?;
    wait(|| {
        let state = app.state::<AppState>();
        let downloads = state.downloads.lock().unwrap();
        [first.as_str(), second.as_str()].iter().all(|view_id| {
            downloads
                .iter()
                .any(|download| download.view_id == *view_id && download.status == "downloading")
        })
    })?;
    wait(|| {
        let state = app.state::<AppState>();
        let downloads = state.downloads.lock().unwrap();
        [first.as_str(), second.as_str()].iter().all(|view_id| {
            downloads
                .iter()
                .any(|download| download.view_id == *view_id && download.status == "complete")
        })
    })?;
    let items = app
        .state::<AppState>()
        .downloads
        .lock()
        .unwrap()
        .iter()
        .filter(|download| download.view_id == first || download.view_id == second)
        .cloned()
        .collect::<Vec<_>>();
    if items.len() != 2 || items[0].id == items[1].id || items[0].path == items[1].path {
        return Err("Concurrent same-URL downloads were merged or duplicated".into());
    }
    for item in &items {
        let bytes = std::fs::read(&item.path).map_err(|e| e.to_string())?;
        if bytes.len() != SLOW_DOWNLOAD_BYTES || bytes.iter().any(|byte| *byte != b'E') {
            return Err("Concurrent download bytes differ".into());
        }
    }
    browser::close(app, &first)?;
    browser::close(app, &second)?;
    browser::activate(app, return_tab)?;
    Ok(())
}
fn permission_test(app: &AppHandle, origin: &str) -> Result<(), String> {
    let id = tauri::async_runtime::block_on(browser::create(
        app.clone(),
        format!("{origin}/permission"),
    ))?;
    wait(|| title(app, &id, "ELIX permission fixture"))?;
    let view = app.get_webview(&id).ok_or("Missing permission view")?;
    view.eval("navigator.mediaDevices.getUserMedia({audio:true}).then(stream=>{stream.getTracks().forEach(t=>t.stop());document.title='ELIX permission unexpectedly granted'},()=>{document.title='ELIX permission denied'})").map_err(|e|e.to_string())?;
    let mut pending = None;
    wait(|| {
        pending = tauri::async_runtime::block_on(crate::security::permissions::list(app))
            .ok()
            .and_then(|items| items.into_iter().find(|p| p.tab_id == id));
        pending.is_some()
    })?;
    let prompt = pending.ok_or("Missing microphone permission prompt")?;
    if prompt.origin != origin || prompt.top_origin != origin {
        return Err("Microphone permission prompt has wrong origin".into());
    }
    tauri::async_runtime::block_on(crate::security::permissions::respond(
        app, &prompt.id, false,
    ))?;
    wait(|| title(app, &id, "ELIX permission denied"))?;
    if !tauri::async_runtime::block_on(crate::security::permissions::list(app))?.is_empty() {
        return Err("Resolved permission prompt remained pending".into());
    }
    browser::close(app, &id)?;
    Ok(())
}
const SLOW_DOWNLOAD_BYTES: usize = 8 * 1024 * 1024;
fn serve_fixture(mut stream: std::net::TcpStream) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(3)));
    let mut request = [0; 8192];
    let size = stream.read(&mut request).unwrap_or(0);
    let request = String::from_utf8_lossy(&request[..size]);
    if request.starts_with("GET /broken ") {
        let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 1000000\r\nContent-Type: application/octet-stream\r\nContent-Disposition: attachment; filename=broken.txt\r\nConnection: close\r\n\r\n");
        // Commit the download before disconnecting mid-transfer. An already
        // truncated response may be retried as navigation instead.
        let _ = stream.flush();
        std::thread::sleep(Duration::from_millis(500));
        let _ = stream.write_all(b"partial");
        let _ = stream.flush();
        std::thread::sleep(Duration::from_millis(500));
        return;
    }
    if request.starts_with("GET /slow ") {
        let offset = request
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                if !name.eq_ignore_ascii_case("range") {
                    return None;
                }
                value
                    .trim()
                    .strip_prefix("bytes=")?
                    .split('-')
                    .next()?
                    .parse::<usize>()
                    .ok()
            })
            .unwrap_or(0)
            .min(SLOW_DOWNLOAD_BYTES - 1);
        let status = if offset > 0 {
            "206 Partial Content"
        } else {
            "200 OK"
        };
        let range = if offset > 0 {
            format!(
                "Content-Range: bytes {}-{}/{}\r\n",
                offset,
                SLOW_DOWNLOAD_BYTES - 1,
                SLOW_DOWNLOAD_BYTES
            )
        } else {
            String::new()
        };
        let headers = format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\nContent-Type: application/octet-stream\r\nContent-Disposition: attachment; filename=elix-resume.bin\r\nAccept-Ranges: bytes\r\nETag: \"elix-resume-fixture-v1\"\r\n{range}Connection: close\r\n\r\n", SLOW_DOWNLOAD_BYTES - offset);
        if stream.write_all(headers.as_bytes()).is_err() {
            return;
        }
        let bytes = [b'E'; 65536];
        let mut sent = offset;
        while sent < SLOW_DOWNLOAD_BYTES {
            let length = bytes.len().min(SLOW_DOWNLOAD_BYTES - sent);
            if stream.write_all(&bytes[..length]).is_err() {
                return;
            }
            let _ = stream.flush();
            sent += length;
            std::thread::sleep(Duration::from_millis(50));
        }
        return;
    }
    if request.starts_with("GET /favicon.png ") {
        // A real PNG fetched by the native renderer, not a shell-assigned icon.
        let png: &[u8] = &[
            137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 16, 0, 0, 0, 16,
            8, 2, 0, 0, 0, 144, 145, 104, 54, 0, 0, 0, 22, 73, 68, 65, 84, 120, 156, 99, 112, 190,
            252, 130, 36, 196, 48, 170, 97, 84, 195, 240, 213, 0, 0, 123, 196, 254, 16, 221, 3, 4,
            4, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
        ];
        let headers = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: image/png\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n", png.len());
        let _ = stream.write_all(headers.as_bytes());
        let _ = stream.write_all(png);
        return;
    }
    let download = request.starts_with("GET /download ");
    let body = if download {
        "elix-native-download".to_owned()
    } else {
        let title = if request.starts_with("GET /two ") {
            "ELIX native two"
        } else if request.starts_with("GET /private-secret ") {
            "ELIX private fixture"
        } else if request.starts_with("GET /permission ") {
            "ELIX permission fixture"
        } else {
            "ELIX native one"
        };
        format!("<!doctype html><html><head><title>{title}</title><link rel=\"icon\" href=\"/favicon.png\" type=\"image/png\"></head><body>Native WebView2 fixture</body></html>")
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
fn exercise(app: &AppHandle) -> Result<(), String> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    let origin = format!(
        "http://{}",
        listener.local_addr().map_err(|e| e.to_string())?
    );
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            // A paused transfer must not prevent navigation, favicon requests, or
            // the resumed Range request from reaching the same local fixture.
            std::thread::spawn(move || serve_fixture(stream));
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
    wait(|| {
        app.state::<AppState>()
            .session
            .lock()
            .unwrap()
            .tabs
            .iter()
            .any(|t| {
                t.id == first
                    && t.favicon
                        .as_deref()
                        .is_some_and(|icon| icon.starts_with("data:image/png;base64,"))
            })
    })?;
    #[cfg(windows)]
    history_shortcut_test(app, &first)?;
    browser::navigate(app, &first, &format!("{origin}/one"))?;
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
    let unused = std::net::TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    let unavailable = format!(
        "http://{}/unavailable",
        unused.local_addr().map_err(|e| e.to_string())?
    );
    drop(unused);
    browser::navigate(app, &first, &unavailable)?;
    wait(|| {
        app.state::<AppState>()
            .session
            .lock()
            .unwrap()
            .tabs
            .iter()
            .any(|tab| tab.id == first && !tab.loading && tab.navigation_error.is_some())
    })?;
    browser::navigate(app, &first, &format!("{origin}/two"))?;
    wait(|| {
        title(app, &first, "ELIX native two")
            && app
                .state::<AppState>()
                .session
                .lock()
                .unwrap()
                .tabs
                .iter()
                .any(|tab| tab.id == first && tab.navigation_error.is_none())
    })?;
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
    tauri::async_runtime::block_on(browser::extensions::smoke(app.clone()))?;
    private_test(app, &first, &origin)?;
    permission_test(app, &origin)?;
    browser::pin(app, &first, true)?;
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
    resumable_download_test(app, &second, &origin)?;
    concurrent_download_test(app, &origin, &second)?;
    let broken_url = format!("{origin}/broken");
    browser::navigate(app, &second, &broken_url)?;
    wait(|| {
        app.state::<AppState>()
            .downloads
            .lock()
            .unwrap()
            .iter()
            .any(|d| d.view_id == second && d.url == broken_url && d.status == "failed")
    })?;
    #[cfg(windows)]
    tauri::async_runtime::block_on(crate::downloads::windows::refresh(app))?;
    let retry_items = app
        .state::<AppState>()
        .downloads
        .lock()
        .unwrap()
        .iter()
        .filter(|download| download.view_id == second && download.url == broken_url)
        .cloned()
        .collect::<Vec<_>>();
    // Runtime retries may create distinct native operations. Preserve actual
    // attempts without merging unrelated same-URL downloads or inventing a
    // single-operation guarantee that WebView2 does not make.
    let ids = retry_items
        .iter()
        .map(|download| &download.id)
        .collect::<std::collections::HashSet<_>>();
    let paths = retry_items
        .iter()
        .map(|download| &download.path)
        .collect::<std::collections::HashSet<_>>();
    if retry_items.is_empty()
        || !retry_items
            .iter()
            .any(|download| download.status == "failed")
        || ids.len() != retry_items.len()
        || paths.len() != retry_items.len()
    {
        return Err(format!(
            "Interrupted download attempts are missing or collide: {}",
            serde_json::to_string(&retry_items).map_err(|e| e.to_string())?
        ));
    }
    // A resumable interruption may already be retrying when cleanup runs.
    // Explicitly cancel the live operation before testing tab suspension.
    let live = app
        .state::<AppState>()
        .downloads
        .lock()
        .unwrap()
        .iter()
        .filter(|download| {
            download.view_id == second
                && (download.can_resume
                    || matches!(download.status.as_str(), "downloading" | "paused"))
        })
        .map(|download| download.id.clone())
        .collect::<Vec<_>>();
    for id in live {
        tauri::async_runtime::block_on(crate::downloads::action(app, &id, "cancel"))?;
    }
    let mut last_refresh = Instant::now() - Duration::from_secs(1);
    let settled = wait(|| {
        #[cfg(windows)]
        if last_refresh.elapsed() >= Duration::from_millis(500) {
            let _ = tauri::async_runtime::block_on(crate::downloads::windows::refresh(app));
            last_refresh = Instant::now();
        }
        !app.state::<AppState>()
            .downloads
            .lock()
            .unwrap()
            .iter()
            .any(|download| {
                download.view_id == second
                    && (download.can_resume
                        || matches!(download.status.as_str(), "downloading" | "paused"))
            })
    });
    if settled.is_err() {
        let phantom = app
            .state::<AppState>()
            .downloads
            .lock()
            .unwrap()
            .iter()
            .filter(|download| {
                download.view_id == second
                    && (download.can_resume
                        || matches!(download.status.as_str(), "downloading" | "paused"))
            })
            .cloned()
            .collect::<Vec<_>>();
        return Err(format!(
            "Canceled transfer left active retry records: {}",
            serde_json::to_string(&phantom).map_err(|e| e.to_string())?
        ));
    }
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
    if !session.tabs.iter().any(|t| {
        t.id == session.active && t.url == format!("{origin}/two") && t.pinned && !t.private
    }) {
        return Err("Pinned active tab was not restored".into());
    }
    Ok(())
}
