# Alpha 8 — 0.1.2-dev.8

Alpha 8 follows the published Alpha 7 (`39c85ec89d133d6745d5473ffb9c27db62f57fdf`) on the Developer channel. Source changes and published installers are separate: check [GitHub Releases](https://github.com/project-elix/elix-browser/releases) for availability.

## Bottom gear

The bottom gear opens quick controls for split view, tabs and workspaces, plus an action to open Settings. Double-clicking the gear opens Settings directly. Internal destinations continue to reuse their own tab, preserving the current webpage. The quick controls stay inside the sidebar; opening them in icon mode temporarily widens it and resizes the native page. Escape and outside clicks dismiss the controls.

## Visible tab dragging

A pressed tab is visibly held. After the drag threshold, a floating card with its icon/title follows the pointer through the tab strip or sidebar and a marker shows the insertion position. Over the native webpage the card stays in browser chrome so it remains visible. Existing workspace and pinned-tab ordering rules still apply. Keyboard reordering remains available. Cancelling a drag does not reorder or open a new window.

## Media-only popout

The live page is retained so its media session can continue. Only a selected existing HTML video/audio element is displayed in the popout; video fills its client area while preserving aspect ratio and audio provides playback controls. The window can be resized, minimized, hidden and returned to its tab. Returning restores the original presentation without deliberately reloading or cloning the media.

Support depends on a usable HTML media element. Same-origin frames and accessible open shadow roots are supported; inaccessible cross-origin players and sites without an extractable element produce an explanation instead of popping out the whole page. Protected streams remain controlled by the website/WebView2. Broad compatibility with individual streaming services is not established by local fixture tests.

## Bookmark groups

Bookmark groups belong to the current profile and appear in the bookmarks page, sidebar, toolbar and side panel. Context actions support editing bookmarks, opening in a new tab, moving between groups, and group creation/rename/removal. Removing a group keeps its bookmarks ungrouped. Empty groups persist. Existing bookmarks start ungrouped.

Group data uses additive SQLite tables while retaining schema version 2; the original schema-v2 database receives a one-time backup before those tables are added. Alpha 6/7 can still read their existing data. The inherited schema-v1 to v2 migration and Beta 1 downgrade limitations remain. Optional encrypted manual sync includes group membership; older clients may omit group metadata.

## Validation

Local validation passed: 54 frontend/release tests, 30 account/SMTP tests, TypeScript checking, the Vite production build and release-manifest validation. Four Rust storage regressions cover persistent groups, old-client compatibility, atomic imports and validation. Chromium interaction tests cover the shell UI and the media presentation script. Native Windows behavior requires Windows CI and signed release qualification; the shell UI harness uses mocked Tauri IPC and cannot establish native rendering, installer behavior or real streaming-site compatibility. Local Chromium launch is restricted in the editing environment, so browser interaction tests run in Windows CI.

## Inherited limitations

See [Alpha 7](alpha7.md#inherited-limits) for inherited constraints. Moving a tab to a separate main window reloads its URL. Miniplayer restoration is a different operation and retains the live view. This release does not add Chrome Web Store installation, CRX support, Discord login or Windows Authenticode signing.
