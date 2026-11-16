# Validation record

## Executed in the build workspace

- Production React/TypeScript/Vite build: passed.
- Vitest release-channel/input/progress tests: 8 passed.
- Actual Rust storage and URL-security modules, headless Linux harness: 4 passed.
- Windows GNU target type-check (Tauri, WebView2 COM hooks, updater, SQLite): passed using x86_64-pc-windows-gnu. This checks Windows-only COM calls; it does not run Windows.
- Start-page visual inspection: passed in Chromium frontend preview. This validates layout, not WebView2 behavior. The screenshot on the public page is labeled accordingly.

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
