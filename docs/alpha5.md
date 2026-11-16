# ELIX Browser Alpha 5 — 0.1.0-dev.5

## Browser controls

- ELIX logo restored at the upper left; native WebView2 favicons are copied as bounded PNG data, without a third-party favicon service.
- Address suggestions use local history, frequent visits and bookmarks. Typing is not sent to an external suggestion service.
- Startup has a loading state, actionable errors and retry; native page loads show activity and navigation failures.
- Logg opens with Ctrl+H from both the ELIX shell and websites. Ctrl+J opens downloads; Ctrl+Shift+N opens a private tab.
- Bookmarks can appear under the address bar, in the left sidebar, or be hidden. Pin/unpin individual tabs; regular pinned tabs persist across restart.

## Downloads and privacy

- Download metadata is stored locally per profile and shown after restart. Saved files remain on disk.
- Actual byte progress, pause, resume (when the engine permits it) and cancel operate on the native download object.
- WebView2 cannot restore live download objects after the browser exits. Incomplete downloads are shown as interrupted after restart and can be started again from the site; no nonfunctional resume button is offered.
- Private tabs use an actual InPrivate controller and a separate disposable data directory. They are excluded from saved sessions and history. Their downloaded files and explicitly saved bookmarks remain.
- Camera and microphone requests show the requesting origin and require an explicit decision for each request. Grants are not saved. Navigation, closure and timeout deny unresolved requests. Other permissions remain blocked.

## Passwords and extensions

- Optional native WebView2 password Save/Update prompts and address autofill; both are off until enabled. Private tabs disable both. Saved passwords stay within WebView2's profile storage.
- Turning password saving off prevents new saves; already saved credentials may still autofill. The settings page offers explicit deletion of all saved passwords in the current profile. There is no password-list/export/edit UI.
- Install unpacked extensions from a user-selected folder containing manifest.json; list, enable/disable and remove them per regular profile. Extensions are disabled in private tabs. WebView2 does not provide Chrome Web Store installation or extension toolbar popup buttons, so compatibility varies.

## Validation

Release is gated by frontend checks, Rust persistence/security tests, Windows native smoke tests and a signed installed upgrade round trip. Record actual run results in VALIDATION.md after the checks finish. Build without the smoke-tests feature for distribution. Publish links only after the immutable signed installer is available.
