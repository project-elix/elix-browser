# ELIX account service

Node 24, built-in HTTP/SQLite/crypto, no runtime npm dependencies. This service is
implemented but not deployed. Local tests inject the mail provider and verify that
Discord routes stay disabled; they never send mail or contact a real Discord account.

Run tests from the repository root:

```sh
npm run test:accounts
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

## Deploy on the existing ELIX Linux server

The selected host is the **same server as `medlem.projectelix.no`**, and the selected
sender is **`no-reply@projectelix.no`**. These are the user's confirmed choices;
they do not indicate that the account service has already been deployed or that a
live Brevo sender check has run.

The checked-in setup uses one Node 24 process, systemd and an Internet-facing Nginx
TLS proxy. It does not provision a new hosting account. Adapt the proxy configuration
to the existing server after confirming its hostname, TLS setup and Node location.
`accounts.projectelix.no` is a possible hostname, not a domain already configured.

The membership source documents SMTP variables (`APP_SMTP_HOST`, `APP_SMTP_PORT`,
`APP_SMTP_USER`, `APP_SMTP_PASSWORD`, `APP_MAIL_FROM`). This browser service currently
uses Brevo's **HTTPS API**, so its `BREVO_API_KEY` must be an API key for the existing
Brevo account, not an SMTP password. Check the actual protected server configuration:
if an API key already exists, supply that value to the new service's protected
environment; if the membership server only has SMTP credentials, an API key must be
configured or an SMTP adapter added before activation. Do not import the membership
service's entire environment into this process. Keep its database, session secrets
and existing mail configuration separate from the browser account service.

Deployment access is still required. The accessible membership repository describes
deployment preparation and does not expose live server configuration or an active
`.github/workflows/deploy.yml`. Reuse the existing deployment mechanism once its
actual location/access is available; no new hosting account is needed.

1. Make the chosen hostname point to the existing server and obtain its TLS
   certificate using that server's existing certificate process. Do not redirect
   an existing ELIX website or change its mail DNS records.
2. Install the files from this directory as root-owned, read-only application code:

   ```sh
   # Run from services/accounts in the reviewed repository checkout on the server.
   /usr/bin/node --version  # must be Node 24
   getent passwd elix-accounts || sudo useradd --system --home /var/lib/elix-accounts --shell /usr/sbin/nologin elix-accounts
   sudo install -d -m 0755 /opt/elix-accounts
   sudo install -m 0644 server.mjs config.mjs brevo.mjs check-brevo.mjs /opt/elix-accounts/
   sudo install -m 0644 deploy/elix-accounts.service /etc/systemd/system/elix-accounts.service
   # First installation only; never overwrite an existing environment file.
   sudo sh -c 'test -e /etc/elix-accounts.env || install -m 0600 .env.example /etc/elix-accounts.env'
   sudoedit /etc/elix-accounts.env
   ```

3. In that server-only file, set `PUBLIC_URL` to the chosen HTTPS origin,
   `DATABASE_PATH=/var/lib/elix-accounts/accounts.sqlite`, `PORT=3210`,
   `TRUST_PROXY=1`, and the existing verified `BREVO_SENDER_EMAIL` and
   `BREVO_API_KEY`. Keep the key out of chat, GitHub variables, logs and the desktop.
   Generate `AUTH_PEPPER` locally on the server with
   `node -e "console.log(require('node:crypto').randomBytes(48).toString('base64url'))"`
   and copy it directly into that protected file. Keep it stable across restarts.
   Production refuses a missing provider, the placeholder pepper or an in-memory DB.
4. Check Brevo credentials and the active sender without sending an email:

   ```sh
   sudo /usr/bin/node --env-file=/etc/elix-accounts.env /opt/elix-accounts/check-brevo.mjs
   # Optional API-format test: Brevo drops this dummy message, no mail is delivered.
   sudo /usr/bin/node --env-file=/etc/elix-accounts.env /opt/elix-accounts/check-brevo.mjs --sandbox
   ```

   The first command only reads `/v3/senders`. The second also checks the exact
   transactional payload with Brevo's `X-Sib-Sandbox: drop` message header. Neither
   creates a login code or proves inbox delivery. Check Brevo's sending-domain
   authentication and the server's allowed IP in Brevo if required by the account.
   Failure diagnostics never print the provider response, API key or code.
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
6. Set GitHub **repository variable** `ELIX_ACCOUNTS_URL` to that same HTTPS origin
   once the service passes verification, then build from `alpha-6`. The Windows CI
   and signed release jobs now embed this public address; invalid origins fail the
   release check. This is the only account-related value the desktop build receives.
   Leave the variable unset until then: existing builds keep local profiles working.
7. On a Windows test build, request a code for a tester-controlled address, verify
   actual receipt, log in, log out, and log back in. Verify expiry, wrong-code limits
   and code reuse rejection. Test a second local profile and another Windows device.
   Keep public release blocked until this real delivery test passes.

Back up the protected `/var/lib/elix-accounts` database and server environment file
separately with restricted access. Use SQLite's online backup API or stop the service
for a consistent snapshot; copying only `accounts.sqlite` while WAL is active is not
a complete backup. Restore into a separate environment before relying on the backup.
Keep one service instance. Restart after code/config changes; do not replace its data
directory. No live server, sender or DNS was modified while preparing these files.

Primary API references used:
- https://developers.brevo.com/docs/send-a-transactional-email
- https://developers.brevo.com/reference/get-senders
- https://developers.brevo.com/docs/using-sandbox-mode
- https://github.com/discord/discord-api-docs/blob/main/developers/topics/oauth2.mdx
