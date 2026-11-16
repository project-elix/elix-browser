# Validation record

## Executed in the build workspace

- Production React/TypeScript/Vite build: passed.
- Vitest release-channel/input/progress tests: 8 passed.
- Actual Rust storage and URL-security modules, headless Linux harness: 4 passed.
- Windows GNU target type-check (Tauri, WebView2 COM hooks, updater, SQLite): passed using x86_64-pc-windows-gnu. This checks Windows-only COM calls; it does not run Windows.
- Start-page visual inspection: passed in Chromium frontend preview. This validates layout, not WebView2 behavior. The public page now uses an actual Windows CI desktop capture, explicitly labeled.

## Windows CI evidence

Run [43874918397](https://github.com/project-elix/elix-browser/actions/runs/43874918397) passed the React tests/build, Windows Rust tests, NSIS build, silent installation and installed application startup. Run [43876238044](https://github.com/project-elix/elix-browser/actions/runs/43876238044) additionally passed the native WebView2 integration test: real navigation/title/status, back/forward, two tabs, native DevTools open call, downloaded file bytes, suspension and bookmark isolation. The signed-upgrade job is separate and must pass before upgrade coverage is claimed.

## Required Windows release acceptance (not passed by a cross-compile)

Use a disposable Windows 11 x64 VM and record runtime version, app commit, installer hash, results and screenshots.

- [ ] Install 0.1.0 for current user; launch without developer tools installed.
- [ ] Visit HTTPS and HTTP pages; redirect, title change, back/forward/reload/stop.
- [ ] Ten tabs, switching, closing active/last tab, popup links, keyboard shortcuts with page focus.
- [ ] Resize, maximize, 100/125/150/200% DPI, sidebar open, update banner visible; native content never covers chrome.
- [ ] Bookmark, history deletion, restart/session restoration, two profiles with distinct cookies and data.
- [ ] Download known test file, two concurrent same-name files, failed transfer, safe folder opening.
- [ ] Toggle Developer Mode; F12/Ctrl+Shift+I/context Inspect bind to correct site; DOM/CSS/JS/Network/Performance/Storage/device emulation function; disabling blocks new openings; close existing DevTools windows manually (Wry cannot close/query them on Windows).
- [ ] CPU/RAM sampling, logs, anonymized export consent/cancel, no URLs/profile names in exported JSON.
- [ ] Site cannot invoke privileged Rust commands or navigate the trusted shell. Test file/data/javascript/tauri protocols and malicious title text.
- [ ] Suspend inactive tab; explain lost renderer/form state; switching restores URL.
- [ ] Camera/microphone denial clearly surfaced; no silent access.

## Mandatory actual update round trip

Use a separate staging repository and owner-controlled test key. Do not reuse the production signing secret in testing.

1. Build signed 0.1.0 with staging public key/HTTPS endpoint. Install it. Create two profiles with cookies, bookmarks, history and settings; record database/schema state.
2. Build signed 0.1.1 with the same identifier/key and publish only after CI. Keep schema v1 for this test.
3. In installed 0.1.0, check → observe 0.1.1 → Later → recheck → download → progress → verified → install/restart.
4. Confirm 0.1.1 executable version, original profiles/cookies/bookmarks/history/settings and restored URLs.
5. Serve a corrupted artifact, wrong signature, unsigned manifest, HTTP URL and a valid old artifact falsely labeled newer. Each must be refused without changing executable/user data.
6. Interrupt download; retry. Deny network; show error and recover. Test installer cancellation and VM power-loss recovery separately.
7. Switch Stable/Beta/Developer; reject mismatched release, never force downgrade. Pause manifest and confirm no new update offer. Already downloaded updates are not remotely revoked.

**This round trip is not claimed complete until these steps are executed on Windows.** SQLite reopen tests are necessary but do not substitute for it.

## Performance

`scripts/measure-performance.ps1` collects process-tree CPU and summed working set to CSV and enforces explicit resource budgets. Use the same VM, runtime versions, power mode, profile state, pages, extensions, warm/cold cache and duration for ELIX/Edge/Chrome. Record at least five repetitions for blank/idle, ten fixed pages and active workload. No comparative results have been collected, so no faster/lighter claim is made.

The Windows CI `smoke-tests` feature launches an actual WebView2 page against a loopback HTTP fixture and checks title, load timing, HTTP status, back/forward, native DevTools open-call acceptance (visual tool inspection remains manual), separate tabs, downloaded file bytes, suspension and profile bookmark isolation. It uses a temporary profile root and exits nonzero on failure. This feature is not included in the official release workflow. The installed-launch and native smoke checks must pass in Actions before they are treated as verified.

`upgrade-test.yml` builds actual 0.1.0 and 0.1.1 NSIS packages using a disposable signing key, serves them on loopback HTTPS with a certificate trusted only in the disposable Windows runner, and invokes the production updater check/download/install code. A feature-gated post-upgrade test verifies the new executable plus seeded profiles/bookmarks/history/settings/profile files and persistent cookies in two isolated WebView2 profiles. It also rejects modified bytes and a signed old installer falsely described as 0.1.1. Test keys and test installers are never published. Real third-party login sessions, visual update interaction, interruptions, cancellation and power loss still require the manual acceptance steps above.

The native smoke test also exercises denial of remote privileged IPC, an interrupted download, and restoration of the active tab after session reload. The CI desktop screenshot is captured from the installed Windows app, not a web preview.

## Alpha distribution scope

Alpha 1 uses the native verification workflow and then builds a separate installer without the `smoke-tests` feature. The distributable itself is silently installed and launched before publication. Automatic ELIX updates are intentionally unconfigured, so an unfinished signed-upgrade test cannot deliver an unverified update to an alpha user. This is not a stable release qualification.

The native download lifetime regression was reproduced on Windows and fixed by retaining each COM download operation until its terminal state. Run [43886856083](https://github.com/project-elix/elix-browser/actions/runs/43886856083) passed the expanded native test, including interrupted download status, remote IPC denial and active-session restoration. The assertion was retained; navigation errors are not substituted for download state.

Alpha 1 (`0.1.0-dev.1`) was built without test features, silently installed, launched successfully and published by the same run on 2026-11-16. The public installer download was independently hashed and matched the CI checksum and GitHub asset digest. Automatic ELIX updates remain disabled; the signed 0.1.0 → 0.1.1 round trip is **not yet qualified**.
