# ELIX account service

Node 24, built-in HTTP/SQLite/crypto, no runtime npm dependencies. This service is
implemented but not deployed. Local tests inject the mail provider and verify that
Discord routes stay disabled; they never send mail or contact a real Discord account.

Run tests from the repository root:

```sh
node --test services/accounts/server.check.mjs
```

Provision one persistent server with TLS reverse proxy, a protected data directory,
backups and a dedicated unprivileged service account. Copy `.env.example` to a
server-only environment file; set a real HTTPS origin, a cryptographically random
AUTH_PEPPER (at least 32 characters), the existing Brevo API key and verified sender.
Do not paste secrets into commits, logs or the frontend.

Discord login is disabled in both the desktop and service, even if old Discord
environment variables are present. Its public polling flow has been removed because
an attacker could forward their authorization link to another browser, then collect
that user's completed login with the attacker-known polling secret.

TODO before enabling Discord: bind an ephemeral native loopback callback and local
state before authorization; deliver a new one-use completion secret only through
that callback; require both that secret and the initiating desktop's proof to finish
login. Test forwarded links, callback/state mismatch, cancellation, timeout and
replay on Windows before activating the provider. A cookie created on visiting a
forwarded authorization link is not sufficient binding.

```sh
node --env-file=services/accounts/.env services/accounts/server.mjs
```

The process binds 127.0.0.1:3210 by default. Configure the reverse proxy to overwrite
forwarded client IP headers, set an upstream/body limit of 6 MB and disable query
logging for authentication endpoints before setting TRUST_PROXY=1. Without trusted proxy
handling, requests share the proxy's rate-limit bucket. Keep the SQLite volume
persistent; do not run multiple replicas against separate files. The health route
is `/health`; provider availability is `/v1/config`.

The desktop build must explicitly embed `ELIX_ACCOUNTS_URL=https://YOUR_ACCOUNT_HOST/`.
It rejects non-HTTPS origins and redirects. An unconfigured build continues to offer
local profiles and explains that account login is unavailable. No provider secret
is a compile-time desktop variable.

Email codes expire after ten minutes and five incorrect attempts. Only HMAC code
verifiers are stored. Login sessions expire after 30 days and logout revokes the
current session. Rate limits persist in SQLite; expired challenges/sessions/limits
are pruned. Corrupt desktop credentials permit signing in again without changing
local profile data; credential replacement uses a flushed temporary file and an
atomic same-directory rename.

Sync stores a versioned encrypted blob per account and uses revision comparisons
for write conflicts. Deletion uses the revision observed before confirmation and keeps
a monotonic revision tombstone with no encrypted blob, so old devices cannot reuse
earlier revisions. The server cannot recover the user's passphrase. SQLite also
contains the account's email or Discord ID, display name and hashed bearer tokens;
backups must be access controlled. Document retention, account deletion/support
contact, provider handling and operational ownership before public activation.

Production acceptance must test real Brevo delivery,
logout/relogin, separate Windows users and two-device encrypted sync, including a
wrong phrase and competing edits. Browser tab/window features do not require this
service. No live delivery or provider authorization has yet been verified.

Primary API references used:
- https://developers.brevo.com/docs/send-a-transactional-email
- https://github.com/discord/discord-api-docs/blob/main/developers/topics/oauth2.mdx
