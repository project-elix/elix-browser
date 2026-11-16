# Test ELIX Browser Alpha 1

Alpha 1 is version **0.1.0-dev.1**, distributed as a public GitHub prerelease. It is a Windows desktop application and does **not** require your own server.

## Install

1. Open the repository's Releases page and choose **ELIX Browser Alpha 1 — Windows**.
2. Download `ELIX.Browser_0.1.0-dev.1_x64-setup.exe` from Assets. The source ZIP is not the installer.
3. Run the installer on Windows 11 x64. It installs for your Windows user and fetches Microsoft WebView2 Runtime if needed. Internet access is required for the runtime download and browsing.
4. Launch ELIX Browser from the Start menu.

The alpha installer has no Windows Authenticode publisher signature; Windows may display an unknown-publisher/SmartScreen warning. `SHA256SUMS.txt` accompanies the official installer so you can compare file integrity. The checksum is not a publisher certificate.

## Things to try

- Enter a website address, open multiple tabs, use back/forward/reload and follow popup links.
- Bookmark a page, close/reopen ELIX and inspect history and restored tabs.
- Create another profile and verify that its bookmarks and website logins are separate.
- Download a small known file, then use the Downloads page to open its folder.
- Under Settings, enable Developer Mode; focus a website and press F12 or Ctrl+Shift+I. The tools inspect that website. `elix://developer` displays actual application measurements and fixed event logs.
- Try gaming mode and optional inactive-tab suspension. Suspension reloads the page later and can discard forms/playback state; leave it off for important work.

## Updating the alpha

**Automatic ELIX updates are disabled in this build.** Install a newer official package manually when it is published. There is no unsigned update fallback. Permanent update signing is configured separately before an official automatic-update channel opens. WebView2 Evergreen updates are managed by Microsoft.

Bookmarks, history, settings and profile browser data are local. A later installer with the same app identifier retains that data. Keep important information in your usual browser too while evaluating this early alpha. See `LIMITATIONS.md` for missing features and `VALIDATION.md` for actual test coverage.

## Reporting a problem

Create a repository issue with the ELIX version, Windows version, the steps and what happened. Developer Mode can export a local diagnostic JSON after consent. Inspect it before choosing to attach it; nothing is submitted automatically. Do not include passwords, cookies or private browsing content in reports.

## Published artifact

Released 2026-11-16: https://github.com/project-elix/elix-browser/releases/download/v0.1.0-dev.1/ELIX.Browser_0.1.0-dev.1_x64-setup.exe

SHA-256: `374e255adff69498678afc49b4ef1b80f67d3332e10dbd9f0f619ff2e437798f`

GitHub normalizes spaces in the uploaded installer filename to dots. The checksum file records the original build filename; compare the hash or rename the local file before using filename-based checksum tools. The public bytes were downloaded and matched both GitHub's digest and the CI checksum.
