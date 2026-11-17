# Alpha 7 source limits

This is a real native implementation, but stable everyday-browser qualification remains outstanding. See [Alpha 7](alpha7.md) for current source scope and validation status, and [Alpha 6](alpha6.md) for prior acceptance evidence.

| Area | Current behavior / limit |
|---|---|
| Platforms | Windows x64 is the target. Linux is only used for headless tests and frontend checks. ARM64/macOS/Linux desktop unsupported. |
| Multi-WebView | Uses Tauri's explicitly unstable API; lockfiles and a pinned Tauri version reduce accidental drift. |
| Windows | Tab detach launches another process/window; transfer reloads the URL rather than preserving live page state. Physical multi-monitor/mixed-DPI acceptance remains pending. The miniplayer reparents the current live view instead of detaching it. |
| DevTools | Actual WebView2 tools, not React replicas. Runtime-version-dependent DevTools features need Windows acceptance testing. Wry on Windows cannot query or programmatically close the DevTools window; turning the mode off blocks new openings, but existing tool windows must be closed manually. |
| Profiles | Separate UUID data directories, cookies/history/bookmarks. Switching closes old native tabs. Optional account sync manually transfers encrypted bookmarks/appearance, not cookies/passwords/history. Complete two-device acceptance remains pending. |
| Sessions | URLs restored; renderer state, unsaved forms, back-stack and scroll are not serialized. |
| Suspension | Closes inactive native views; resuming reloads the URL. Off by default. Can interrupt audio and discard unsaved state; active/split-visible, private/pinned tabs, miniplayer and download-protected tabs are excluded. Restored lazy tabs are not marked asleep. |
| Downloads | Native byte progress, pause/resume/cancel and per-profile persisted history. Resume requires a live engine object and CanResume; after restart an incomplete transfer must start again. Private metadata is excluded; files remain. |
| Navigation | HTTP/HTTPS only. Custom external protocols are blocked. Popups become tabs; opener-dependent login flows need testing. |
| Permissions | Camera/microphone/location/notifications have per-profile, per-origin decisions. Private tabs and third-party frames ask separately. Other permissions remain denied. |
| Security features | No custom phishing/malware service; no claim of complete equivalence to Edge/Chrome's browser-level protections. Runtime itself must stay updated. |
| Extensions | Chrome Web Store discovery and trusted publisher ZIP/unpacked-source import with permission review. No direct store installation, CRX signature verification/import, automatic store updates, full Chrome compatibility or toolbar action/popup entry points. Source can remain on disk after native removal. |
| Media/PDF | Miniplayer retains the live webpage and fullscreen uses native bounds. Site-specific media/DRM behavior and native PDF wheel scrolling need Windows evidence; source changes alone do not prove acceptance. |
| Other browser features | Bookmark HTML import, optional encrypted sync and Windows Default Apps handoff are implemented. No individual saved-password editor or print UI. Actual Windows default-browser selection remains manual acceptance work. |
| Diagnostics | CPU/RAM/process tree and top-level load data. DNS/TLS/cache/subrequests and per-tab process attribution unavailable. |
| Crash reports | Process failure event codes only; no automatic dumps or uploads. |
| Resource limits | No fabricated CPU/RAM limiter, FPS boost or performance superiority claims. |
| Admin | GitHub roles + Actions/Environments + Releases. A custom admin web app is not built. |
| Rollout | Three static channel manifests; pause supported. No percentage rollout or cohort server. Already downloaded updates cannot be remotely revoked. |
| Signing | Tauri update signatures required for official update builds. Authenticode code signing is separate and not configured. |
| Publication | Published packages and source preparation are separate; consult GitHub Releases and the channel manifests. Stable qualification is outstanding. Tauri signatures do not remove Windows publisher/SmartScreen warnings. |
