> **Version v0.4.1:** The admin header shows the web version and queries `/api/version` for the running Rust backend version. If Settings is absent, the old HTML is still being served.

# VG Statusbar – Rust (pilot 0.1)

A self-hosted Windows IT incident bar that reserves screen space above the taskbar while active and disappears after resolution. Rust server (Axum + SQLite + LDAP) and native Rust Win32 AppBar client. **All source code included. No prebuilt .exe supplied.**

## Quick start – server

Prerequisites: Docker Engine / Compose V2, outbound access to Rust crates at first image build, HTTPS reverse proxy, and (optionally) LDAPS access to AD.

```bash
./scripts/bootstrap.sh
# Creates .env and three different strong random tokens automatically
# Leave AD_ENABLED=false for initial smoke test, then configure LDAPS below
sudo docker compose up -d --build
sudo docker compose logs -f status-server
```

Open the admin UI via your HTTPS reverse proxy at `https://status.example.local/` and enter `ADMIN_TOKEN`. For a loopback-only smoke test on the Docker host, use `http://127.0.0.1:8080/` (never transmit bearer credentials over an untrusted network).

**Reverse proxy:** Set HTTPS termination with a certificate trusted by your Windows machines, forward only your internal admin/client ranges to `http://127.0.0.1:8080`, and **do not strip Authorization headers**. Keep Docker bound to loopback by default. Configure your reverse proxy separately (not bundled).

## Active Directory settings

Set in `.env`:

```env
AD_ENABLED=true
AD_URL=ldaps://dc01.example.local:636
AD_BIND_DN=CN=svc_status,OU=Service Accounts,DC=example,DC=local
AD_BIND_PASSWORD=YOUR_SERVICE_ACCOUNT_PASSWORD
AD_BASE_DN=DC=example,DC=local
AD_SYNC_SECONDS=300
```

Use a dedicated AD read-only account. The Linux container must trust the issuing CA for the domain controller certificate; mount your approved CA store into the container or extend `server/Dockerfile` to install the certificate and update CA trust. Test LDAPS separately if sync fails. UI shows sync status. `AD_USER_FILTER` controls which accounts are loaded. Groups are **direct user memberOf** entries; OU targeting uses user's **distinguishedName**. Computer OUs are not used.

For test with no AD, only 'all' is selectable. On error directory keeps last successful snapshot; a new group-specific incident is blocked until a successful sync. Group/OU targeting is based on **user accounts**, not the computer objects.

## Build Windows client

On Windows 10/11 with Rust toolchain MSVC and Visual Studio C++ Build Tools:

```powershell
cd client
cargo build --release
```

Result: `client\target\release\vg-status-client.exe`. Alternatively, push this repository to your own source control and use `.github/workflows/build.yml` (Windows CI artifact). Github Actions is optional, not required for deployment.

## Manual install on Windows (test machine)

Launch elevated PowerShell in repository root:

```powershell
.\scripts\install-client.ps1 `
  -ExePath .\client\target\release\vg-status-client.exe `
  -ServerUrl 'https://status.example.local' `
  -Token '<CLIENT_TOKEN from .env>'
```

Sign out and sign in. `HKLM\...\Run` starts a client **in each signed-in user session**. For immediate testing execute `C:\Program Files\VGStatusbar\vg-status-client.exe` from a **normal user** session. Do not run it as SYSTEM or as a Windows service: UI windows need to run in the user's desktop session.

Try creating a test incident in the UI with target 'all'; the red bar should appear on the next poll (~10 s). Resolve incident; green bar is shown until `RESOLVED_DISPLAY_SECONDS` elapses (default 90 s), then disappears.

Uninstall (elevated after signing users out):

```powershell
.\scripts\uninstall-client.ps1
```

## Webhook sample

```bash
curl -X POST 'https://status.example.local/api/webhooks/incidents' \
  -H "Authorization: Bearer $WEBHOOK_TOKEN" -H 'Content-Type: application/json' \
  -d '{"title":"VOIS gestört","message":"Die IT arbeitet an der Behebung","severity":"error","target_type":"all"}'
```

Targets: `all`, `group`, `ou`, `user`. For `group` / `ou`, send `target_dn` with exact DN shown in web UI. For `user`, `target_dn` is `sAMAccountName`. Creation returns incident ID. Resolve via admin API only: `POST /api/admin/incidents/{id}/resolve` with `ADMIN_TOKEN`. Webhook token cannot resolve incidents.

## Data, maintenance and deployment

SQLite persists in named volume `status-data`. Back it up consistently (`sqlite3 .backup` or stop container and archive named volume). Review and secure `.env` permissions (`chmod 600 .env` on server). Take backup before upgrades. Docker image rebuild uses current selected Rust crates unless you generate and commit `Cargo.lock` with `cargo build`.

For baramundi, use the same PowerShell installer and EXE as payload, deploy in machine context then start the app at users' next login. Use separate customer/environment tokens. Test RDS **per session** before deployment, and handle process cleanup at logoff.

Read [Security and limitations](docs/SECURITY.md) before production use.


## LDAP Port 389 / StartTLS and AD web admins (v0.2)

- In `.env`: `AD_ENABLED=true`, `AD_URL=ldap://dc01.example.local:389`, `AD_ADMIN_GROUP=Serveradmin`.
- `ldap://` **always uses StartTLS before bind**. Unencrypted simple bind is intentionally unsupported.
- The domain controller needs a TLS server certificate. Copy its issuing CA (PEM `.crt`) to `server/ca/vg-root-ca.crt`. Then `docker compose up -d --build`. Do not copy a private key.
- Log into the web UI with your individual AD account and password. The account must be a direct member of `Serveradmin` (matching the exact CN). No admin token is requested in the browser. Cookies are HttpOnly + Secure, so use HTTPS.
- AD admin sessions are kept in memory and expire after `ADMIN_SESSION_HOURS` (default 8); they are lost on server restart and membership is checked against the latest successful AD sync.
- This is **AD password login, not silent Kerberos SSO**. To enable true automatic Windows logon, an authenticated Kerberos reverse proxy needs a separately secured integration.
- Do **not** publish the app over plain HTTP. The existing `ADMIN_TOKEN` remains for API automation/emergency admin access; `WEBHOOK_TOKEN` remains for integrations.
- AD group membership uses direct `memberOf` values; nested groups are not supported.
- Deploy changes with `docker compose up -d --build` and inspect `docker compose logs -f status-server`.

## Settings / White-labeling (v0.3 development)

The administrator UI includes a **Settings** page with persistent page name,
subtitle, header color, and an optional uploaded logo (PNG/JPEG/WebP, max 1 MB).
No image or placeholder is rendered when the logo is not set.
These settings live in the existing SQLite database and are visible on the
login page. The brand header does not require a user login.

Settings also provides **custom notification groups**. An administrator can
search existing AD group names, select several groups, and save them under an
arbitrary name such as `testusers-for-notifications`. Group membership uses an
**OR** operation: any direct member of any chosen AD group is targeted. Groups
can be edited or deleted, except while they have active incidents targeting
them. Existing AD group / OU / individual / all targeting remains available.

The incident API accepts `"target_type":"custom"` and
`"target_dn":"<custom-group-uuid>"`. IDs can be read through the authenticated
`GET /api/admin/custom-groups` endpoint.

### Upgrading from v0.2

1. Back up your SQLite Docker volume and `.env` (never commit them).
2. Replace project source files, keeping your existing `.env` and CA files.
3. Run `docker compose up -d --build --force-recreate`.
4. Check logs, log in through HTTPS, and open **Einstellungen**.

The database schema migration is additive (`CREATE TABLE IF NOT EXISTS`).
The old incidents table and AD settings are retained. No new `.env` values are
needed for v0.3. The Windows client protocol is unchanged.

**Development limitations:** direct AD membership only; no SSO; the existing
shared client token does not strongly attest user identity. Do not use for
confidential messages. Full Rust compilation and Windows integration testing
remain required before production rollout.


## Internationalization (v0.4)

The administration interface supports English (default) and German. In
**Settings → Interface language**, select the site-wide default and save the
branding settings. Strings come from `server/locales/en.json` and
`server/locales/de.json`; missing keys use English fallback text.

Admins can import community language packs (JSON) with locale, name, author,
source, version, license and translated strings. Attribution is displayed
alongside installed language packs. The import is text-only and does not run
scripts. See [v0.4 update and translation format](docs/UPDATE-v0.4.md).

**Scope:** This release localizes the web UI, not arbitrary incident messages
or the Windows AppBar client. Testing is required before production rollout.
