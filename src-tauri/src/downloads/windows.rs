//! COM download operations stay on the WebView2 UI thread for their full lifetime.
//! A COM identity tracks one retained native wrapper. WebView2 can create a new
//! wrapper for another attempt, so distinct objects must never be merged by URL.
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
    on_accepted: impl Fn(&str) + 'static,
) -> windows::core::Result<()> {
    #[cfg(feature = "smoke-tests")]
    trace_navigation(core, &view_id)?;
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
            #[cfg(feature = "smoke-tests")]
            probe("starting", Some(&args));
            let _ = refresh_on_ui(&app);
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
                    // Repeated callbacks for this same native wrapper reuse its
                    // destination and handlers rather than stranding a duplicate row.
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
                #[cfg(feature = "smoke-tests")]
                trace(serde_json::json!({"event":"created","id":id,"view":view_id}));
                app.state::<AppState>().downloads.lock().unwrap().push(super::Download::new(
                    id.clone(), path.clone(), profile.clone(), private, uri.clone(), view_id.clone(),
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
                match update(&app, &id, &operation, true) {
                    Ok(true) => { OPERATIONS.with(|items| items.borrow_mut().remove(&identity)); }
                    Ok(false) => {}
                    Err(_) => {
                        // Progress getters can be unavailable at startup. The installed
                        // handlers can read them again without canceling the transfer.
                        super::persist(&app);
                        let _ = app.emit_to("shell", "downloads-changed", ());
                    }
                }
                on_accepted(&uri);
                if app.state::<AppState>().session.lock().unwrap().active == view_id {
                    let _ = app.emit_to("shell", "notice", "Nedlastingen er startet. Åpne Nedlastinger med Ctrl+J for å følge den.");
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

pub(crate) unsafe fn take_string(raw: PWSTR) -> windows::core::Result<String> {
    let value = raw.to_string();
    windows::Win32::System::Com::CoTaskMemFree(Some(raw.0.cast()));
    Ok(value?)
}

fn classify(
    state: COREWEBVIEW2_DOWNLOAD_STATE,
    reason: COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON,
    received: i64,
    can_resume: bool,
) -> (&'static str, bool) {
    if state == COREWEBVIEW2_DOWNLOAD_STATE_COMPLETED {
        return ("complete", false);
    }
    if state == COREWEBVIEW2_DOWNLOAD_STATE_IN_PROGRESS {
        // Verified WebView2 retry inconsistency: an abandoned zero-byte response
        // reports IN_PROGRESS forever, but has a content-length mismatch and no
        // resumable transfer. Keep its wrapper so genuine later progress can
        // recover; do not generalize this to stale USER_PAUSED or other reasons.
        if reason == COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_SERVER_CONTENT_LENGTH_MISMATCH
            && received == 0
            && !can_resume
        {
            return ("failed", false);
        }
        return ("downloading", false);
    }
    if reason == COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_USER_CANCELED {
        ("canceled", false)
    } else if reason == COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_USER_PAUSED {
        ("paused", can_resume)
    } else {
        ("failed", can_resume)
    }
}

unsafe fn update(
    app: &tauri::AppHandle,
    id: &str,
    operation: &ICoreWebView2DownloadOperation,
    persist: bool,
) -> windows::core::Result<bool> {
    // A successful user Cancel is terminal even when the runtime later emits a
    // stale IN_PROGRESS/INTERRUPTED snapshot for the abandoned native wrapper.
    if app
        .state::<AppState>()
        .downloads
        .lock()
        .unwrap()
        .iter()
        .any(|download| download.id == id && download.status == "canceled")
    {
        return Ok(true);
    }
    let mut native_state = COREWEBVIEW2_DOWNLOAD_STATE_IN_PROGRESS;
    let mut reason = COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_NONE;
    let mut received = 0;
    let mut total = -1;
    let mut can_resume = windows::core::BOOL(0);
    operation.State(&mut native_state)?;
    operation.BytesReceived(&mut received)?;
    operation.TotalBytesToReceive(&mut total)?;
    // Some WebView2 builds retain IN_PROGRESS for a discarded response while its
    // interrupt reason already says the response body did not match its length.
    // Read all native fields, rather than hiding the reason behind State.
    operation.InterruptReason(&mut reason)?;
    operation.CanResume(&mut can_resume)?;
    let (status, can_resume) = classify(native_state, reason, received, can_resume.as_bool());
    {
        let state = app.state::<AppState>();
        let mut downloads = state.downloads.lock().unwrap();
        if let Some(download) = downloads.iter_mut().find(|d| d.id == id) {
            download.status = status.into();
            download.bytes_received = received.max(0) as u64;
            download.total_bytes = (total >= 0).then_some(total as u64);
            download.can_resume = can_resume;
        }
        if persist && super::history::save(&state.download_root, &downloads).is_err() {
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
                    "cancel" => {
                        operation.Cancel().map_err(|e| e.to_string())?;
                        // S_OK accepts the cancellation. Some runtime wrappers
                        // retain their previous State/CanResume afterward; that
                        // stale snapshot must not offer Resume or block tab closure.
                        {
                            let state = callback_app.state::<AppState>();
                            let mut downloads = state.downloads.lock().unwrap();
                            if let Some(download) = downloads
                                .iter_mut()
                                .find(|download| download.id == active.id)
                            {
                                // Completion can win the race while Cancel is executing.
                                if download.status != "complete" {
                                    download.status = "canceled".into();
                                }
                                download.can_resume = false;
                            }
                        }
                        OPERATIONS.with(|items| items.borrow_mut().remove(&identity));
                        super::persist(&callback_app);
                        let _ = callback_app.emit_to("shell", "downloads-changed", ());
                        return Ok(());
                    }
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

/// Reconcile native getters as well as events. Some WebView2 builds can miss a
/// StateChanged callback during automatic retries; read State and InterruptReason.
unsafe fn refresh_on_ui(app: &tauri::AppHandle) -> windows::core::Result<()> {
    let operations = OPERATIONS.with(|items| {
        items
            .borrow()
            .iter()
            .map(|(key, active)| (*key, active.clone()))
            .collect::<Vec<_>>()
    });
    for (identity, active) in operations {
        if update(app, &active.id, &active.operation, true)? {
            OPERATIONS.with(|items| items.borrow_mut().remove(&identity));
        }
    }
    Ok(())
}

#[cfg(feature = "smoke-tests")]
pub async fn refresh(app: &tauri::AppHandle) -> Result<(), String> {
    let (tx, rx) = std::sync::mpsc::channel();
    let callback_app = app.clone();
    app.run_on_main_thread(move || {
        #[cfg(feature = "smoke-tests")]
        unsafe {
            probe("refresh", None);
        }
        let result = unsafe { refresh_on_ui(&callback_app) }.map_err(|e| e.to_string());
        let _ = tx.send(result);
    })
    .map_err(|e| e.to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        rx.recv_timeout(std::time::Duration::from_secs(10))
            .map_err(|_| "Nedlastingsstatus svarte ikke".to_string())?
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(feature = "smoke-tests")]
static TRACE: std::sync::Mutex<Vec<serde_json::Value>> = std::sync::Mutex::new(Vec::new());
#[cfg(feature = "smoke-tests")]
fn trace(value: serde_json::Value) {
    let mut trace = TRACE.lock().unwrap();
    if trace.len() >= 180 {
        trace.remove(0);
    }
    trace.push(value);
}
#[cfg(feature = "smoke-tests")]
pub fn debug_trace() -> serde_json::Value {
    serde_json::json!(TRACE.lock().unwrap().clone())
}

#[cfg(feature = "smoke-tests")]
unsafe fn probe(event: &str, args: Option<&ICoreWebView2DownloadStartingEventArgs>) {
    let requested_path = args.and_then(|args| {
        let mut raw = PWSTR::null();
        args.ResultFilePath(&mut raw).ok()?;
        take_string(raw).ok().map(PathBuf::from)
    });
    let mut observations = Vec::new();
    let operations = OPERATIONS.with(|items| items.borrow().values().cloned().collect::<Vec<_>>());
    for active in operations {
        let mut state = COREWEBVIEW2_DOWNLOAD_STATE_IN_PROGRESS;
        let mut reason = COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_NONE;
        let mut received = 0;
        let mut total = -1;
        let mut resumable = windows::core::BOOL(0);
        let state_ok = active.operation.State(&mut state).is_ok();
        let reason_ok = active.operation.InterruptReason(&mut reason).is_ok();
        let bytes_ok = active.operation.BytesReceived(&mut received).is_ok();
        let total_ok = active.operation.TotalBytesToReceive(&mut total).is_ok();
        let resume_ok = active.operation.CanResume(&mut resumable).is_ok();
        let mut raw = PWSTR::null();
        let native_path = active
            .operation
            .ResultFilePath(&mut raw)
            .ok()
            .and_then(|_| take_string(raw).ok())
            .map(PathBuf::from);
        observations.push(serde_json::json!({"id":active.id,"state_ok":state_ok,"state":state.0,"reason_ok":reason_ok,"reason":reason.0,"bytes_ok":bytes_ok,"received":received,"total_ok":total_ok,"total":total,"resume_ok":resume_ok,"can_resume":resumable.as_bool(),"canonical_path_matches":native_path.as_ref()==Some(&active.path),"new_path_matches":requested_path.is_some()&&native_path==requested_path}));
    }
    trace(serde_json::json!({"event":event,"operations":observations}));
}

#[cfg(feature = "smoke-tests")]
unsafe fn trace_navigation(core: &ICoreWebView2, view_id: &str) -> windows::core::Result<()> {
    use webview2_com::{NavigationCompletedEventHandler, NavigationStartingEventHandler};
    let starting_view = view_id.to_owned();
    let mut token = 0;
    core.add_NavigationStarting(&NavigationStartingEventHandler::create(Box::new(move|_,args|{
        if let Some(args)=args{
            let mut id=0;let mut redirected=windows::core::BOOL(0);let mut user=windows::core::BOOL(0);
            args.NavigationId(&mut id)?;args.IsRedirected(&mut redirected)?;args.IsUserInitiated(&mut user)?;
            trace(serde_json::json!({"event":"navigation_start","view":starting_view,"navigation":id,"redirected":redirected.as_bool(),"user":user.as_bool()}));
        }Ok(())
    })),&mut token)?;
    let completed_view = view_id.to_owned();
    core.add_NavigationCompleted(&NavigationCompletedEventHandler::create(Box::new(move|_,args|{
        if let Some(args)=args{
            let mut id=0;let mut success=windows::core::BOOL(0);let mut error=COREWEBVIEW2_WEB_ERROR_STATUS_UNKNOWN;
            args.NavigationId(&mut id)?;args.IsSuccess(&mut success)?;args.WebErrorStatus(&mut error)?;
            trace(serde_json::json!({"event":"navigation_complete","view":completed_view,"navigation":id,"success":success.as_bool(),"error":error.0}));
        }Ok(())
    })),&mut token)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discarded_content_length_attempt_does_not_remain_active() {
        let mismatch = COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_SERVER_CONTENT_LENGTH_MISMATCH;
        assert_eq!(
            classify(COREWEBVIEW2_DOWNLOAD_STATE_IN_PROGRESS, mismatch, 0, false),
            ("failed", false)
        );
        // A genuine retry starts transferring: the stale reason must not hide it.
        assert_eq!(
            classify(
                COREWEBVIEW2_DOWNLOAD_STATE_IN_PROGRESS,
                mismatch,
                128,
                false
            ),
            ("downloading", false)
        );
        assert_eq!(
            classify(COREWEBVIEW2_DOWNLOAD_STATE_INTERRUPTED, mismatch, 0, true),
            ("failed", true)
        );
    }

    #[test]
    fn resumed_download_and_completed_empty_file_are_not_failed() {
        assert_eq!(
            classify(
                COREWEBVIEW2_DOWNLOAD_STATE_IN_PROGRESS,
                COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_USER_PAUSED,
                0,
                false
            ),
            ("downloading", false)
        );
        assert_eq!(
            classify(
                COREWEBVIEW2_DOWNLOAD_STATE_IN_PROGRESS,
                COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_NONE,
                0,
                false
            ),
            ("downloading", false)
        );
        assert_eq!(
            classify(
                COREWEBVIEW2_DOWNLOAD_STATE_INTERRUPTED,
                COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_USER_PAUSED,
                32,
                true
            ),
            ("paused", true)
        );
        assert_eq!(
            classify(
                COREWEBVIEW2_DOWNLOAD_STATE_COMPLETED,
                COREWEBVIEW2_DOWNLOAD_INTERRUPT_REASON_SERVER_CONTENT_LENGTH_MISMATCH,
                0,
                false
            ),
            ("complete", false)
        );
    }
}
