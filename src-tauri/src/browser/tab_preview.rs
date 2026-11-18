//! On-demand preview for the trusted shell. No captures are stored on disk.
use crate::AppState;
use base64::Engine;
use tauri::{AppHandle, Manager};
use webview2_com::{CapturePreviewCompletedHandler, Microsoft::Web::WebView2::Win32::*};
use windows::Win32::{
    Foundation::HGLOBAL,
    System::Com::{
        StructuredStorage::CreateStreamOnHGlobal, IStream, STATFLAG_NONAME, STATSTG, STREAM_SEEK_SET,
    },
};

fn eligible(app: &AppHandle, id: &str, url: &str) -> bool {
    let state = app.state::<AppState>();
    let session = state.session.lock().unwrap();
    session.tabs.iter().any(|tab| {
        tab.id == id && tab.url == url && !tab.private && !tab.suspended && !tab.loading
            && (url.starts_with("https://") || url.starts_with("http://"))
    })
}

unsafe fn jpeg(stream: &IStream) -> windows::core::Result<Option<String>> {
    let mut stat = STATSTG::default();
    stream.Stat(&mut stat, STATFLAG_NONAME)?;
    if !(3..=768 * 1024).contains(&stat.cbSize) {
        return Ok(None);
    }
    stream.Seek(0, STREAM_SEEK_SET, None)?;
    let mut bytes = vec![0; stat.cbSize as usize];
    let mut read = 0;
    stream.Read(bytes.as_mut_ptr().cast(), bytes.len() as u32, Some(&mut read)).ok()?;
    if read as usize != bytes.len() || !bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        return Ok(None);
    }
    Ok(Some(format!("data:image/jpeg;base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes))))
}

pub async fn capture(app: &AppHandle, id: &str, url: &str) -> Option<String> {
    if !eligible(app, id, url) { return None; }
    let view = app.get_webview(id)?;
    let (send, mut receive) = tauri::async_runtime::channel(1);
    view.with_webview(move |platform| {
        let setup = || -> windows::core::Result<()> {
            unsafe {
                let core = platform.controller().CoreWebView2()?;
                let stream = CreateStreamOnHGlobal(HGLOBAL::default(), true)?;
                let result = stream.clone();
                core.CapturePreview(COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_JPEG, &stream,
                    &CapturePreviewCompletedHandler::create(Box::new(move |status| {
                        let image = status.and_then(|_| jpeg(&result)).ok().flatten();
                        let _ = send.try_send(image);
                        Ok(())
                    })))
            }
        };
        let _ = setup();
    }).ok()?;
    let image = tokio::time::timeout(std::time::Duration::from_secs(3), receive.recv())
        .await.ok()?.flatten()?;
    eligible(app, id, url).then_some(image)
}
