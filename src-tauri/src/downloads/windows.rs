//! COM download operations stay on the WebView2 UI thread for their full lifetime.
//! In particular, an interrupted download may be paused or automatically retried.
use crate::AppState;
use std::{cell::RefCell, collections::HashMap, path::PathBuf};
use tauri::{Emitter, Manager};
use webview2_com::{
    BytesReceivedChangedEventHandler, DownloadStartingEventHandler,
    Microsoft::Web::WebView2::Win32::*, StateChangedEventHandler,
};
use windows::core::{Interface, PWSTR};

thread_local! {
    static OPERATIONS: RefCell<HashMap<PathBuf, ICoreWebView2DownloadOperation>> = RefCell::new(HashMap::new());
}

pub unsafe fn attach(core: &ICoreWebView2, app: tauri::AppHandle) -> windows::core::Result<()> {
    let mut token = 0;
    core.cast::<ICoreWebView2_4>()?.add_DownloadStarting(
        &DownloadStartingEventHandler::create(Box::new(move |_, args| {
            if let Some(args) = args {
                // ELIX presents its own download list and controls.
                args.SetHandled(true)?;
                let operation = args.DownloadOperation()?;
                let path = result_path(&operation)?;
                OPERATIONS.with(|items| items.borrow_mut().insert(path.clone(), operation.clone()));
                let event_app = app.clone();
                let mut token = 0;
                operation.add_StateChanged(
                    &StateChangedEventHandler::create(Box::new(move |operation, _| {
                        if let Some(operation) = operation {
                            if update(&event_app, &operation, true)? {
                                let path = result_path(&operation)?;
                                OPERATIONS.with(|items| items.borrow_mut().remove(&path));
                            }
                        }
                        Ok(())
                    })),
                    &mut token,
                )?;
                let progress_app = app.clone();
                operation.add_BytesReceivedChanged(
                    &BytesReceivedChangedEventHandler::create(Box::new(move |operation, _| {
                        if let Some(operation) = operation {
                            update(&progress_app, &operation, false)?;
                        }
                        Ok(())
                    })),
                    &mut token,
                )?;
                // A small response can finish before handlers are registered.
                if update(&app, &operation, true)? {
                    OPERATIONS.with(|items| items.borrow_mut().remove(&path));
                }
            }
            Ok(())
        })),
        &mut token,
    )
}

unsafe fn result_path(
    operation: &ICoreWebView2DownloadOperation,
) -> windows::core::Result<PathBuf> {
    let mut raw = PWSTR::null();
    operation.ResultFilePath(&mut raw)?;
    let value = raw.to_string();
    windows::Win32::System::Com::CoTaskMemFree(Some(raw.0.cast()));
    Ok(value.map(PathBuf::from)?)
}

unsafe fn update(
    app: &tauri::AppHandle,
    operation: &ICoreWebView2DownloadOperation,
    persist: bool,
) -> windows::core::Result<bool> {
    let path = result_path(operation)?;
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
        if let Some(download) = downloads.iter_mut().find(|d| d.path == path) {
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
        let operation = OPERATIONS.with(|items| items.borrow().get(&path).cloned());
        let result = (|| -> Result<(), String> {
            let operation = operation
                .ok_or("Nedlastingen er ikke lenger aktiv. Start den på nytt fra nettstedet.")?;
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
                if update(&callback_app, &operation, true).map_err(|e| e.to_string())? {
                    OPERATIONS.with(|items| items.borrow_mut().remove(&path));
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
