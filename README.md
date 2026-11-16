# ELIX Browser

<img src="public/elix-mark.png" width="120" alt="ELIX Browser logo">

Windows desktop browser with a Rust backend, Tauri 2, WebView2, React/TypeScript and SQLite. Norwegian interface; dark blue/cyan ELIX identity.

Product site: https://browser.projectelix.no

**Source version 0.1.2-dev.6 is the Alpha 6 development branch, not a published installer.** See [Alpha 6 implementation and activation status](docs/alpha6.md). This repository contains real native browser integration, not an iframe browser. See [validation status](docs/VALIDATION.md) and [limitations](docs/LIMITATIONS.md) before distribution.

## Beta test for Windows

**[Download Beta 1 for Windows x64](https://github.com/project-elix/elix-browser/releases/download/v0.1.1-beta.1/ELIX.Browser_0.1.1-beta.1_x64-setup.exe)** — version 0.1.1-beta.1, promoting the Alpha 5 feature set. It needs no personal server. Alpha 3/4/5 users can choose **Innstillinger → Oppdateringer → Beta → Se etter oppdateringer**. Install manually over Alpha 1/2 without deleting application data, then select Beta for future signed beta updates. The updater signature is separate from Windows Authenticode; the installer has no Authenticode publisher certificate. See [Beta 1 release notes](docs/beta1.md), [feature limitations](docs/alpha5.md), [validation evidence](docs/VALIDATION.md) and the actual [published releases](https://github.com/project-elix/elix-browser/releases); do not use the source ZIP as an installer.

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
- Real InPrivate tabs, per-request camera/microphone prompts, opt-in native password saving/autofill and unpacked extension management. See [Alpha 5](docs/alpha5.md) for scope and limitations.
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
