use serde_json::Value;
use tauri::AppHandle;

pub async fn cookies(app: &AppHandle, origin: String, delete: bool) -> Result<Value, String> {
    let url = url::Url::parse(&origin).map_err(|_| "Ugyldig nettsted")?;
    if !crate::security::remote_allowed(&url) || url.origin().ascii_serialization() != origin {
        return Err("Velg et gyldig nettsted".into());
    }
    #[cfg(windows)]
    {
        native(app, url.host_str().unwrap().to_string(), delete).await
    }
    #[cfg(not(windows))]
    {
        let _ = (app, delete);
        Err("Krever Windows/WebView2".into())
    }
}

#[cfg(windows)]
async fn native(app: &AppHandle, host: String, delete: bool) -> Result<Value, String> {
    use serde_json::json;
    use webview2_com::{GetCookiesCompletedHandler, Microsoft::Web::WebView2::Win32::*};
    use windows::core::{w, Interface, PWSTR};
    let (view, temporary) = super::extensions::profile_view(app)?;
    let (tx, rx) = std::sync::mpsc::channel();
    let started = view
        .with_webview(move |platform| {
            let result = (|| -> windows::core::Result<()> {
                unsafe {
                    let manager = platform
                        .controller()
                        .CoreWebView2()?
                        .cast::<ICoreWebView2_2>()?
                        .CookieManager()?;
                    let callback_manager = manager.clone();
                    let callback_tx = tx.clone();
                    manager.GetCookies(
                        w!(""),
                        &GetCookiesCompletedHandler::create(Box::new(move |result, list| {
                            let result = (|| -> windows::core::Result<Value> {
                                result?;
                                let mut out = Vec::new();
                                if let Some(list) = list {
                                    let mut count = 0;
                                    list.Count(&mut count)?;
                                    for index in 0..count {
                                        let cookie = list.GetValueAtIndex(index)?;
                                        let mut raw = PWSTR::null();
                                        cookie.Domain(&mut raw)?;
                                        let domain = raw.to_string().unwrap_or_default();
                                        windows::Win32::System::Com::CoTaskMemFree(Some(
                                            raw.0.cast(),
                                        ));
                                        let d = domain.trim_start_matches('.');
                                        if host != d
                                            && !(domain.starts_with('.')
                                                && host.ends_with(&format!(".{d}")))
                                        {
                                            continue;
                                        }
                                        if delete {
                                            callback_manager.DeleteCookie(&cookie)?;
                                            continue;
                                        }
                                        cookie.Name(&mut raw)?;
                                        let name = raw.to_string().unwrap_or_default();
                                        windows::Win32::System::Com::CoTaskMemFree(Some(
                                            raw.0.cast(),
                                        ));
                                        cookie.Path(&mut raw)?;
                                        let path = raw.to_string().unwrap_or_default();
                                        windows::Win32::System::Com::CoTaskMemFree(Some(
                                            raw.0.cast(),
                                        ));
                                        out.push(json!({"name":name,"domain":domain,"path":path}));
                                    }
                                }
                                Ok(json!(out))
                            })()
                            .map_err(|e| e.to_string());
                            let _ = callback_tx.send(result);
                            Ok(())
                        })),
                    )?;
                    Ok(())
                }
            })();
            if let Err(e) = result {
                let _ = tx.send(Err(e.to_string()));
            }
        })
        .map_err(|e| e.to_string());
    let result = match started {
        Err(e) => Err(e),
        Ok(()) => tauri::async_runtime::spawn_blocking(move || {
            rx.recv_timeout(std::time::Duration::from_secs(15))
                .map_err(|_| "WebView2 svarte ikke".to_string())?
        })
        .await
        .map_err(|e| e.to_string())?,
    };
    if temporary {
        let _ = view.close();
    }
    result
}
