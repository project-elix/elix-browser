//! COM download operations stay on the WebView2 UI thread for their full lifetime.
//! WebView2 can raise DownloadStarting again when the same operation retries. Its
//! COM identity, not the mutable result path or URL, identifies one logical transfer.
use crate::AppState;
use std::{cell::RefCell, collections::HashMap, path::PathBuf};
use tauri::{Emitter, Manager};
use webview2_com::{
    BytesReceivedChangedEventHandler, DownloadStartingEventHandler,
    Microsoft::Web::WebView2::Win32::*, StateChangedEventHandler,
};
use windows::core::{IUnknown, Interface, HSTRING, PWSTR};

#[derive(Clone)]
struct ActiveDownload {
    id: String,
    path: PathBuf,
    operation: ICoreWebView2DownloadOperation,
}
thread_local! {
    static OPERATIONS: RefCell<HashMap<usize, ActiveDownload>> = RefCell::new(HashMap::new());
}

pub unsafe fn attach(
    core: &ICoreWebView2,
    app: tauri::AppHandle,
    view_id: String,
) -> windows::core::Result<()> {
    let state = app.state::<AppState>();
    let profile = state.profile.lock().unwrap().clone();
    let private = state
        .session
        .lock()
        .unwrap()
        .tabs
        .iter()
        .find(|tab| tab.id == view_id)
        .map(|tab| tab.private);
    let mut token = 0;
    core.cast::<ICoreWebView2_4>()?.add_DownloadStarting(
        &DownloadStartingEventHandler::create(Box::new(move |_, args| {
            let Some(args) = args else { return Ok(()) };
            args.SetHandled(true)?;
            let mut created: Option<(usize, String)> = None;
            let result = (|| -> windows::core::Result<()> {
                let Some(private) = private else {
                    args.SetCancel(true)?;
                    return Ok(());
                };
                let operation = args.DownloadOperation()?;
                // Querying IUnknown gives the stable identity even if an event
                // supplies a different interface pointer for the same operation.
                let identity = operation.cast::<IUnknown>()?.as_raw() as usize;
                let existing = OPERATIONS.with(|items| items.borrow().get(&identity).cloned());
                if let Some(existing) = existing {
                    // Reuse the original destination and handlers. A retry must
                    // never create a second row or strand the first row in progress.
                    args.SetResultFilePath(&HSTRING::from(existing.path.as_os_str()))?;
                    app.state::<AppState>().logs.add("info", "download_native_retry");
                    if update(&app, &existing.id, &operation, true)? {
                        OPERATIONS.with(|items| items.borrow_mut().remove(&identity));
                    }
                    return Ok(());
                }
                let mut suggested = PWSTR::null();
                args.ResultFilePath(&mut suggested)?;
                let suggested = take_string(suggested)?;
                let root = app.path().download_dir().map_err(|_| windows::core::Error::from_hresult(windows::core::HRESULT(0x80004005u32 as i32)))?;
                let (id, path) = super::destination(root, std::path::Path::new(&suggested)).map_err(|_| windows::core::Error::from_hresult(windows::core::HRESULT(0x80004005u32 as i32)))?;
                args.SetResultFilePath(&HSTRING::from(path.as_os_str()))?;
                let mut uri = PWSTR::null();
                operation.Uri(&mut uri)?;
                let uri = take_string(uri)?;
                app.state::<AppState>().downloads.lock().unwrap().push(super::Download::new(
                    id.clone(), path.clone(), profile.clone(), private, uri, view_id.clone(),
                ));
                OPERATIONS.with(|items| items.borrow_mut().insert(identity, ActiveDownload {
                    id: id.clone(), path, operation: operation.clone(),
                }));
                created = Some((identity, id.clone()));
                let event_app = app.clone();
                let event_id = id.clone();
                let mut token = 0;
                operation.add_StateChanged(
                    &StateChangedEventHandler::create(Box::new(move |operation, _| {
                        if let Some(operation) = operation {
                            if update(&event_app, &event_id, &operation, true)? {
                                OPERATIONS.with(|items| items.borrow_mut().remove(&identity));
                            }
                        }
                        Ok(())
                    })), &mut token,
                )?;
                let progress_app = app.clone();
                let progress_id = id.clone();
                operation.add_BytesReceivedChanged(
                    &BytesReceivedChangedEventHandler::create(Box::new(move |operation, _| {
                        if let Some(operation) = operation { update(&progress_app, &progress_id, &operation, false)?; }
                        Ok(())
                    })), &mut token,
                )?;
                // A small response can finish before handlers are registered.
                // Capture the stable record id: ResultFilePath can lag behind the
                // DownloadStarting args until that event has returned.
                if update(&app, &id, &operation, true)? {
                    OPERATIONS.with(|items| items.borrow_mut().remove(&identity));
                }
                Ok(())
            })();
            if result.is_err() {
                // Cancellation is requested even if native handler registration
                // failed; finish the new record ourselves because no callback may exist.
                let canceled = args.SetCancel(true);
                if let Some((identity, id)) = created {
                    OPERATIONS.with(|items| items.borrow_mut().remove(&identity));
                    {
                        let state = app.state::<AppState>();
                        let mut downloads = state.downloads.lock().unwrap();
                        if let Some(download) = downloads.iter_mut().find(|download| download.id == id) {
                            download.status = "failed".into();
                            download.can_resume = false;
                        }
                    }
                    super::persist(&app);
                    let _ = app.emit_to("shell", "downloads-changed", ());
                }
                app.state::<AppState>().logs.add("warn", "download_start_failed");
                canceled?;
                let _ = app.emit_to("shell", "notice", "Nedlastingen kunne ikke startes. Kontroller at nedlastingsmappen er tilgjengelig, og prøv igjen.");
            }
            Ok(())
        })), &mut token,
    )
}

unsafe fn take_string(raw: PWSTR) -> windows::core::Result<String> {
    let value = raw.to_string();
    windows::Win32::System::Com::CoTaskMemFree(Some(raw.0.cast()));
    Ok(value?)
}

unsafe fn update(
    app: &tauri::AppHandle,
    id: &str,
    operation: &ICoreWebView2DownloadOperation,
    persist: bool,
) -> windows::core::Result<bool> {
    let mut native_state = COREWEBVIEW2_DOWNLOAD_STATE_IN_PROGRESS;
    let mut reason = COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_NONE;
    let mut received = 0;
    let mut total = -1;
    let mut can_resume = windows::core::BOOL(0);
    operation.State(&mut native_state)?;
    operation.BytesReceived(&mut received)?;
    operation.TotalBytesToReceive(&mut total)?;
    if native_state == COREWEBVIEW2_DOWNLOAD_STATE_INTERRUPTED {
        operation.InterruptReason(&mut reason)?;
        operation.CanResume(&mut can_resume)?;
    }
    let status = if native_state == COREWEBVIEW2_DOWNLOAD_STATE_COMPLETED {
        "complete"
    } else if native_state == COREWEBVIEW2_DOWNLOAD_STATE_IN_PROGRESS {
        "downloading"
    } else if reason == COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_USER_PAUSED {
        "paused"
    } else if reason == COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_USER_CANCELED {
        "canceled"
    } else {
        "failed"
    };
    {
        let state = app.state::<AppState>();
        let mut downloads = state.downloads.lock().unwrap();
        if let Some(download) = downloads.iter_mut().find(|d| d.id == id) {
            download.status = status.into();
            download.bytes_received = received.max(0) as u64;
            download.total_bytes = (total >= 0).then_some(total as u64);
            download.can_resume = can_resume.as_bool();
        }
        if persist && super::history::save(&state.root, &downloads).is_err() {
            state.logs.add("error", "download_history_write_failed");
        }
    }
    let _ = app.emit_to("shell", "downloads-changed", ());
    // Interrupted operations must remain alive: WebView2 can retry by itself and
    // the user can resume a paused/resumable transfer through the shell.
    Ok(status == "complete" || status == "canceled")
}

pub async fn action(app: &tauri::AppHandle, path: PathBuf, action: String) -> Result<(), String> {
    let (tx, rx) = std::sync::mpsc::channel();
    let callback_app = app.clone();
    app.run_on_main_thread(move || {
        // Drop the registry borrow before invoking COM: it can synchronously
        // raise StateChanged, which in turn removes a completed operation.
        let active = OPERATIONS.with(|items| {
            items
                .borrow()
                .iter()
                .find(|(_, active)| active.path == path)
                .map(|(key, active)| (*key, active.clone()))
        });
        let result = (|| -> Result<(), String> {
            let (identity, active) = active
                .ok_or("Nedlastingen er ikke lenger aktiv. Start den på nytt fra nettstedet.")?;
            let operation = active.operation;
            unsafe {
                match action.as_str() {
                    "pause" => operation.Pause().map_err(|e| e.to_string())?,
                    "resume" => {
                        let mut can_resume = windows::core::BOOL(0);
                        operation
                            .CanResume(&mut can_resume)
                            .map_err(|e| e.to_string())?;
                        if !can_resume.as_bool() {
                            return Err(
                                "Nettstedet tillater ikke gjenopptakelse av denne nedlastingen."
                                    .into(),
                            );
                        }
                        operation.Resume().map_err(|e| e.to_string())?;
                    }
                    "cancel" => operation.Cancel().map_err(|e| e.to_string())?,
                    _ => return Err("Ukjent nedlastingshandling".into()),
                }
                if update(&callback_app, &active.id, &operation, true).map_err(|e| e.to_string())? {
                    OPERATIONS.with(|items| items.borrow_mut().remove(&identity));
                }
            }
            Ok(())
        })();
        let _ = tx.send(result);
    })
    .map_err(|e| e.to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        rx.recv_timeout(std::time::Duration::from_secs(10))
            .map_err(|_| "WebView2 svarte ikke på nedlastingshandlingen".to_string())?
    })
    .await
    .map_err(|e| e.to_string())?
}
