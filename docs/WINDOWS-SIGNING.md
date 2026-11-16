# Windows signing: free route

Status, 2026-11-16: permanent Tauri updater signing is configured. Windows Authenticode signing is **not** configured. No paid account or subscription has been ordered. No SignPath application has been submitted or accepted.

## What is already free

The existing GitHub release pipeline builds ELIX from source, signs updater packages with the permanent ELIX key, and publishes HTTPS channel metadata after protected environment approvals. Users do not need to run an ELIX server. This cryptographic signature authenticates updates inside ELIX; it does not provide a Windows-trusted publisher identity or remove SmartScreen warnings.

## SignPath Foundation readiness

SignPath Foundation offers free signing to eligible open-source projects. The certificate belongs to **SignPath Foundation**, not Project ELIX. Acceptance is discretionary and executable projects need verifiable reputation.

Remaining requirements:

- Owner must choose and approve an OSI-approved license. A public repository alone is not an open-source license. This repository currently has no LICENSE file; none has been added on the owner's behalf.
- Review dependency licenses and explicitly ask SignPath to assess Microsoft WebView2 Evergreen as a system runtime dependency. Do not claim eligibility has been confirmed.
- Verify MFA for every maintainer and establish author/reviewer/approver roles. Existing release environments require owner approval, but this does not prove all SignPath requirements are met.
- Describe source-to-artifact provenance, limit signing to approved ELIX products/versions, and retain manual approval for every signing request.
- Supply honest reputation evidence: published releases, download statistics and independent community use when available. Do not invent adoption or press coverage for a new alpha.
- Finalize privacy contact and the public code signing policy before approval. Only state that SignPath provides signing after acceptance; no sponsorship claim is currently justified.
- Owner must provide the account/notification email and accept the applicable SignPath conditions and data processing consent at submission. Optional marketing consent is unnecessary.

## Application draft — not submitted

Project name: ELIX Browser

Repository: https://github.com/project-elix/elix-browser

Homepage/download: https://elix-browser-download.andreasdan04.chatgpt.site/

Tagline: A Windows browser with a minimal gaming interface and real WebView2 developer tools.

Description: ELIX Browser combines everyday web browsing, locally stored profiles, bookmarks and history, configurable appearance, and developer diagnostics in a Windows desktop application. The project is maintained by Project ELIX and distributes builds through a controlled GitHub Actions release process.

Build system: GitHub Actions, Rust, Tauri 2, React and TypeScript; Windows x64 NSIS installer. Permanent Tauri update signatures are a separate trust mechanism.

Reputation: Early public alpha. Releases and their CI evidence are available in the repository; broader adoption has not yet been independently established.

Maintainer/account email: awaiting owner choice. License: awaiting owner approval. Category: select the form's appropriate browser/internet category. Terms: not accepted. CAPTCHA: not attempted.

After acceptance, integrate SignPath into the controlled build: sign the application executable and final installer as required, verify Windows Authenticode and timestamp, then generate the Tauri signature over the **final** installer bytes. Never modify an installer after generating its updater signature. Test installation, updates and data preservation again before publishing it.

## Alternative: Microsoft Store MSIX

Microsoft documents a free developer-account registration flow and Store signing for MSIX packages. This requires account/identity verification, a separately prepared MSIX package and Store review. It does not sign the current downloadable EXE installer. No Store account, MSIX package or Store listing has been created here.

## Primary sources

- https://signpath.org/terms.html
- https://signpath.org/apply.html
- https://learn.microsoft.com/en-us/windows/apps/publish/partner-center/open-a-developer-account?tabs=company
- https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/code-signing-options
- https://v2.tauri.app/plugin/updater/
