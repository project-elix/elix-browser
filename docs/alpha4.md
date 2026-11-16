# ELIX Browser Alpha 4 — 0.1.0-dev.4

- Sidebar: icon-only, expanded labels, or hidden. Toolbar menu always restores it. Selection persists in SQLite.
- Settings categories: appearance, profiles, gaming, developer, updates, privacy.
- Alpha names are visible next to the exact installed version.
- Empty/unavailable update channels show an informational state without claiming an installation failed. Connection and parsing failures remain errors.
- Sidebar/theme changes retain a verified pending update; changing the release channel invalidates it.

Validation: frontend build and existing Vitest suite; Rust storage migration and isolation tests; Windows-target compile including smoke tests. Release workflow additionally tests the actual sidebar controls, native WebView bounds, settings categories, engine operations, signed upgrades, and final installer startup.

Distribution: publish only through the signed-release workflow, then refresh Sites with the final immutable installer URL. Alpha 3 users on Developer can request this version in Settings → Updates. Windows Authenticode remains unavailable; Tauri update signatures are enabled.
