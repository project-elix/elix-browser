<img src="public/elix-mark.png" width="120" alt="ELIX Browser logo">

Windows desktop browser with a Rust backend, Tauri 2, WebView2, React/TypeScript and SQLite. Norwegian interface; dark blue/cyan ELIX identity.

Product site: https://browser.projectelix.no

**Current source: Stable 1 candidate (0.1.4).** This candidate builds on the published Alpha 9 (0.1.4-dev.9). The [Stable 1 qualification checklist](docs/stable1.md) tracks the Windows scenarios and installer evidence required before publication. Beta 2 (0.1.3-beta.2) promotes the [Alpha 8 feature set](docs/alpha8.md).

## Beta test for Windows

The [Beta 2 release page](https://github.com/project-elix/elix-browser/releases/tag/v0.1.3-beta.2) and the [Alpha 9 release page](https://github.com/project-elix/elix-browser/releases/tag/v0.1.4-dev.9) list the Windows x64 installers when published. In ELIX, choose **Innstillinger → Oppdateringer**, select **Beta** or **Developer**, then check for updates. Alpha 1/2 require a manual install. The updater verifies release signatures. Windows may still show an unknown-publisher warning for the installer.

## Run on Windows

Install Node.js 24 LTS, stable Rust (MSVC), Visual Studio 2022 Build Tools with **Desktop development with C++**, and Microsoft Edge WebView2 Evergreen Runtime.

```powershell
git clone https://github.com/project-elix/elix-browser.git
cd elix-browser
npm ci
npm run desktop
```

Build a development installer (no updater key configured):

```powershell
npm run tauri build -- --bundles nsis -- --locked
```

The NSIS installer is written to `src-tauri/target/release/bundle/nsis/`. Ordinary CI also uploads a development installer artifact after its checks. This installer is not an official signed update.

## Implemented

- Native child WebViews for tabs; navigation, redirects, titles, new-window links redirected to tabs, native back/forward/reload/stop.
- Bookmarks, history, profile records, settings and tab-session persistence in SQLite. Profile-specific WebView2 data directories.
- Native download progress, pause/resume/cancel, per-profile persisted list and Show folder. Files are never automatically opened; interrupted transfers must restart after app exit.
- Local address suggestions, native favicons, pinned tabs, configurable bookmark placement and Ctrl+H history.
- Real InPrivate tabs, per-origin permission controls, opt-in native password saving/autofill and profile-scoped extension management. Alpha 7 adds publisher ZIP import with permission review and Chrome Web Store discovery; direct store installation, CRX import and extension toolbar popups are unavailable. See [Alpha 7](docs/alpha7.md) for scope and validation status.
- Bottom-gear quick controls, visible tab dragging, persistent bookmark groups/context actions, internal pages in separate reusable tabs, Ctrl+Alt+S split view, and a media-only live native miniplayer. Alpha 7's new native regressions cover media fullscreen and real PDF wheel scrolling; see its validation notes before treating these as accepted Windows behavior.
- Start page, gaming dashboard, optional reload-based tab suspension, side panel.
- Native minimize, maximize/restore, fullscreen (F11/Esc) and close controls.
- Five theme presets, custom accent/background colors, density, corner radius and reduced motion; preferences persist locally and older settings remain readable.
- Developer mode enables WebView2 DevTools on page WebViews, F12/Ctrl+Shift+I/context menu Inspect. Local diagnostic dashboard uses process measurements and native navigation events.
- Updater checks at startup and every six hours, channel selection, progress, signature verification, explicit install step and persistent user data. Official updater configuration is embedded at build time, never accepted from a web page.
- Windows CI, signed release workflow, admin publication gate, pause-channel workflow and a fail-closed download website.

## Verify

```powershell
npm test
npm run build
npm run check:release
cargo test --manifest-path src-tauri/Cargo.toml --locked
```

The headless persistence/security harness can also run on Linux:

```bash
cargo test --manifest-path tests/core/Cargo.toml --locked
```

Multi-WebView APIs require Tauri's **unstable** feature. The project pins Tauri 2.12.5 and commits dependency lockfiles. WebView2 remains responsible for web-page rendering and JavaScript; Rust alone does not make websites faster.

## Release administration

GitHub Actions, Environments and Releases are the administration surface. GitHub authentication and repository roles protect it; there is no custom web admin accepting arbitrary executables. [Release setup and operations](docs/RELEASING.md) describes keys, environments, channels, pauses and promotion.

**Before first stable release:** complete the remaining manual Windows acceptance checklist, finalize publisher/privacy contact details, and review [free Windows signing requirements](docs/WINDOWS-SIGNING.md). Protected signing environments and permanent updater credentials are configured, and the automated signed 0.1.0 → 0.1.1 round trip has passed; broader stable qualification remains outstanding.

## Layout

`src-tauri/src/browser/engine.rs` and `browser/windows.rs` are the engine seam. Storage, profiles, downloads, diagnostics and updater are separate Rust modules. The UI lives in `src/`; the public landing-page source in `website/`; CI in `.github/workflows/`; reproducible checks in `tests/` and `scripts/`.
