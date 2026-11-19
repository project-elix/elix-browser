//! Query only when a background tab is eligible. Unknown signals leave it live.
use tauri::Webview;

const PAGE_IDLE: &str = "(()=>{try{const idle=w=>w.__elixFrameIdle?.()===true&&Array.from(w.frames).every(idle);return idle(window)}catch{return false}})()";

pub(super) fn idle(view: &Webview) -> Result<bool, String> {
    let (tx, rx) = std::sync::mpsc::channel();
    view.eval_with_callback(PAGE_IDLE, move |value| { let _ = tx.send(value); })
        .map_err(|e| e.to_string())?;
    let page_idle = rx.recv_timeout(std::time::Duration::from_secs(2))
        .map_err(|_| "Kunne ikke kontrollere aktivitet i fanen")?;
    if page_idle != "true" { return Ok(false); }

    #[cfg(windows)]
    {
        let (tx, rx) = std::sync::mpsc::channel();
        view.with_webview(move |platform| {
            use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2_8;
            use windows::core::{Interface, BOOL};
            let audio = (|| -> windows::core::Result<bool> {
                unsafe {
                    let mut playing = BOOL::default();
                    platform.controller().CoreWebView2()?.cast::<ICoreWebView2_8>()?
                        .IsDocumentPlayingAudio(&mut playing)?;
                    Ok(playing.as_bool())
                }
            })();
            let _ = tx.send(audio.map_err(|e| e.to_string()));
        }).map_err(|e| e.to_string())?;
        let playing = rx.recv_timeout(std::time::Duration::from_secs(2))
            .map_err(|_| "Kunne ikke kontrollere lyden i fanen")??;
        Ok(!playing)
    }
    #[cfg(not(windows))]
    Ok(false)
}
