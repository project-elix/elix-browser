# ELIX account service

Node 24, built-in HTTP/SQLite/crypto and a separately pinned Nodemailer dependency
for SMTP. This service supports Brevo SMTP or its HTTPS API. Local tests inject
local mail and Discord providers or use a loopback SMTP server.

As of 2026-11-17, the service is deployed at **https://medlem.projectelix.no** on the existing ELIX
server. The systemd service is active, loopback `/health` returns HTTP 200, and the
public HTTPS `/v1/config` reports `email:true`, `sync:true`, `discord:false`.
Verified SMTP TLS and authentication checks pass. The environment file is root-owned
with mode 0600; the service's state directory is 0700 and its SQLite files are 0600.
One authorized email-code request through the public HTTPS endpoint was accepted
by the SMTP relay. Inbox delivery and a complete Windows login/sync flow remain
unverified. The deployed runtime is commit `1c60e6e273f4a1bae3de5989cba222f1f729e68f`.

Run tests from the repository root:

```sh
npm ci --prefix services/accounts --omit=dev --ignore-scripts
npm run test:accounts
```

Provision one persistent server with TLS reverse proxy, a protected data directory,
backups and a dedicated unprivileged service account. Copy `.env.example` to a
server-only environment file; set a real HTTPS origin, a cryptographically random
AUTH_PEPPER (at least 32 characters), the existing Brevo credentials and verified sender.
Do not paste secrets into commits, logs or the frontend.

Stable 1 and Alpha 9 use the email-code client. Once a user sets a password,
those clients cannot start another session for that account. Roll out password
enrollment to devices with a password-capable client. Returning to a server
which predates password enrollment would re-enable code-only login for accounts
that require passwords. Use a compatible service build when rolling back.

Alpha 10 adds password sign-in and recovery. Deploy the matching service release
with the existing SQLite database, AUTH_PEPPER and mail configuration. The service
adds its password table at startup. Registration, migration from an email-code
account and password resets require a new eight-digit code delivered to the
account address. The service preserves the existing account ID and encrypted sync
copy. Setting a password revokes previous sessions and codes. Passwords use
scrypt with a distinct 16-byte random salt, N=131072, r=8 and p=1. A password
reset does not recover the separate sync passphrase. Older services still offer
code sign-in in the Alpha 10 client.

To enable Discord, the application owner registers the exact redirect URI
`https://medlem.projectelix.no/v1/auth/discord/callback` in the Discord Developer
Portal and sets `DISCORD_CLIENT_ID` and `DISCORD_CLIENT_SECRET` in the protected
server environment. Restart the updated service and verify that `/v1/config`
advertises `discord:true`. For a different PUBLIC_URL, use that origin with the
same callback path. An installation without both values leaves Discord disabled.
The existing proxy must forward `/v1/auth/` and omit callback query strings from
logs. The desktop requests the `identify` scope and opens the system browser.
The service delivers a short-lived ticket to the initiating instance's ephemeral
127.0.0.1 listener. Redemption also requires that instance's independent random
proof. Test the return to the initiating ELIX window, account selection, denial,
timeout, cancellation and replay on Windows before rollout. Discord accounts use
their provider ID and remain separate from email accounts.

```sh
node --env-file=services/accounts/.env services/accounts/server.mjs
```

The process binds 127.0.0.1:3210 by default. Configure the reverse proxy to overwrite
forwarded client IP headers, set an upstream/body limit of 6 MB and disable query
logging for authentication endpoints before setting TRUST_PROXY=1. Without trusted proxy
handling, requests share the proxy's rate-limit bucket. Keep the SQLite volume
persistent; do not run multiple replicas against separate files. The health route
is `/health`; provider availability is `/v1/config`.

The desktop embeds `ELIX_ACCOUNTS_URL` at build time and rejects non-HTTPS origins
and redirects. Windows CI and signed release workflows default to
`https://medlem.projectelix.no`; a nonempty repository variable overrides that origin.
Local builds without this variable continue to offer local profiles and explain that
account login is unavailable. Existing installers need rebuilding to gain the
service address. No provider secret is a compile-time desktop variable.

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
service. SMTP authentication is verified; inbox delivery and these Windows
acceptance checks remain outstanding.

## Deploy on the existing ELIX Linux server

The live account service shares **`https://medlem.projectelix.no`** with the membership
site through `/elix-accounts-health` and explicit `/v1/*` Nginx routes. Its configured sender is
**`no-reply@projectelix.no`**. SMTP TLS/authentication and acceptance of one test
message passed. Provider acceptance does not establish inbox delivery.

The checked-in setup uses one Node 24 process, systemd and an Internet-facing Nginx
TLS proxy. The deployed service uses the existing hosting account, hostname and
certificate. No separate `accounts.projectelix.no` hostname is needed.

The membership source documents SMTP variables (`APP_SMTP_HOST`, `APP_SMTP_PORT`,
`APP_SMTP_USER`, `APP_SMTP_PASSWORD`, `APP_MAIL_FROM`). Reuse those protected server
values by setting `MAIL_TRANSPORT=smtp` and mapping them to `SMTP_HOST`, `SMTP_PORT`,
`SMTP_USER`, `SMTP_PASSWORD`, and `BREVO_SENDER_EMAIL` respectively. Set
`BREVO_SENDER_NAME=ELIX Browser`. For the existing Brevo relay use
`SMTP_HOST=smtp-relay.brevo.com` and `SMTP_PORT=587`. No API key is required in SMTP
mode. Copy only these mail fields; do not import the membership service's entire
environment or change its existing configuration.

SMTP requires STARTTLS on port 587 (and other explicit ports), or implicit TLS on
465, with certificate verification and at least TLS 1.2. Authentication is required;
there is no plaintext fallback. A send has an 18-second deadline that aborts its
connection, bounded connection/greeting/socket timeouts, and no automatic retry
because a timeout can follow provider acceptance. Provider acceptance does not
prove inbox delivery. Raw SMTP responses, credentials and login codes are never
logged. SMTP credentials remain server-only.

The default `MAIL_TRANSPORT=brevo-api` preserves existing API deployments and uses
`BREVO_API_KEY`, `BREVO_SENDER_EMAIL`, and `BREVO_SENDER_NAME`. An SMTP password is
not an API key. Invalid configuration fails at startup; `/v1/config` reports the
selected provider's configuration, not a live mail check.

The reviewed deployment helper is documented in [deploy/INSTALL-SMTP.md](deploy/INSTALL-SMTP.md).
It copies only the required SMTP fields from protected server configuration and
adds isolated routes to the existing HTTPS vhost. Its first-install mode deliberately
refuses an existing or partial installation. Inspect the existing state and plan
an exact upgrade or recovery instead of repeating a first installation.

The following manual alternative is for a separate, dedicated hostname. Adapt its
Node executable path and certificate locations to that server; do not add a second
vhost for the already deployed membership hostname.

1. Make the chosen hostname point to the existing server and obtain its TLS
   certificate using that server's existing certificate process. Do not redirect
   an existing ELIX website or change its mail DNS records.
2. Install the files from this directory as root-owned, read-only application code:

   ```sh
   # Run from services/accounts in the reviewed repository checkout on the server.
   /usr/bin/node --version  # must be Node 24
   getent passwd elix-accounts || sudo useradd --system --home /var/lib/elix-accounts --shell /usr/sbin/nologin elix-accounts
   sudo install -d -m 0755 /opt/elix-accounts
   sudo install -m 0644 server.mjs config.mjs brevo.mjs smtp.mjs mail.mjs check-mail.mjs check-brevo.mjs package.json package-lock.json /opt/elix-accounts/
   sudo npm ci --prefix /opt/elix-accounts --omit=dev --ignore-scripts
   sudo install -m 0644 deploy/elix-accounts.service /etc/systemd/system/elix-accounts.service
   # First installation only; never overwrite an existing environment file.
   sudo sh -c 'test -e /etc/elix-accounts.env || install -m 0600 .env.example /etc/elix-accounts.env'
   sudoedit /etc/elix-accounts.env
   ```

3. In that server-only file, set `PUBLIC_URL` to the chosen HTTPS origin,
   `DATABASE_PATH=/var/lib/elix-accounts/accounts.sqlite`, `PORT=3210`,
   `TRUST_PROXY=1`, the existing verified `BREVO_SENDER_EMAIL`, and the selected
   transport's credentials as described above. Keep credentials out of chat,
   GitHub variables, logs and the desktop.
   Generate `AUTH_PEPPER` locally on the server with
   `node -e "console.log(require('node:crypto').randomBytes(48).toString('base64url'))"`
   and copy it directly into that protected file. Keep it stable across restarts.
   Production refuses a missing provider, the placeholder pepper or an in-memory DB.
4. Check the selected provider without sending an email:

   ```sh
   sudo /usr/bin/node --env-file=/etc/elix-accounts.env /opt/elix-accounts/check-mail.mjs
   # API mode only, optional: Brevo drops this dummy message, no mail is delivered.
   sudo /usr/bin/node --env-file=/etc/elix-accounts.env /opt/elix-accounts/check-brevo.mjs --sandbox
   ```

   In SMTP mode the first command checks DNS, TCP, verified TLS and SMTP
   authentication only. SMTP cannot verify that Brevo will accept the sender;
   check its approved sender/domain in Brevo and then confirm real delivery.
   In API mode the first command reads `/v3/senders` to check the active sender.
   The optional API command also checks the transactional payload with Brevo's
   `X-Sib-Sandbox: drop` header. Neither creates a login challenge or proves inbox
   delivery. Check the server's allowed IP in Brevo if required by the account.
   Failure diagnostics never print provider responses, credentials or codes.
5. Add a separate Nginx vhost from `deploy/nginx.conf.example`, replacing the example
   hostname and certificate paths. Its `X-Forwarded-For` **overwrites** client input.
   It assumes no extra CDN/load balancer; if one exists, configure trusted client IP
   handling for that actual chain before enabling `TRUST_PROXY`. Keep port 3210
   private. Validate Nginx before reloading it:

   ```sh
   sudo nginx -t
   sudo systemctl daemon-reload
   sudo systemctl enable --now elix-accounts
   sudo systemctl reload nginx
   curl --fail http://127.0.0.1:3210/health
   # Also check /health and /v1/config on the real HTTPS hostname from outside.
   ```

   `/v1/config` should report `email:true, discord:false, sync:true`. This indicates
   configuration only. `/health` indicates process health only; neither tests Brevo
   deliverability. Graceful shutdown finishes in-flight requests before closing SQLite.
   Code is root-owned; the unprivileged service writes persistent data in its protected
   state directory. Production creates database/WAL files with private permissions.
6. Build from `alpha-6` using the Windows verification workflow (`ci.yml`). Windows
   CI and signed release jobs embed `https://medlem.projectelix.no` by default;
   set repository variable `ELIX_ACCOUNTS_URL` only to override the origin.
   Invalid origins fail the release check. An empty variable uses the default;
   disabling this workflow default requires removing its fallback. This is the
   only account-related value the desktop build receives. The older `alpha.yml`
   publisher is pinned to Alpha 2 and is not the Alpha 6 build route.
7. On a Windows test build, request a code for a tester-controlled address, verify
   actual receipt, log in, log out, and log back in. Verify expiry, wrong-code limits
   and code reuse rejection. Test a second local profile and another Windows device.
   Keep public release blocked until this real delivery test passes.

Back up the protected `/var/lib/elix-accounts` database and server environment file
separately with restricted access. Use SQLite's online backup API or stop the service
for a consistent snapshot; copying only `accounts.sqlite` while WAL is active is not
a complete backup. Restore into a separate environment before relying on the backup.
Keep one service instance. Restart after code/config changes; do not replace its data
directory.

Primary API references used:
- https://nodemailer.com/smtp
- https://developers.brevo.com/docs/send-a-transactional-email
- https://developers.brevo.com/reference/get-senders
- https://developers.brevo.com/docs/using-sandbox-mode
- https://github.com/discord/discord-api-docs/blob/main/developers/topics/oauth2.mdx

