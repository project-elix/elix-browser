# Release setup and operations

## Repository configuration

The `release-signing` and `release-publication` environments have been created in `project-elix/elix-browser`, restricted to the `main` branch, with `andreasdan04` as required reviewer and administrator bypass disabled. Self-review remains permitted so the sole owner can approve a manually started release. Permanent encrypted updater key and password are configured as environment secrets in `release-signing`, together with the `ELIX_UPDATE_PUBLIC_KEY` environment variable. Public-key SHA-256 fingerprint: `515739145370edcde3ba17444835187376930f98e2e182d2f0023535c24ca847`. No Windows Authenticode publisher certificate is configured.

## Key recovery and future setup

1. Retain the configured Environment protection. Before adding more maintainers, require PR checks/reviews on main, protect version tags and require organization 2FA. Add a second reviewer if independent approval is desired. YAML references alone do not create approval policies.
2. The permanent key is already configured. Do **not** generate a replacement for routine releases: existing installations trust the original public key. For a new independent application only, generate a signing key on an owner-controlled machine using the installed, locked Tauri CLI:

   ```powershell
   npm run tauri signer generate -- -w "$env:USERPROFILE\.tauri\elix.key"
   ```

   Keep an encrypted offline backup when provisioning keys. No owner-held offline backup has been delivered by this setup; the configured key and password are currently retained in protected GitHub environment secrets. Do not delete those secrets or rotate the key casually. Do not commit the key or paste it in chat.
3. Store the private-key contents in GitHub Actions secret `TAURI_SIGNING_PRIVATE_KEY`, and its password in `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`, scoped to `release-signing`. Put the public-key contents in environment variable `ELIX_UPDATE_PUBLIC_KEY` in `release-signing`. CI embeds the public key and HTTPS channel base into the Rust binary.
4. Verify the CI installer smoke test, run the Windows acceptance checklist and the signed updater round trip. Complete publisher details/privacy contact; review the free Authenticode route and remaining owner decisions in [WINDOWS-SIGNING.md](WINDOWS-SIGNING.md) before broad public distribution. Tauri signing does not establish SmartScreen reputation.

No private keys are included in this source. The public verification key is available in `docs/update-public-key.txt`. Ordinary development builds use an empty updater configuration and report `unconfigured`; they cannot silently accept unsigned updates.

## Normal release

1. Update version on a branch: `npm run version:set -- 0.1.1 stable`. Beta example `0.2.0-beta.1 beta`; Developer example `0.2.0-dev.1 developer`.
2. Refresh lockfile package versions (`npm install --package-lock-only`, `cargo update --manifest-path src-tauri/Cargo.toml --workspace`). Review the diff, test, merge.
3. Run **Build and publish signed release** from `main`, enter exact committed version, channel and plain-text notes. Inputs are passed as environment variables, never interpolated into shell code.
4. Release qualification first runs the complete native Windows workflow and the disposable-key signed upgrade round trip on the release source. Only after both pass does the protected signing job validate versions/required keys and build a signed NSIS installer and creates a **draft** GitHub Release. Existing published versions are never overwritten.
5. The `release-publication` job waits for configured environment approval. After acceptance, it publishes the immutable version assets, then updates the selected channel metadata. Only then does the application offer the release.
6. Inspect the Actions result, release assets and channel JSON. Test on a separate installed client. The website enables the download only from the official Stable channel metadata.

Updater URLs: `https://github.com/project-elix/elix-browser/releases/download/channels/<channel>.json`. Version assets use immutable `v<version>` tags. `requireSignedVersion` binds the advertised version to the signed package to resist valid-old-package/new-version metadata replay. Only HTTPS artifacts are accepted. The app never overrides the updater's newer-version comparison; returning from Beta to Stable waits for a newer Stable version.

## Pause

Run **Pause update channel**, select the channel and approve through `release-publication`. The workflow removes the manifest and its website discovery entry, so new checks do not offer that release. Existing installed versions and downloaded packages remain; static metadata cannot revoke them. Publish a fixed higher version to resume. A missing channel yields a visible check error and retains the current app/data.

## Administration and failures

- Versions/notes: GitHub Releases.
- Build and signature status: GitHub Actions.
- Authorization: GitHub organization membership, repository write/admin roles and protected Environments.
- Voluntary diagnostics: users export a local redacted JSON file and choose whether to attach it to a support issue. No background collection is installed.
- Gradual rollout is unavailable with this static channel host. Do not present a percentage slider as functional; add an authenticated updater service with persisted cohorts before implementing it.

## Interrupted updates

Downloads are verified by the updater before bytes become installable. Failed downloads/signatures discard staged bytes; the original app and profile stay in place. Installation persists the tab session first. A hard power failure during installer execution still needs testing; recovery is reinstalling the same/newer official package while retaining the app-data directory. Do not claim transactional installer rollback until tested.

## Resume a failed publication without rebuilding

If native verification, the signed upgrade test and the production build all succeeded, but publication failed **before** the draft was published, use **Resume verified draft publication** with the original release run ID, exact version and channel. The job still requires `release-publication` approval and reruns publisher tests. It verifies the original main-branch release workflow, each required successful job, and the draft's exact source commit. It will not publish a draft from another commit or overwrite a published release. No signing secrets or arbitrary executable uploads are involved.

The initial Alpha 3 publication exposed GitHub's published-only lookup by tag. Draft lookup now uses authenticated, paginated release listings. For an already published version that is already the current channel version, enable `repair_channel_metadata` to refresh only its channel JSON/index. The original successful verification/build run and exact source commit are still required. This mode never edits or uploads installer/signature assets and cannot advance or downgrade the channel. Draft URLs are never carried into final metadata: the publisher reloads asset URLs after publication.
