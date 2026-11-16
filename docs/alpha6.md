# Alpha 6 — 0.1.2-dev.6

Development branch based on published Beta 1. Not yet a signed or published release.
The patch version is higher than Beta 1 so the updater can offer it on Developer
when a qualified Alpha 6 release is eventually published. Beta 1 remains available.

## Implemented in this branch

- Bookmark PNG favicons from WebView2's fetched icon stream, stored in SQLite and
  shown in the toolbar, sidebar and bookmark list. Imported/old bookmarks acquire
  an icon after visiting the page; no third-party favicon service is contacted.
- Ctrl+Shift+T restores the latest ordinary closed tab (20 retained); private tabs
  are excluded, including on restart.
- Import standard browser bookmark HTML. Preview the valid HTTP/HTTPS link count,
  then import. Duplicates are ignored; folders flatten into the current profile.
- Site settings list/delete cookies without exposing cookie values, and manage
  camera, microphone, location and notification decisions per profile and origin.
  The default is Ask. Private tabs and third-party frames still ask separately.
- NSIS registers ELIX as a candidate HTTP/HTTPS browser. The setting opens Windows
  Default Apps so the user makes the actual choice. Links open in an independent
  window with the current profile. Local HTML files are intentionally not registered
  because the browser only permits HTTP/HTTPS remote navigation.
- SQLite session checkpoints, fallback to a prior parseable snapshot and a notice
  after unclean shutdown. Crashed native views can be recreated via Reload.
- Named workspaces, group labels, two native pages side by side, native per-tab mute
  and Ctrl+Shift+A search across open tabs/workspaces.
- Pointer drag reorders tabs. Dragging beyond the window opens a new ELIX window
  at the pointer's screen position. Context menu → New window is also available.
  Transfer uses an anonymous pipe; the original is removed only after the child
  window acknowledges startup. Active downloads block transfer. Moving reloads
  the URL, so unsubmitted form state and in-page navigation history do not transfer.
  Private moves start a fresh private session. Physical two-monitor and mixed-DPI
  acceptance remains required before release.
- Each window has a separate in-memory session and persisted session key. They
  share the selected profile's ordinary site data, bookmarks and permission rules.
  Settings → Profiles → Recover tabs from other windows restores saved tabs from
  windows whose owner process is no longer running. Private tabs are never included.

## Accounts and sync: code ready for integration, service not deployed

Email one-time-code login uses Brevo's transactional API. Provider secrets never
enter React or the Windows executable. A desktop session token is stored with
Windows DPAPI, bound to its profile and configured service origin. Credential
replacement is atomic; damaged credentials allow a fresh login without deleting
local profile data.

Discord login is disabled in both the desktop and service. Review found that the
initial polling flow could let someone forward their authorization link to another
person and collect that person's completed login. It must be replaced by a native
callback bound to the initiating ELIX instance before Discord can be enabled.

The standalone Node 24 service is in `services/accounts`; see its README for the
required server configuration. The account UI is explicitly unavailable until a
real HTTPS `ELIX_ACCOUNTS_URL` is embedded at build time. No Brevo sender, key,
Discord application, callback registration or production domain was invented or
changed. Account linking is not implemented.

Sync is optional and manual: upload or fetch-and-merge bookmarks and appearance.
AES-256-GCM, fresh 16-byte salt/12-byte nonce, PBKDF2-SHA256 at 600,000 iterations.
The passphrase is never sent or saved, and cannot be reset to recover existing data.
The service stores only an encrypted envelope; compare-and-swap revisions reject
stale uploads and deletions. Deletions retain a revision tombstone so an old device
cannot overwrite a later upload after a delete. Downloads merge bookmarks (no deletion propagation) and replace
appearance after decryption/validation. Cookies, history, passwords, private tabs
and account tokens are excluded. This is not automatic continuous synchronization.

## Validation

Local frontend build, JavaScript unit tests, account API integration tests with
mock providers, persistence migration/import/checkpoint tests and Windows cross-target
compilation are used. The native Windows smoke suite additionally exercises reorder,
reopen, mute, split view bounds, the split controls in the rendered shell and cookie
deletion. It also launches an actual child window with an isolated test profile,
checks shared cookies and transferred tab metadata, verifies saved-session recovery,
and checks that a failed child startup preserves the source tab.

A local Chromium visual-check attempt was blocked by the environment's socket
restriction. Compilation and unit tests do not establish visual or multi-monitor
acceptance. No release channel metadata or public download site has been changed.

Schema v2 adds bookmark icons, stored site permissions and session backups. A v1
SQLite backup is made before migration. Older Beta 1 refuses this newer schema;
there is no in-place downgrade claim. Do not test by deleting application data.

## Local verification — 16 November 2026

- TypeScript and Vite production build passed.
- 23 frontend/release unit tests and 5 account-service integration tests passed.
- 13 storage/security/download core tests passed, including v1 → v2 migration,
  atomic invalid-import rollback, duplicate suppression, profile isolation and
  bounded session fallback snapshots.
- Windows GNU target check passed with `--features smoke-tests --tests`.
- Version consistency and `git diff --check` passed.
- Upload and Windows testing were explicitly authorized. The implementation is on
  branch `alpha-6` in [draft PR #1](https://github.com/project-elix/elix-browser/pull/1).
- [Initial Windows CI](https://github.com/project-elix/elix-browser/actions/runs/43957080849)
  built the installer, launched the installed application and passed the native
  WebView2 integration suite. Follow-up review fixes and extended window-transfer
  coverage require a fresh CI run on the updated PR head.
- No signed release, updater channel change or account-service deployment has occurred.

Before release: run Windows CI; inspect the actual UI; test pointer drag out onto
  a second monitor (including negative coordinates and mixed DPI), confirm the
  original tab survives a failed new-window startup, test Windows Default Apps,
  and exercise real provider login/two-device sync once the service is configured.
