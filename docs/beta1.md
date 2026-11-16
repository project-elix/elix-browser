# ELIX Browser Beta 1 — 0.1.1-beta.1

Beta 1 promotes the Alpha 5 feature set to the Beta update channel. It adds a visible Beta 1 label in the browser. The feature set and limitations are documented in [Alpha 5](alpha5.md).

The patch version increases to 0.1.1 so that Beta 1 is newer than 0.1.0-dev.5 under semantic version ordering. Reusing 0.1.0-beta.1 would sort below Alpha 5 and be rejected by the updater. Downgrade protection remains enabled.

To upgrade from Alpha 5, select **Innstillinger → Oppdateringer → Beta → Se etter oppdateringer**, then install and restart when prompted. The Developer channel retains Alpha 5 until a newer developer release is published.

Beta 1 must be rebuilt and signed from its own committed version. The release requires the Windows native tests, signed installed upgrade test, production installer startup and signature verification. The existing Alpha 5 executable and signature cannot be relabeled as Beta 1. Actual publication evidence belongs in VALIDATION.md.
