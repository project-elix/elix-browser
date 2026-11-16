# ELIX Browser Alpha 3 — 0.1.0-dev.3

## Changes

- Permanent, encrypted Tauri updater signing key configured in the protected GitHub `release-signing` environment. Releases require Windows/native tests, a real signed upgrade test and approval before signing and publication.
- Smaller start-page and sidebar logo. Removed decorative logos from the top-right corner, internal tabs and settings; tabs use a neutral home icon. The title-bar drag area remains functional.
- Settings offers **Motta alpha-oppdateringer** for this alpha when the current channel is Stable. This explicitly opts into the experimental Developer channel. Existing preferences are preserved until the user chooses to change them.
- Existing custom colors, five theme presets and native window controls remain available.

## First installation and later updates

Install Alpha 3 manually once over Alpha 1/2, without uninstalling or deleting application data. Earlier alphas did not contain the permanent updater public key and cannot discover this update automatically.

Open **Innstillinger → Motta alpha-oppdateringer** to select Developer. Future higher Developer versions can then be offered by the update checker. Downloaded packages must pass the permanent-key signature check and signed-version verification before installation. Installation is a user choice; this is not a silent forced update. No separate ELIX server is required.

Stable is still unreleased. A check on a channel without published metadata can report that it could not check; this does not remove data or install anything. Changing back to Stable does not downgrade an installed alpha.

## Signing scope

The updater signature is free and protects updates inside ELIX. This release does **not** have a Windows Authenticode publisher certificate; Windows may still display unknown-publisher or SmartScreen warnings. No paid signing service has been ordered. See [Windows signing readiness](WINDOWS-SIGNING.md) for the free SignPath route and remaining owner decisions.

## Verification

Release source: `20b61c2007bc699fdfb9299a7c1d4124d0b6e267`.

[Release workflow](https://github.com/project-elix/elix-browser/actions/runs/43902529321): native Windows verification and the signed 0.1.0 → 0.1.1 upgrade test passed. The upgrade test uses a disposable test key and checks preserved user data, corrupted signatures and version replay. Production signing uses a separate permanent key.

Manual checks remain: Windows display scaling, interrupted installation/power loss recovery, and longer everyday browsing sessions. This remains an experimental alpha, not a qualified stable release.

## Published artifact

https://github.com/project-elix/elix-browser/releases/download/v0.1.0-dev.3/ELIX.Browser_0.1.0-dev.3_x64-setup.exe

SHA-256: `0687a49fd99c9673c50c2bb5b34a9d30d5addde040ce16e748457832e70bffc8` (4547605 bytes). Published 2026-11-16T16:23:54Z.

The public installer bytes match GitHub’s SHA-256 digest. An independent `minisign-verify` check validated the permanent-key signature, signed version and rejection after a one-byte mutation. The Developer manifest and channel index match the actual published URL and signature.

Publication completed in [43906863977](https://github.com/project-elix/elix-browser/actions/runs/43906863977); final metadata address repair in [43907644207](https://github.com/project-elix/elix-browser/actions/runs/43907644207). Original native/upgrade/build gates remained required and the binary was not replaced.
