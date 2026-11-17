//! Favicons come from WebView2's already-fetched image stream, never an external icon service.
use crate::{browser, AppState};
use base64::Engine;
use tauri::{Emitter, Manager, Webview};
use webview2_com::{
    FaviconChangedEventHandler, GetFaviconCompletedHandler, Microsoft::Web::WebView2::Win32::*,
};
use windows::{
    core::{Interface, PWSTR},
    Win32::System::Com::{CoTaskMemFree, STATFLAG_NONAME, STATSTG},
};

pub fn attach(view: &Webview) -> Result<(), String> {
    let app = view.app_handle().clone();
    let id = view.label().to_string();
    view.with_webview(move |platform| {
        let result = (|| -> windows::core::Result<()> {
            unsafe {
                let core = platform.controller().CoreWebView2()?;
                // Older runtimes simply retain the generic icon.
                let Ok(core15) = core.cast::<ICoreWebView2_15>() else {
                    return Ok(());
                };
                let icon_app = app.clone();
                let icon_id = id.clone();
                let mut token = 0;
                core15.add_FaviconChanged(
                    &FaviconChangedEventHandler::create(Box::new(move |sender, _| {
                        if let Some(core) = sender {
                            update(&core, &icon_app, &icon_id)?;
                        }
                        Ok(())
                    })),
                    &mut token,
                )?;
                update(&core, &app, &id)?;
            }
            Ok(())
        })();
        if result.is_err() {
            app.state::<AppState>()
                .logs
                .add("warn", "favicon_unavailable");
        }
    })
    .map_err(|e| e.to_string())
}

unsafe fn update(
    core: &ICoreWebView2,
    app: &tauri::AppHandle,
    id: &str,
) -> windows::core::Result<()> {
    let core15 = core.cast::<ICoreWebView2_15>()?;
    let mut raw = PWSTR::null();
    core.Source(&mut raw)?;
    let source = raw.to_string().unwrap_or_default();
    CoTaskMemFree(Some(raw.0.cast()));
    let app = app.clone();
    let id = id.to_string();
    core15.GetFavicon(
        COREWEBVIEW2_FAVICON_IMAGE_FORMAT_PNG,
        &GetFaviconCompletedHandler::create(Box::new(move |result, stream| {
            result?;
            let mut favicon = None;
            if let Some(stream) = stream {
                let mut stat = STATSTG::default();
                stream.Stat(&mut stat, STATFLAG_NONAME)?;
                // Bound untrusted images before allocation / JSON transport.
                if stat.cbSize > 0 && stat.cbSize <= 512 * 1024 {
                    let mut data = vec![0u8; stat.cbSize as usize];
                    let mut read = 0;
                    stream
                        .Read(data.as_mut_ptr().cast(), data.len() as u32, Some(&mut read))
                        .ok()?;
                    data.truncate(read as usize);
                    if data.starts_with(b"\x89PNG\r\n\x1a\n") {
                        favicon = Some(format!(
                            "data:image/png;base64,{}",
                            base64::engine::general_purpose::STANDARD.encode(data)
                        ));
                    }
                }
            }
            let mut save = false;
            {
                let state = app.state::<AppState>();
                let mut session = state.session.lock().unwrap();
                if let Some(tab) = session
                    .tabs
                    .iter_mut()
                    .find(|t| t.id == id && t.url == source)
                {
                    tab.favicon = favicon.clone();
                    save = !tab.private;
                }
            }
            if save {
                let state = app.state::<AppState>();
                let profile = state.profile.lock().unwrap().clone();
                let _ = state.store.lock().unwrap().bookmark_icon(
                    &profile,
                    &source,
                    favicon.as_deref(),
                );
                let _ = app.emit_to("shell", "bookmarks-changed", ());
            }
            browser::emit(&app);
            Ok(())
        })),
    )
}
