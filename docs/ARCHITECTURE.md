# Architecture and trust boundaries

One native Tauri Window (`main`) contains a trusted local React WebView (`shell`) and an untrusted native child WebView for each live external tab. Child views have distinct `tab-UUID` labels. Only one is visible. Internal `elix://` pages are logical UI routes, not globally registered protocols or remote content. Child bounds come from the shell's actual content rectangle, with DPI handled through logical coordinates.

The unstable feature is required for `WebviewBuilder`, `WindowBuilder` and `Window::add_child` in Tauri 2.12.5. View creation runs from asynchronous commands or tasks, as synchronous Windows command/event creation may deadlock. A future CEF implementation replaces the engine adapter and Windows-native hooks; tab, storage and UI models remain separate. This is an architectural seam, not a claim of a ready drop-in CEF port.

## Privilege isolation

- Capabilities target exactly `shell`, not a window wildcard; no remote URL is authorized.
- The custom Rust dispatch command independently checks the calling WebView label AND local shell origin. All page-origin invokes are refused even if a Tauri invoke bridge is reachable.
- Shell navigation is restricted to bundled local content (and the exact development host in debug builds). Remote page navigation permits HTTP/HTTPS only.
- No executable shell command or arbitrary file read/write API is exposed to pages. Show download folder uses a recorded destination with an argument array; it never executes downloaded files.
- Site permissions for camera/microphone/location are denied pending a proper origin-bound consent UI. No fake consent dialog is presented.
- Production CSP forbids remote shell scripts, objects and frames. React escapes untrusted titles and release notes.

## Storage

The stable application identifier is `no.projectelix.browser`. Under the OS-provided local app data directory: `elix.sqlite` contains profiles/bookmarks/history/settings/sessions; `profiles/<UUID>/webview2/` isolates browser data. Database migrations use transactions, foreign keys, WAL and a pre-migration SQLite online backup when upgrading an existing schema. A database from a newer schema is rejected instead of destructively downgraded. Do not change the identifier or delete this directory in installers.

The current schema is v1. No SQL schema change is needed for patch 0.1.1; the reopen preservation test is not the installer upgrade acceptance test. History listing is limited to the newest 1000 records per query. History itself is retained until the user clears it. Download and diagnostic event lists are session-local.

## Measurements

`sysinfo` samples the application and observed descendants every five seconds, retaining at most 120 samples. CPU is normalized against logical CPU count. Summed working sets are not exclusive physical memory; shared pages may be counted multiple times. Processes may serve several tabs and cannot be attributed accurately to individual tabs.

Native navigation events expose top-level HTTP status and load duration. DNS/TLS/cache/subresource byte totals are not bridged into the dashboard. The full native DevTools Network panel is the correct tool for those inspections.

Logs contain fixed event codes, not arbitrary URLs, error bodies, passwords or file paths. Diagnostic export includes an explicit consent flag and OS save dialog. Export omits tab records and profile names. No upload service is configured.

## Sources checked 2026-11-16

- https://docs.rs/tauri/2.12.5/tauri/webview/struct.WebviewBuilder.html
- https://docs.rs/tauri/2.12.5/tauri/ (unstable feature)
- https://v2.tauri.app/plugin/updater/
- The locked Tauri/Wry/WebView2/Updater crate sources were inspected, and the Windows target is type-checked.

## Native download lifetime

The Windows adapter retains COM download operations until completion/interruption, reads their initial state, and observes future state changes. It identifies concurrent downloads by the engine result path, not just URL. This avoids relying solely on a temporary operation reference. Context: https://github.com/MicrosoftEdge/WebView2Feedback/issues/2180. Navigation failure is not treated as proof of download failure.
