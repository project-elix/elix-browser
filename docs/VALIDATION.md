# Validation record

## Executed in the build workspace

- Production React/TypeScript/Vite build: passed.
- Vitest release-channel/input/progress, theme contrast and publisher recovery tests: 14 passed.
- Actual Rust storage and URL-security modules, headless Linux harness: 6 passed (including Alpha 1 preference compatibility and theme persistence).
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

Alpha 1 (`0.1.0-dev.1`) was built without test features, silently installed, launched successfully and published by the same run on 2026-11-16. The public installer download was independently hashed and matched the CI checksum and GitHub asset digest. Automatic ELIX updates remain disabled; at that time the signed round trip was not yet qualified (subsequent result below).

## Alpha 2 verification — 2026-11-16

- Run [43897753259](https://github.com/project-elix/elix-browser/actions/runs/43897753259) passed the new native minimize/maximize/restore/fullscreen/close path alongside existing real browsing, DevTools, download, profile and IPC tests. Its desktop capture shows the supplied ELIX logo and visible window buttons.
- Run [43897753542](https://github.com/project-elix/elix-browser/actions/runs/43897753542) **passed the actual signed 0.1.0 → 0.1.1 round trip**: installed old NSIS package, production check/download/signature verification/install, automatic restart into the new executable, preserved two profiles, bookmarks, history, preferences, persistent WebView2 cookies and profile files. Corrupted bytes and a correctly signed old installer replayed as a newer version were rejected.
- The first attempted test hung at the interactive CurrentUser root-certificate trust dialog. The corrected fixture uses the LocalMachine root store on its disposable hosted Windows runner and removes the ephemeral certificate afterward. TLS verification remains enabled. No test certificate or test signing key is used in distributed alpha builds.
- The automatic round trip qualifies the tested path, not the whole manual acceptance matrix. Third-party logins, UI interaction, high-DPI visuals, installer interruption/cancellation and power loss still need manual acceptance.
- At Alpha 2 publication, permanent production signing credentials and a Windows Authenticode publisher certificate were not configured. Passing a disposable-key test alone does not activate a public updater channel. See the Alpha 3 record below for the later permanent-key configuration.

Alpha 2 publication run [43897911998](https://github.com/project-elix/elix-browser/actions/runs/43897911998) passed native verification, built the distributable without test features, silently installed/launched it, and published `0.1.0-dev.2`. Public bytes were independently hashed and matched both CI and GitHub digests.

## Alpha 3 verification — 2026-11-16

Release source `20b61c2007bc699fdfb9299a7c1d4124d0b6e267`, run [43902529321](https://github.com/project-elix/elix-browser/actions/runs/43902529321). Both native Windows verification and the separate signed upgrade round trip passed on this source. The upgrade result explicitly verifies profiles, bookmarks, history, settings, cookies and profile files, with tampered/replayed packages rejected. Permanent production updater credentials have now been configured separately from those disposable test credentials. Windows Authenticode remains unconfigured.

The signed production distributable installed and started successfully. Independent public-byte signature, signed-version and tamper-rejection checks passed. The initial publication needed fixes for draft lookup and post-publication asset URLs; gated recovery runs 43906863977 and 43907644207 completed successfully. The final Developer JSON and release index were checked against the published immutable installer URL and signature. Remaining manual acceptance limits above still apply.

## Alpha 5 verification — 2026-11-16

Source `ff4b710450c714759d73822b38211e9c86ab0c5a`, Windows run [43926494207](https://github.com/project-elix/elix-browser/actions/runs/43926494207): **passed** 19 JavaScript tests, 15 Windows Rust tests, production frontend build, NSIS installation/startup, native integration and the idle resource budget.

Native checks exercised sidebar expansion/hiding and content bounds, settings categories, both bookmark placements, real Ctrl+H keyboard input with website focus, navigation/title/status/favicon, loading failure/recovery, private cookie/history/session isolation, native extension lifecycle and password preferences, microphone prompt/denial, remote IPC rejection, pinned session restoration and profile isolation. Download checks verified actual 8 MiB pause/resume bytes, separate simultaneous downloads of the same URL, persisted records, and active/paused cancellation that aborted the HTTP transfer before its full payload arrived.

The native regression reproduced a discarded zero-byte response whose WebView2 State remained IN_PROGRESS while InterruptReason reported SERVER_CONTENT_LENGTH_MISMATCH and CanResume was false. The application now interprets that exact combination as failed; genuine later byte progress can recover. Successful explicit cancellation remains terminal despite stale native callbacks. Tests verified classification before manual cancellation and independently verified that cancellation stops network transfer. Distinct native attempts are not merged merely because their URLs match.

The idle check measured 0% mean process-tree CPU and 336.1 MB peak summed working set in that runner. This is only a budget check, not a performance comparison. Exact WebView2 runtime version was not recorded in the successful job log. The remaining manual acceptance matrix above still applies.

Signed release run [43926787374](https://github.com/project-elix/elix-browser/actions/runs/43926787374) passed native verification and the actual signed 0.1.0 → 0.1.1 installation/restart round trip on the same source. Profiles, bookmarks, history, settings, cookies and profile files survived; corrupt and replayed installers were rejected. The production build excluded smoke-test features and its signed distributable installed and launched successfully. Its actual Windows capture is [alpha5-windows.png](images/alpha5-windows.png).

The immutable [Alpha 5 release](https://github.com/project-elix/elix-browser/releases/tag/v0.1.0-dev.5) was published at 2026-11-16T18:27:19Z. The public 4,626,415-byte installer had SHA-256 `c9d72a7a903cb1218cc7969179443175f49d91a3a26514897330eff3c4b30cc5`, matching GitHub's asset digest. Independent verification passed against the permanent public key, the signed version `0.1.0-dev.5`, and a tampered copy. Developer channel metadata matched the final immutable installer URL and signature. Windows Authenticode remains unconfigured.

Sites version 8, source `488cdaaf0208c1fa50ee2466afff37ed15d6b774`, was published successfully with the verified Alpha 5 download, actual signed-build Windows capture and the new settings descriptions. The live configuration section was visually checked; [publication capture](images/alpha5-site.jpg).

## Beta 1 verification — 2026-11-16

Source `71da83cc856978398d0daaabd77c4e30e95ba14d` promotes the Alpha 5 feature set to `0.1.1-beta.1` on the Beta channel. The patch bump makes Beta 1 newer than `0.1.0-dev.5`; regression tests verify that promotion and reject the reverse downgrade. The browser displays “Beta 1” alongside the complete version.

Release run [43931804293](https://github.com/project-elix/elix-browser/actions/runs/43931804293) passed native Windows verification: 20 JavaScript tests, 15 Rust tests, frontend build, NSIS installation/startup, the complete Alpha 5 native integration suite and the idle resource budget. The runner measured 0% mean CPU and 337.9 MB peak summed working set; this is not a browser performance comparison. Local frontend tests/build, release-version validation and Windows cross-target compilation also passed.

The same run passed the signed 0.1.0 → 0.1.1 installed upgrade fixture. Profiles, bookmarks, history, settings, cookies and profile files were preserved; corrupt and replayed installers were rejected. This fixture validates the production updater path with disposable test signing credentials; it does not claim an interactive Alpha 5 → Beta 1 user acceptance run. The existing manual acceptance limits remain.

The fixed 12-second desktop capture from the production build was blank, so publication was held until a separate installed-UI check completed. Run [43941832449](https://github.com/project-elix/elix-browser/actions/runs/43941832449) downloaded the exact existing signed draft (asset `756036552`) and verified its source and SHA-256 before installation. Windows UI Automation detected the enabled, visible address bar and start-page search after 6.1 seconds; the completed capture/check logged 7.3 seconds. Its [actual Windows capture](images/beta1-windows.png) and accessibility tree show “Beta 1 · 0.1.1-beta.1”. No rebuild or artifact substitution occurred. This confirms usable startup on that runner; it is not a general startup-time benchmark.

The owner explicitly authorized one protected review execution with temporary draft-read access. The one-off workflow was removed from `main` in commit `4679954e60c3d91a3a024a37dee370c97ccd63a5` immediately after the successful review. Existing release signing and publication approval gates remain unchanged.

The immutable [Beta 1 release](https://github.com/project-elix/elix-browser/releases/tag/v0.1.1-beta.1) was published at 2026-11-16T19:45:26Z. The public 4,624,663-byte installer has SHA-256 `24701de2eef2fbd23cbd9be59ddd0280de5ded0c1ac16df55907b404ae700274`, matching both the reviewed draft and GitHub asset digest. Independent public-byte verification passed against the permanent update public key, the exact signed version and a deliberately altered copy. The Beta manifest and release index match the final immutable installer URL and signature. Developer remains on `0.1.0-dev.5`; Stable has no release. Windows Authenticode remains unconfigured.

Sites version 9, source `1850ee0b73ffef1d775aa098c9693e5494db7cfb`, was published successfully at [browser.projectelix.no](https://browser.projectelix.no/) with the verified Beta 1 installer link, current configuration descriptions, upgrade instructions and the actual installed Beta 1 Windows capture. The live download section was visually checked; [publication capture](images/beta1-site.jpg).
