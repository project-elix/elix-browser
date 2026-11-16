# ELIX Browser Alpha 2 — 0.1.0-dev.2

## New in Alpha 2

- The supplied transparent ELIX logo replaces the old image in the sidebar, start page, internal tabs, title bar, settings, website, favicon and Windows application/installer icons. The original proportions are retained.
- Native minimize, maximize/restore, fullscreen and close buttons. F11 toggles fullscreen and Escape leaves it, including while a page WebView has keyboard focus. Closing persists the tab session.
- Five presets: ELIX Midnight, Obsidian, Frost, Forest and Ember. Custom accent and background colors, compact/comfortable spacing, 0–20 px corner radius, start-page gradients and reduced motion.
- Appearance changes preview immediately, save locally, or can be cancelled/reset. Existing Alpha 1 settings deserialize with appearance defaults while retaining the existing preferences.

## Distribution and signing

This alpha uses manual installation. Permanent updater signing credentials are not configured yet, and the binary therefore cannot accept an unsigned automatic update. The protected production release workflow remains gated by native Windows checks and a signed upgrade round trip.

Tauri updater signatures and Windows Authenticode publisher certificates are different. This alpha has no Authenticode publisher certificate. Installing it does not require a personal ELIX server.

## Useful checks on your Windows PC

1. Install over Alpha 1 without uninstalling or deleting its application data. Check bookmarks, history and profile logins.
2. Minimize and restore from the taskbar, maximize/restore, enter fullscreen with the button and F11, leave with Escape, then close and reopen.
3. Repeat F11/Escape while a website has focus. Resize and check that page content stays below the toolbar.
4. Open Settings, select Frost and then a dark preset; choose custom colors. Preview, cancel, save, restart and verify persistence.
5. Try compact spacing and reduced motion. Test 100%, 125%, 150% and 200% Windows scaling; high-DPI visual qualification remains manual.

## Published artifact

Published 2026-11-16 after native Windows verification and a separate installed launch of the distributable without test features.

https://github.com/project-elix/elix-browser/releases/download/v0.1.0-dev.2/ELIX.Browser_0.1.0-dev.2_x64-setup.exe

SHA-256: `15d38bfb3995e6ee0fa269109f15c943ce2557d8325da6943104d5869fb1d7c5`

The public bytes were independently downloaded and matched the CI checksum and GitHub asset digest.
