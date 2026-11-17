# First SMTP installation

`install-smtp.mjs` is a first-install-only operator helper. Run it with the existing
Node 24 executable. Download the helper from a reviewed, full commit SHA, then use
that same SHA for `install -r`. It fetches only the account service's listed files
from that exact revision. It never reads database rows, logs credentials, changes
membership settings, sends mail, or changes DNS.

Use source environment files in their real override order. Each must be a regular,
root-owned file with no world access or group write. The helper reads dotenv values
with Node's parser; it never executes or shell-sources them. Only the four SMTP
settings and the final sender address are copied. The new service receives its own
random pepper, database and protected environment file. Existing files or releases
cause installation to stop rather than overwrite a previous attempt.

```sh
# Replace all example paths, origin and revision with reviewed values.
/path/to/node /tmp/install-smtp.mjs preflight \
  -o https://member.example.com \
  -e /etc/member/smtp-config.env -e /etc/member/smtp-secret.env \
  -e /etc/member/mail-common.env \
  -n /etc/nginx/sites-enabled/member.example.com

/path/to/node /tmp/install-smtp.mjs install \
  -o https://member.example.com -r REVIEWED_40_CHARACTER_COMMIT \
  -e /etc/member/smtp-config.env -e /etc/member/smtp-secret.env \
  -e /etc/member/mail-common.env
```

Preflight is read-only. It checks source permissions, Node 24, the SMTP field mapping,
whether loopback port 3210 is free, existing install state, public route availability,
and optionally the reviewed Nginx insertion point. It prints only non-secret state.

Installation fetches pinned files into `/opt/elix-accounts/releases/COMMIT`, installs
the locked runtime dependencies with scripts disabled, writes a root-only environment
file, and verifies TLS/authentication against SMTP without sending mail. It
creates a dedicated nologin service identity and starts a hardened systemd unit
bound to loopback. Startup failure disables this new service and retains files for
inspection. Any existing account-service user/group or data directory prevents a
first installation, including artifacts from a failed earlier attempt. SMTP checking
uses a minimal process environment so operator exports cannot override the newly
written protected configuration. Existing membership and staff services are not
restarted. Installation does not publish account routes through Nginx.

## Optional publication on an existing HTTPS membership origin

Only use `publish` after inspecting the actual Nginx file. This mode requires the
reviewed layout: one matching HTTPS server block, one exact line containing four
spaces then `client_max_body_size 64k;`, no existing `/v1` locations, and no existing
accounts include. A different layout causes a stop for review.

```sh
/path/to/node /tmp/install-smtp.mjs publish \
  -o https://member.example.com \
  -n /etc/nginx/sites-enabled/member.example.com
```

It preserves the existing virtual-host text except for adding one include directly
after that body-size line. The separate snippet owns exact `/v1/config`, `/v1/account`,
`/v1/sync` locations, the `/v1/auth/` prefix, and `/elix-accounts-health` mapped to
loopback `/health`. Each has a 6 MB body limit, disabled access logging and overwritten
proxy headers. It does not expose loopback port 3210 through a firewall.

The original virtual-host file is backed up under `/root` with mode 0600 before the
atomic replacement. Nginx must pass `nginx -t` before reload. Failure of syntax,
reload, or public health/config checks restores and reloads the original virtual-host
file; the helper reports the failed phase and, for public verification, HTTP statuses
without response bodies. Public verification allows up to ten seconds for the new
workers, using connections that close after each request. The helper reports if the
rollback reload also fails. Concurrent changes
to the virtual-host file stop automatic replacement or restoration instead of
overwriting another operator's edits. It preserves the new
snippet and backup for inspection. It never removes or restores account data.
After a successful rollback, `publish` may be retried: an existing snippet is accepted
only if it is a root-owned regular file without group/world write and exactly matches
the helper's expected text. All route and virtual-host checks still apply.

Successful SMTP authentication and `/v1/config` do not prove sender verification,
inbox delivery or successful desktop login. Test those explicitly with an authorized
tester before publishing a release. A development installer can embed a verified
service origin for that test; record inbox delivery and desktop login as outstanding
until they have actually been observed.
Do not use `install` for upgrades or retries with partially created state: inspect
and plan the exact recovery first. No secret value belongs in CLI arguments, GitHub,
chat or desktop build variables.

Local checks: `node --test services/accounts/deploy/install-smtp.check.mjs`.
