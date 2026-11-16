# 0.1.0 limits

This is a real native implementation, but stable everyday-browser qualification remains outstanding.

| Area | Current behavior / limit |
|---|---|
| Platforms | Windows x64 is the target. Linux is only used for headless tests and frontend checks. ARM64/macOS/Linux desktop unsupported. |
| Multi-WebView | Uses Tauri's explicitly unstable API; lockfiles and a pinned Tauri version reduce accidental drift. |
| Windows | One main window. The dashboard reports actual window count; additional main windows are not implemented. |
| DevTools | Actual WebView2 tools, not React replicas. Runtime-version-dependent DevTools features need Windows acceptance testing. |
| Profiles | Separate UUID data directories, cookies/history/bookmarks. Switching closes old native tabs. No account synchronization. |
| Sessions | URLs restored; renderer state, unsaved forms, back-stack and scroll are not serialized. |
| Suspension | Closes inactive native views; resuming reloads the URL. Off by default. Can interrupt audio/download-related pages and discard unsaved state. |
| Downloads | Native download requests and completion callbacks; unique destination subfolders. No byte progress, pause/resume or cross-session list. |
| Navigation | HTTP/HTTPS only. Custom external protocols are blocked. Popups become tabs; opener-dependent login flows need testing. |
| Permissions | Camera/microphone/location/etc. denied until an origin-bound permission UI is implemented. |
| Security features | No custom phishing/malware service; no claim of complete equivalence to Edge/Chrome's browser-level protections. Runtime itself must stay updated. |
| Unsupported browser features | No extensions, private browsing, password manager, import, sync, default-browser registration UI, print UI or per-site permission manager. |
| Diagnostics | CPU/RAM/process tree and top-level load data. DNS/TLS/cache/subrequests and per-tab process attribution unavailable. |
| Crash reports | Process failure event codes only; no automatic dumps or uploads. |
| Resource limits | No fabricated CPU/RAM limiter, FPS boost or performance superiority claims. |
| Admin | GitHub roles + Actions/Environments + Releases. A custom admin web app is not built. |
| Rollout | Three static channel manifests; pause supported. No percentage rollout or cohort server. Already downloaded updates cannot be remotely revoked. |
| Signing | Tauri update signatures required for official update builds. Authenticode code signing is separate and not configured. |
| Publication | No official stable package until signing and Windows acceptance are complete. Domain browser.projectelix.no requires owner DNS configuration. |
