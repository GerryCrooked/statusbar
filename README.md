# VG Statusbar v0.4.3

Self-hosted incident banner for Windows 10/11 with a Rust/Axum server, SQLite persistence, a native Win32 AppBar client, multi-monitor display, optional AD targeting, and a **standalone mode without Active Directory**. Source code is included. No credentials or private certificates are bundled.

> **Release status:** v0.4.3 includes an **experimental** MSTSC compatibility adjustment; it must be verified on Windows and on the intended RDP configuration before broad deployment. A Linux cross-build does not replace interactive Windows tests.

## Contents

1. [Requirements and security model](#1-requirements-and-security-model)
2. [Fresh server installation](#2-fresh-server-installation)
3. [AD mode versus standalone mode](#3-ad-mode-versus-standalone-mode)
4. [TLS and the CA certificate in `server/ca/`](#4-tls-and-the-ca-certificate-in-serverca)
5. [Client display / MSTSC compatibility](#5-client-display--mstsc-compatibility)
6. [Local Windows cross-compilation in Docker](#6-local-windows-cross-compilation-in-docker)
7. [AD CS Code Signing, step by step](#7-ad-cs-code-signing-step-by-step)
8. [Deploy and test a client](#8-deploy-and-test-a-client)
9. [Upgrade from v0.4.2 and rollback](#9-upgrade-from-v042-and-rollback)
10. [Diagnostics, backups and limitations](#10-diagnostics-backups-and-limitations)

## 1. Requirements and security model

- Linux server with Docker Engine and Compose v2; HTTPS reverse proxy in front of the server.
- Windows 10/11 desktop clients; the app runs **in each interactive user session**, not as a Windows service.
- Optional AD: read-only LDAP bind account and a DC with valid TLS server certificate.
- Optional AD CS: issued Code Signing certificate belonging to a designated signer and Windows SDK Signing Tools on a **separate Windows signing workstation**, not on a domain controller.
- Three independent secrets in server `.env`: `ADMIN_TOKEN` (administrator API and standalone login), `WEBHOOK_TOKEN` (incident webhook), `CLIENT_TOKEN` (current shared client authentication). Never put these in public source control.
- The `CLIENT_TOKEN` is still shared with deployed clients and can be extracted by a local user who can read `client.json`. Do **not** use current authentication for sensitive identity claims or production emergency alarm functions; per-device identity and delivery acknowledgement are not implemented.
- A valid internal Authenticode signature verifies publisher and file integrity under your enterprise trust policy. It does **not** guarantee that SmartScreen or endpoint application control will permit execution.

## 2. Fresh server installation

```bash
unzip vg-statusbar-v0.4.3.zip -d /opt/docker/statusbar-v043-extracted
cd /opt/docker/statusbar-v043-extracted/vg-statusbar
bash scripts/bootstrap.sh
# Edit .env before production: AD_ENABLED, URLs, ports, tokens, etc.
chmod 600 .env
docker compose up -d --build
curl -fsS http://127.0.0.1:8080/api/version
```

`bootstrap.sh` produces random tokens; it must not overwrite existing secrets during an upgrade. The default Compose service listens on `127.0.0.1:8080` on the Linux host. Publish it only behind a properly secured HTTPS reverse proxy. Forward `Authorization` and cookies. For administration, restrict access by network / VPN where practical. Check `docker compose logs --tail=100 status-server` after startup.

The running build reports `{"version":"0.4.3"}` on `/api/version`.

## 3. AD mode versus standalone mode

Only **one** login form is shown, determined by the server `.env` setting at startup. The UI retrieves this from the public read-only `GET /api/auth/mode` endpoint; browser users cannot choose or override the mode.

| `.env` | Sign-in UI | Recipients | Directory access |
|---|---|---|---|
| `AD_ENABLED=yes` | AD username and AD password **only** | All, AD group, user OU, AD user, custom AD group | Enabled |
| `AD_ENABLED=no` | Local administrator token **only** (username fixed internally to `admin`) | **All clients only** | Disabled |

For compatibility, `true/false`, `1/0`, and `on/off` also work. Invalid boolean values cause startup to fail instead of silently switching modes. `AD_ENABLED` is the only switch; there is no separate `STANDALONE_ENABLED` to accidentally conflict.

### AD mode

```dotenv
AD_ENABLED=yes
AD_URL=ldap://dc01.example.local:389
AD_BIND_DN=CN=svc_status,OU=Service Accounts,DC=example,DC=local
AD_BIND_PASSWORD=SET_IN_PRIVATE_ENV_ONLY
AD_BASE_DN=DC=example,DC=local
AD_ADMIN_GROUP=Serveradmin
AD_SYNC_SECONDS=300
```

`ldap://` always upgrades to StartTLS before binding; `ldaps://...:636` is also supported. DC certificate verification is enabled. The sign-in requires a real AD username/password and direct membership in the configured AD administrators' group (nested memberships are not resolved). Logging into the browser is **not** Kerberos SSO. Cookies use `Secure`, `HttpOnly`, and `SameSite=Strict`; access over HTTPS only.

The `ADMIN_TOKEN` is still retained for trusted administration **API** automation / break-glass; it is **not displayed as a second browser login** in AD mode. Treat it as highly privileged.

### Standalone / workgroup mode

```dotenv
AD_ENABLED=no
# ADMIN_TOKEN must be generated uniquely and strongly, e.g. openssl rand -hex 32
```

The web page shows only a local administrator-token password field; the fixed username `admin` is supplied internally. No AD login fields, AD directory dashboard or custom AD group editor appear. Only `All users` is selectable for incident recipients. The API also rejects any non-`all` recipient target in standalone mode, independently of the UI.

Clients do **not** need domain membership. They need to trust the server's **HTTPS certificate** and be able to reach the configured server URL. Keep the reverse proxy and admin API protected and generate unique tokens per installation. The local login uses the existing `ADMIN_TOKEN`; this is suitable as an initial bootstrap model, not a substitute for individual admin accounts and audit trails.

To switch modes, change `AD_ENABLED` in `.env` and recreate the server:

```bash
cd /opt/docker/vg-statusbar
docker compose up -d --force-recreate status-server
curl -fsS https://status.example.local/api/auth/mode
```

Expected: `{"mode":"ad","ad_enabled":true}` or `{"mode":"standalone","ad_enabled":false}`. Open the web page with a hard refresh. Existing browser sessions may be invalidated by container restart (sessions are in memory).

## 4. TLS and the CA certificate in `server/ca/`

**There are three distinct certificate functions. Do not mix their private keys.**

| Item | Used for | Typical location |
|---|---|---|
| AD/DC TLS certificate | Encrypt LDAP StartTLS/LDAPS and authenticate the DC | DC local computer certificate store; **private key stays on DC** |
| Public CA certificate(s), PEM `.crt` | The Linux container trusts the issuer of the **DC TLS certificate** | `server/ca/vg-root-ca.crt` (and intermediate CA if applicable) |
| AD CS Code Signing certificate + private key | Sign Windows client EXE with Authenticode | `Cert:\CurrentUser\My` on restricted Windows signing workstation |

### Export the public AD CA certificate

On a permitted Windows admin workstation with access to the CA, run PowerShell:

```powershell
# Public certificate only, never a .pfx or private key.
certutil -config "DC01-CA-Ichenhausen\\DC01-CA-Ichenhausen" -ca.cert C:\Temp\vg-ad-ca.cer
```

**CA host / CA name are examples.** Check the actual CA configuration first with `certutil -config - -ping` or from Certification Authority MMC. Alternatively, in `certsrv.msc`, right-click the CA → Properties → View Certificate → Details → Copy to File → **Base-64 encoded X.509 (.CER)**. Export the issuing/root CA's *public certificate*, not the DC's private key. If exporting in DER form, convert on Debian with `openssl x509 -inform DER -in vg-ad-ca.cer -out vg-root-ca.crt`. If exporting Base-64, the file will contain `-----BEGIN CERTIFICATE-----` and can be copied directly as `.crt`.

Copy the public CA file (and required intermediates) to `server/ca/`, e.g. `server/ca/vg-root-ca.crt`, then rebuild the server image:

```bash
openssl x509 -in server/ca/vg-root-ca.crt -noout -subject -issuer -dates
# The certificate must be the CA which issued the DC's TLS certificate.
docker compose build status-server
docker compose up -d --force-recreate status-server
```

To verify the DC certificate chain before use (example):

```bash
openssl s_client -starttls ldap -connect dc01.example.local:389 \
  -servername dc01.example.local -CAfile server/ca/vg-root-ca.crt </dev/null
```

Check `Verify return code: 0 (ok)` **and** the presented hostname/identity; use the actual DC FQDN. If your PKI uses intermediate CAs, include them in a CA bundle, or install each public CA certificate in `server/ca/`. This CA file is **not** a client code-signing certificate. It contains **no private key** and is not a signing secret; however, do not publish organization-specific certificates unintentionally.

For the **Windows clients' HTTPS connection to your Statusbar server**, the issuing CA of the HTTPS reverse-proxy certificate must be trusted in the Windows certificate store. This may be a different CA from the DC's LDAP issuer.

## 5. Client display / MSTSC compatibility

Server `.env` controls the whole client fleet via `/api/client/state`:

```dotenv
BAR_POSITION=top
BAR_ALL_MONITORS=true
BAR_HEIGHT_PX=44
BAR_INITIAL_HEIGHT_PX=88
BAR_INITIAL_SECONDS=5
RESOLVED_DISPLAY_SECONDS=90
BAR_RDP_COMPAT_MODE=false
```

`BAR_POSITION` is `top` or `bottom`. Normal height is clamped to 24–160 px; initial enlarged height to 24–300 px, initial seconds to 0–60; resolved seconds to 0–3600. An AppBar reserves Windows working area, which is generally respected by normal maximized windows.

The new opt-in `BAR_RDP_COMPAT_MODE=true` makes the client **experimentally** detect visible, maximized `mstsc.exe` windows and reposition them to the remaining working area when they overlap a registered AppBar. It attempts to restore earlier window placement on removal, but only if the user has not changed the window position. **True remote desktop full-screen, disconnected sessions, varying DPI, multi-monitor RDP, and smart sizing are not guaranteed.** Verify on a test device and do not enable fleet-wide before acceptance testing. Disable at any time by setting `BAR_RDP_COMPAT_MODE=false` and restarting/reloading client status. Avoid using this behavior on safety-critical remote displays without prior testing.

## 6. Local Windows cross-compilation in Docker

Use the **separate local build kit** `vg-statusbar-local-build-kit` supplied alongside `vg-statusbar` in this ZIP. `cargo-xwin` builds a Windows MSVC executable on the Linux Docker host, with no private certificate or shared client token passed to the compiler.

```bash
cd /opt/docker/vg-statusbar
# After upgrading v0.4.3 source files:
bash scripts/build-v043.sh
# Or, directly:
cd /opt/docker/vg-statusbar-local-build-kit
bash linux/prepare-release.sh
```

Expected outputs:

```
/opt/docker/vg-statusbar-local-build-kit/dist/unsigned/vg-status-client.exe
/opt/docker/vg-statusbar-local-build-kit/dist/unsigned/SHA256SUMS.txt
/opt/docker/vg-statusbar-local-build-kit/dist/private/client.json
```

The config is private because the current client uses a shared `CLIENT_TOKEN`. The build process needs network access to download crates and Microsoft redistributable components. Pin toolchain/image versions and retain `Cargo.lock` for production reproducibility. Build failures should be fixed before signing; a successful build does not imply verified GUI behavior.

## 7. AD CS Code Signing, step by step

**Never export the CA's private key to sign code.** The signer uses an **issued leaf certificate** that includes the Code Signing EKU (`1.3.6.1.5.5.7.3.3`) and keeps its own private key on the signing workstation.

### 7.1 Publish an AD CS certificate template (PKI administrator)

1. On an authorized PKI administration workstation: `certtmpl.msc` → duplicate an appropriate template for a **user** Code Signing certificate, or inspect an existing one (e.g. `VGIchenhausenCodeSigning`).
2. **Extensions**: Application Policies / Enhanced Key Usage must include **Code Signing**. Avoid granting unnecessary additional usages.
3. **Security**: establish a small designated AD group, e.g. `GG_CodeSigning`, with Read + Enroll. **Authenticated Users must not have Enroll or Autoenroll**. Retain required CA/system administrative access; avoid accidental removal of Read rights necessary for CA/template discovery.
4. **Request Handling / Cryptography**: use a modern key algorithm (e.g. RSA 3072+ and SHA-256), preferably non-exportable private key. Use approved key storage and permissions; for higher assurance, a hardware-backed provider may be appropriate.
5. **Validity period**: configure based on policy; actual lifetime is bounded by the CA and template. Publish the template from `certsrv.msc` → Certificate Templates → New → Certificate Template to Issue. Follow your organization’s CA change-control process.

Do not indiscriminately change permissions on an existing template used by other systems.

### 7.2 Enroll the signing certificate (signing workstation)

1. Log on to a dedicated, patched Windows workstation as an authorized signing user. Do **not** sign on a domain controller.
2. Run `certmgr.msc` → Personal → Certificates → All Tasks → Request New Certificate. Select the published Code Signing template.
3. Inspect the issued certificate, key presence, expiry and EKU:

```powershell
Get-ChildItem Cert:\CurrentUser\My |
 Where-Object { $_.EnhancedKeyUsageList.ObjectId -contains '1.3.6.1.5.5.7.3.3' } |
 Select-Object Subject, Thumbprint, NotAfter, HasPrivateKey
```

4. Validate trust:

```powershell
$c = Get-Item 'Cert:\CurrentUser\My\YOUR_NEW_THUMBPRINT'
$chain = [Security.Cryptography.X509Certificates.X509Chain]::new()
$chain.Build($c)
$chain.ChainStatus
```

Expect `True` and no chain errors. Distribute appropriate public CA certificate(s) to clients via approved enterprise GPO/PKI trust policies; private keys stay on the signer.

### 7.3 Install SignTool and sign

Install **Windows SDK Signing Tools for Desktop Apps** from Microsoft's official Windows SDK installer. You do not need the full Visual Studio IDE just to sign.

A copy/paste-ready PowerShell form:

```powershell
$signtool = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits" -Filter signtool.exe -Recurse -ErrorAction SilentlyContinue | Where-Object FullName -Match '\\x64\\signtool.exe$' | Sort-Object FullName -Descending | Select-Object -First 1 -ExpandProperty FullName
$exe = 'C:\Statusbar-Signing\vg-status-client.exe'
$thumb = 'YOUR_NEW_THUMBPRINT'
& $signtool sign /sha1 $thumb /s My /fd SHA256 $exe
if ($LASTEXITCODE -ne 0) { throw 'Signing failed' }
& $signtool verify /pa /v $exe
if ($LASTEXITCODE -ne 0) { throw 'Verification failed' }
Get-AuthenticodeSignature $exe | Select-Object Status,StatusMessage,SignerCertificate
```

For the supported deployment ZIP, copy `vg-status-client.exe` and private `client.json` to a restricted local Windows folder; copy the `windows/` folder from the local build kit alongside them, and run:

```powershell
.\windows\Sign-And-Package.ps1 -ExePath .\vg-status-client.exe -ConfigPath .\client.json -CertificateThumbprint 'YOUR_NEW_THUMBPRINT'
```

This script finds SignTool, validates the Code Signing EKU and creates a private deployment ZIP with installer and backup/uninstaller. It signs a **copy** of the EXE. Use an approved RFC-3161 timestamp service (`-TimestampUrl 'https://your-approved-timestamp-service'`) and test timestamp validation before production distribution. Without a trusted timestamp, signatures may cease to validate after the signing certificate expires. Do not invent a timestamp service URL or rely on the issuer CA to act as a timestamp server.

## 8. Deploy and test a client

The private `client.json` contains `server_url` and `client_token`. Protect it (especially the shared token); never commit it. Ensure its JSON is UTF-8 **without BOM** (Windows PowerShell 5.1 `Set-Content -Encoding UTF8` can introduce BOM).

On a normal Windows test machine, first run the signed EXE interactively beside the private config, then create an `all` test incident and resolve it. The process is `vg-status-client.exe`:

```powershell
Start-Process 'C:\Statusbar-Test\vg-status-client.exe'
Get-Process vg-status-client -ErrorAction SilentlyContinue
Get-AuthenticodeSignature 'C:\Statusbar-Test\vg-status-client.exe'
```

For packaging, use the build kit's `Install-VGStatusbar.ps1` in an elevated session. Test `-NoAutostart` first; in normal deployment, it installs a Run key for each interactive user session. Client `client.json` can be found under `C:\ProgramData\VGStatusbar\client.json` or beside the EXE (ProgramData takes precedence). Back up old binaries and config before installing. For baramundi, distribute the **private** deployment package over a restricted channel; do not publish it in an open file share.

## 9. Upgrade from v0.4.2 and rollback

**Before the upgrade:** back up `.env`, the named SQLite data volume, the code-signing deployment and current client binaries. Keep the old signed client available. Note: v0.4.3 server source includes the standalone login mode, which is not available on the v0.4.2 server.

```bash
# Example layout, adjust ZIP path to where you uploaded it.
mkdir -p /opt/docker/statusbar-update-v043
unzip -o /opt/docker/vg-statusbar-v0.4.3.zip -d /opt/docker/statusbar-update-v043
cd /opt/docker/statusbar-update-v043/vg-statusbar
bash scripts/upgrade-v043.sh /opt/docker/vg-statusbar
cd /opt/docker/vg-statusbar
# Edit .env: AD_ENABLED=yes for existing AD environment; leave it unchanged otherwise.
# Enable BAR_RDP_COMPAT_MODE=true only for TEST clients.
docker compose build status-server
docker compose up -d --force-recreate status-server
curl -fsS https://status.example.local/api/version
curl -fsS https://status.example.local/api/auth/mode
```

The upgrade script copies only reviewed source and docs, takes a timestamped **source backup** under `backups/`, and preserves `.env`, CA certificates and SQLite data. The upgrade itself does not roll back the database; migrations must be considered separately.

If the new server fails to start, restore the backed-up source files in the recorded backup directory and rebuild the container. The previous signed client binary can be restored using the local build kit's `Restore-LastBackup.ps1` where applicable. The `BAR_RDP_COMPAT_MODE` feature is disabled by default.

## 10. Diagnostics, backups and limitations

```bash
cd /opt/docker/vg-statusbar
docker compose ps
docker compose logs --tail=150 status-server
curl -fsS https://status.example.local/api/version
curl -fsS https://status.example.local/api/auth/mode
```

- `GET /api/version` should match the deployed version. A green Docker build only validates server compilation and startup; Windows integration still requires tests.
- Browser login can fail if reverse proxy strips cookies or HTTPS isn't in use. In AD mode, check `AD_ADMIN_GROUP`, AD sync state and DC TLS CA trust.
- Standalone `all` incidents require **no AD**, but all connected clients must carry the server's `CLIENT_TOKEN` and trust its HTTPS certificate.
- Test `BAR_RDP_COMPAT_MODE` with classic `mstsc.exe` specifically. It is experimental and not guaranteed for true full-screen RDP. The user can disable it centrally.
- Current client polling is not server-push / WebSocket; changes are fetched at the client's configured `poll_seconds` interval. Messages appear on the next poll.
- DB and AD session data: SQLite in Docker named volume `status-data`; admin browser sessions are in memory and reset on server restart. Back up SQLite consistently (SQLite backup API or stop-and-copy volume), and restrict access to `.env`.
- Rotating `CLIENT_TOKEN` requires updating existing clients. Client secrets are not embedded into the EXE and cannot be secured solely by Code Signing.
- Do not use this software as the sole emergency notification path. No guaranteed-delivery mechanism, recipient acknowledgements, or verified individual client identity is implemented.

Related docs: [`docs/DISPLAY-CONFIG.md`](docs/DISPLAY-CONFIG.md), [`docs/SECURITY.md`](docs/SECURITY.md).
