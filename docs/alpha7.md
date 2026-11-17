# Alpha 7 — 0.1.2-dev.7

Alpha 7 is prepared for the Developer channel from the Alpha 6 source at
`c45b587fc6dfb935d9c4c51f41fd8b7b352ace63`. The source version and publication
status are separate: check [GitHub Releases](https://github.com/project-elix/elix-browser/releases)
for available installers. Official installer and updater assets must come from
the protected signed release workflow. Ordinary CI installers are test artifacts.

## Menu and sidebar

- Workspace selection, tab grouping, split view, reopening closed tabs and tab
  search move into the left sidebar, removing their separate horizontal toolbar.
- The menu button shows or hides the sidebar. Its own expand button switches
  between icons and labels. Sidebar mode remains a saved appearance setting;
  hidden mode keeps the toolbar button available to restore it.
- Tab search opens the expanded sidebar. Workspace/group controls in icon mode
  expand it to show the associated inputs.
- The expanded sidebar includes the current workspace's tabs, collapsible group
  headings and existing-group suggestions. Joining a group places a tab beside
  that group. Dragging shows whether the drop is before or after its target;
  Alt+Up/Down provides keyboard reordering within workspace and pinned partitions.
- Ctrl+Alt+S toggles split view from the shell and from a focused native webpage.
  Opening split view needs two eligible webpages in the same workspace and
  private/ordinary context. The sidebar also lets the user choose the second page
  and leave split view. Ctrl+Shift+A and Ctrl+Shift+T keep their tab-search and
  reopen actions.

## Internal pages and address focus

History, Settings, Downloads and the other internal pages open or reuse their own
tab in the current workspace. Opening them preserves the previously displayed
webpage and its live state. The plus button and Ctrl+T focus the address field
after the new tab is created.

## Media and PDF viewing

The miniplayer moves the existing native webpage into a small, resizable window
above other applications. It can be hidden without ELIX stopping its audio,
muted, shown again and returned to its tab without deliberately reloading the page. It is a
live webpage window, not a copied video stream; the website still controls media
playback and protected content behavior.

HTML video fullscreen uses the native window's client area, hides the browser
shell and restores the prior browser layout when fullscreen ends. A native
regression covers fullscreen, split restoration and miniplayer playback/state
continuity. Another regression uses a real three-page PDF and native mouse-wheel
input in both directions. PDF scrolling is a validation target, not a claimed
engine fix before Windows evidence is available.

## Extension discovery and publisher packages

Settings can open or search Chrome Web Store for extension and publisher
information. ELIX cannot install through the store's **Add to Chrome** button.
Instead, a user can import a ZIP of extension source files supplied by a trusted
publisher, or select an existing unpacked extension folder. The native dialog
shows declared API, site and optional permissions before installation into the
selected profile. Extensions remain disabled in private tabs.

ZIP import accepts a root manifest or a single enclosing publisher directory,
stores the source with the profile and rejects unsafe paths, links, encrypted
archives, colliding names and excessive expansion. Limits are 64 MiB compressed,
256 MiB expanded, 32 MiB per file and 10,000 entries. A user-selected folder must
remain in place and unchanged while installed. Imported source can remain on
disk after removal or an uncertain native installation result; removal disables
the installed extension, and does not promise source-file deletion.

This is local source installation, not Chrome Web Store signature verification.
CRX packages are rejected rather than having their signed container stripped.
Automatic store updates, full Chrome API compatibility and extension toolbar
action/popup entry points are unavailable. Store-only extensions without a
publisher-provided source package cannot be installed through this workflow.

The implementation follows [WebView2's unpacked extension API](https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/win32/icorewebview2profile7)
and [Google's extension distribution guidance](https://developer.chrome.com/docs/extensions/how-to/distribute).

## Email login recovery

Pending email challenges retain their address and expiry, focus the code field
after sending, and offer resend/change-address actions. Expired codes and expired
account sessions return to an actionable login state. Successful verification
requires a valid token before reporting login, keeps that token out of the shell
response and stores it for the initiating profile. Profile changes and stale
responses are guarded. These changes do not establish actual inbox delivery or
complete Windows login acceptance by themselves.

## Tab sleep and restoration

Alpha 6 treated remote tabs restored without a native view as sleeping tabs.
That mixed up deferred loading with an explicit sleep action. Persisted old idle
timestamps could also make a newly backgrounded tab immediately eligible for
automatic sleep.

The fix separates these cases: restored and recovered tabs start without a
sleep marker, restart/background transitions reset idle timing, and automatic
sleep considers only an existing native view. Active, split-visible, pinned,
private and download-protected tabs remain excluded as appropriate to the action.
Manual sleep closes only the selected eligible tab's native view; waking it
recreates that view and reloads its URL. It does not suspend a shared renderer
process or deliberately sleep sibling tabs on the same site.

Sleep is still a reload-based feature. It can discard unsaved page state and
interrupt audio. Automatic sleep remains off by default; this change does not
add session serialization or a promise that every page resumes its prior state.

## Validation status at source preparation

The package, Tauri and Cargo manifests and application entries in both lockfiles
use `0.1.2-dev.7`. ZIP import reuses the already locked ZIP 9.6.0 and compression
dependencies; no dependency version upgrade is included. The Windows native
pixel checks enable the existing Windows crate's GDI feature.

Local validation passed: 45 frontend/release tests, 30 account/SMTP tests,
TypeScript checking, the Vite production build and release-manifest validation.
The native regressions await Windows CI at source preparation. The native acceptance
scope includes Ctrl+Alt+S while a webpage has focus, sidebar/content bounds,
restored tabs without false sleep markers, automatic-sleep eligibility, repeated
sleep/wake, and a same-origin sibling whose JavaScript state/timer keeps running
when another tab sleeps. These are validation targets until their results are
recorded; prior Alpha 6 passes do not establish Alpha 7 behavior.

Expanded native coverage also targets internal-page reuse without destroying the
original page, new-tab address focus, before/after reorder, real PDF wheel
scrolling, HTML fullscreen, live miniplayer reparenting, and ZIP import followed
by the native extension install/enable/disable/remove lifecycle. Seven Rust
package tests cover manifest permissions, extraction safety, expansion limits,
CRX rejection and source retention. These Rust/native tests have not run in the
local environment, which has no Rust toolchain.

Before publication, run the complete frontend/account/Rust suites, the Windows
native installer checks and signed upgrade round trip. Inspect the production
signed installer's screenshot after installation. A source build or a successful
startup alone is not full Windows acceptance.

## Inherited limits

The [Alpha 6 implementation and acceptance notes](alpha6.md) continue to apply:

- Windows x64 is the desktop target. Physical second-monitor drag with negative
  coordinates/mixed DPI and Windows Default Apps selection still need manual
  acceptance. Moving a tab reloads its URL; unsaved forms and navigation history
  do not transfer, and private moves start a fresh private session.
- Email login uses the deployed HTTPS account service and Brevo SMTP. SMTP
  authentication and relay acceptance have been checked, but actual inbox receipt,
  complete Windows login/logout/relogin and two-device encrypted sync remain
  manual acceptance work. Discord login and account linking are unavailable.
- Sync is optional and manual. It merges bookmarks and replaces appearance after
  decrypting the encrypted envelope; it excludes cookies, history, passwords,
  private tabs and account tokens. There is no passphrase recovery or automatic
  continuous synchronization.
- SQLite schema v2 migration backs up a schema-v1 database. Older Beta 1 refuses
  the newer schema; an in-place downgrade is not supported. Testing must preserve
  application data. The automated two-version upgrade fixture builds both test
  versions from current source, so it does not prove an upgrade from the actual
  published Beta 1 binary.
- Tauri updater signatures verify updates inside ELIX. Windows Authenticode
  signing is not configured; publisher/SmartScreen warnings may still appear.
  Stable everyday-browser qualification remains outstanding.
