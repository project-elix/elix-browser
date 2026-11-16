# ELIX Browser Beta 1 — 0.1.1-beta.1

Beta 1 promotes the Alpha 5 feature set to the Beta update channel. It adds a visible Beta 1 label in the browser. The feature set and limitations are documented in [Alpha 5](alpha5.md).

The patch version increases to 0.1.1 so that Beta 1 is newer than 0.1.0-dev.5 under semantic version ordering. Reusing 0.1.0-beta.1 would sort below Alpha 5 and be rejected by the updater. Downgrade protection remains enabled.

To upgrade from Alpha 5, select **Innstillinger → Oppdateringer → Beta → Se etter oppdateringer**, then install and restart when prompted. The Developer channel retains Alpha 5 until a newer developer release is published.

Beta 1 was rebuilt and signed from its own committed version and published on 16 November 2026. Windows native tests, the signed installed upgrade fixture, a separate usable-UI check of the exact production installer, and independent public signature verification passed. See [validation evidence](VALIDATION.md) and the [published release](https://github.com/project-elix/elix-browser/releases/tag/v0.1.1-beta.1).
