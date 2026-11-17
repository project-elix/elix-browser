# Alpha 6 — 0.1.2-dev.6

Alpha 6 is intended for the Developer channel. Its version is higher than
published Beta 1 so the updater can offer it to users who select Developer.
Beta and Stable channel metadata are unchanged by an Alpha 6 publication.
The public installer and updater metadata must come from the protected signed
release workflow; ordinary CI installers are development test artifacts.

## Features

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
  behavior still needs manual acceptance testing.
- Each window has a separate in-memory session and persisted session key. They
  share the selected profile's ordinary site data, bookmarks and permission rules.
  Settings → Profiles → Recover tabs from other windows restores saved tabs from
  windows whose owner process is no longer running. Private tabs are never included.

## Accounts and sync

Email one-time-code login uses Brevo SMTP on the existing ELIX server. Provider
secrets never enter React or the Windows executable. A desktop session token is
stored with Windows DPAPI, bound to its profile and configured service origin.
Credential replacement is atomic; damaged credentials allow a fresh login
without deleting local profile data.

The Node 24 account service is deployed at **https://medlem.projectelix.no** through
isolated Nginx routes, with **no-reply@projectelix.no** as sender. The existing
membership application keeps its routes. HTTPS health/config checks and verified
SMTP TLS/authentication passed on 17 November 2026. One authorized email-code
request was accepted by the SMTP relay; actual inbox receipt and a complete
Windows login/two-device sync flow remain unverified.

Windows CI and signed release builds default to this verified HTTPS origin.
A nonempty `ELIX_ACCOUNTS_URL` repository variable overrides it. Local builds
without an embedded origin show account login as unavailable. No provider secret
is a desktop build variable. Account linking is not implemented.

The service uses a separate restricted system identity and database. Its
environment file is root-owned with mode 0600, state directory mode 0700 and
SQLite files mode 0600. Missing production mail configuration fails startup.
Provider errors are sanitized, SMTP requires verified TLS and authentication, and
failed mail requests remove their login challenge. See
[`services/accounts/README.md`](../services/accounts/README.md) for deployment,
transport checks and remaining acceptance work. Brevo's HTTPS API remains an
alternative transport.

Discord login remains disabled. It requires a native callback bound to the
initiating ELIX instance before it can be enabled.

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
uses native mouse input to drag out after a single fast movement, checks shared
cookies and transferred tab metadata, verifies saved-session recovery,
and checks that a failed child startup preserves the source tab.

Compilation and automated tests do not establish physical multi-monitor,
mixed-DPI or complete account-flow acceptance. Inspect the production signed
installer's screenshot as part of publication.

Schema v2 adds bookmark icons, stored site permissions and session backups. A v1
SQLite backup is made before migration. Older Beta 1 refuses this newer schema;
there is no in-place downgrade claim. Do not test by deleting application data.

## Verification — 17 November 2026

[Windows CI 44](https://github.com/project-elix/elix-browser/actions/runs/44064815199)
passed for account-activation commit
`1cf51ef024a5427716dadf42f83b2ddec98c96f9`:

- Frontend tests/build, version checks, 29 account/deployment tests and 23 Rust
  tests passed.
- The NSIS test installer built, installed and started. Native WebView2 checks
  passed, including tab transfer/recovery, sidebar and content bounds, profile
  isolation, downloads and remote IPC restrictions.
- The idle CPU/memory budget check passed. This is not a comparison with other browsers.
- Build logs confirmed `ELIX_ACCOUNTS_URL=https://medlem.projectelix.no`.
- The deployed service passed public HTTPS health/config checks and SMTP
  authentication. Provider acceptance is not proof of inbox delivery.

The protected release workflow separately runs native verification and a
disposable-key signed installer upgrade round trip, then builds the production
installer with the existing permanent updater key. That round trip builds both
test versions from the current source; it does not prove an upgrade from the
actual published Beta 1 binary. The schema-v1 fixture test separately verifies
migration backup and bookmark preservation.

Remaining manual acceptance: physical second-monitor drag with negative
coordinates/mixed DPI, Windows Default Apps selection, actual inbox delivery,
Windows login/logout/relogin and two-device encrypted sync, including wrong
passphrases and competing edits. These checks have not been claimed complete.

Tauri updater signatures verify updates inside ELIX. Windows Authenticode signing
is not configured, so Windows publisher/SmartScreen warnings may still appear.
