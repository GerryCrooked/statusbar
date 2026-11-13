<div align="center">

# Statusbar

**An unobtrusive IT incident banner for Windows desktops.**  
Self-hosted · Rust · Active Directory-aware · Open for contributions

[![Build and test](https://github.com/GerryCrooked/statusbar/actions/workflows/build.yml/badge.svg)](https://github.com/GerryCrooked/statusbar/actions/workflows/build.yml)
![Status](https://img.shields.io/badge/status-early%20pilot-orange)
![Platform](https://img.shields.io/badge/client-Windows%2010%2F11-blue)
![Backend](https://img.shields.io/badge/backend-Rust-dea584)

[Features](#features) · [Quick start](#quick-start) · [Windows client](#windows-client) · [Security](#security-and-limitations) · [Support](#support-the-project)

</div>

> [!WARNING]
> **Early pilot (v0.4.1).** The Windows client, RDS behavior, multi-monitor support, identity verification and full end-to-end deployment have not yet been validated for production. Check the latest [CI results](https://github.com/GerryCrooked/statusbar/actions) before deploying.

## What is Statusbar?

Statusbar helps IT teams communicate outages and maintenance notices directly on Windows desktops. While an incident is active, the client shows a bar **above the taskbar** and uses the Windows AppBar API to reserve screen space. After an incident is resolved, it briefly displays an all-clear message, then disappears.

The server runs on your own infrastructure. Administrators can publish messages from a browser or integrate existing monitoring and ticketing workflows through an API.

<!-- Add screenshots when the Windows client has been verified.
![Incidents](docs/images/incidents.png)
![Settings](docs/images/settings.png)
-->

## Features

| Area | Capabilities |
| --- | --- |
| **Windows desktop** | Native Rust client, persistent incident bar, resolution message, automatic hide |
| **Administration** | Web UI, incident management, AD-based administrator login |
| **Directory targeting** | AD users, direct AD groups, user OUs, and custom notification groups combining multiple AD groups |
| **Integrations** | Authenticated REST/webhook endpoint for incident creation |
| **White-labeling** | Editable site name, subtitle, header color and optional logo; no empty logo placeholder |
| **Languages** | English by default, German included; community translation packs with attribution metadata |
| **Hosting** | Docker Compose, Rust/Axum, SQLite, LDAP over StartTLS or LDAPS |

**Targeting note:** Custom notification groups use OR semantics: membership in *any* included AD group matches. Nested AD groups are not currently supported. The current client identity model must not be used to distribute confidential information.

## Architecture

```text
IT Admin UI ─────┐
Monitoring/API ──┼── HTTPS ──> Statusbar server (Rust/Axum)
                 │                    ├── SQLite
                 └────────────────────└── Active Directory (StartTLS / LDAPS)
                                      │
                                      └── HTTPS polling ──> Windows AppBar client
```

## Quick start

**Requirements:** Docker Engine, Docker Compose v2, an HTTPS reverse proxy and (for AD features) a domain controller with a trusted TLS certificate.

```bash
git clone https://github.com/GerryCrooked/statusbar.git
cd statusbar

./scripts/bootstrap.sh
# Edit .env before starting, especially the public URL / AD configuration.

docker compose up -d --build
docker compose logs -f status-server
```

The default Docker binding is loopback-only (`127.0.0.1:8080`). Configure `HTTP_PORT` and `BIND_ADDRESS` in `.env` for your environment; **do not expose the admin UI without HTTPS and network restrictions**.

Back up the SQLite Docker volume and `.env` before upgrading. Keep your production `.env` out of Git.

### Active Directory

Example `.env` values (replace with your own infrastructure):

```dotenv
AD_ENABLED=true
AD_URL=ldap://dc01.example.local:389
AD_BIND_DN=CN=svc_status,OU=Service Accounts,DC=example,DC=local
AD_BIND_PASSWORD=CHANGE_ME
AD_BASE_DN=DC=example,DC=local
AD_SYNC_SECONDS=300
AD_ADMIN_GROUP=Serveradmin
ADMIN_SESSION_HOURS=8
```

- `ldap://...:389` requires **StartTLS before authentication**; unencrypted simple bind is not supported.
- Alternatively, use `ldaps://...:636` with a valid domain controller certificate.
- Install your trusted CA certificate as a PEM-encoded `.crt` under `server/ca/`, then rebuild the image. **Never commit private keys or real production certificates.**
- Use a dedicated least-privileged AD bind account. Admins sign in with their individual AD credentials and must belong directly to the configured administrator group.
- AD login is **not** automatic Kerberos SSO. Browser sessions require HTTPS.

## Settings and customization

The administrator **Settings** page provides:

- **Branding:** site name, subtitle, header color and optional PNG/JPEG/WebP logo. With no logo configured, no image placeholder is displayed.
- **Notification groups:** give a group your own name, search and multiselect synchronized AD groups, then use the combined group as an incident target.
- **Language:** select English (default) or German. Translation files live under `server/locales/`; additional JSON packs can include translator, source, version and license information.

Interface translations do not automatically translate user-written incident text or Windows-native client messages.

## Windows client

The repository includes a Windows client in `client/` and PowerShell install/uninstall scripts in `scripts/`.

**Prebuilt artifacts (when CI succeeds):** Visit [GitHub Actions](https://github.com/GerryCrooked/statusbar/actions), open a successful build and download `vg-status-client-windows-x64`. The executable is currently **unsigned**.

Or build it yourself on Windows with Rust and the MSVC C++ toolchain:

```powershell
cd client
cargo build --release --target x86_64-pc-windows-msvc
```

For a manual test, open an elevated PowerShell in the project root and run:

```powershell
.\scripts\install-client.ps1 `
  -ExePath '.\vg-status-client.exe' `
  -ServerUrl 'https://status.example.org' `
  -Token '<CLIENT_TOKEN from .env>'
```

Sign out and back in to start the client in the interactive user session. **Do not run the UI client as SYSTEM or as a Windows service.** Test the AppBar on a single machine before planning distribution with baramundi or another endpoint management system.

## API / integrations

A webhook token can be used to create incidents, for example:

```bash
curl -X POST 'https://status.example.org/api/webhooks/incidents' \
  -H "Authorization: Bearer $WEBHOOK_TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"title":"Application unavailable","message":"IT is investigating.","severity":"error","target_type":"all"}'
```

Supported target types include `all`, `group`, `ou`, `user` and `custom`. See the code and [security documentation](docs/SECURITY.md) for their current requirements and limitations. Never expose API tokens in screenshots, logs or commits.

## Security and limitations

- Use HTTPS for all client/admin traffic and enforce trusted LDAP TLS certificates.
- Protect `.env`, tokens, the SQLite volume and internal network details.
- The current client uses a shared token and does **not** cryptographically prove the signed-in user's identity.
- Nested AD groups, RDS-session behavior, multi-monitor handling and Explorer restarts require additional testing.
- A successful source build does **not** replace functional and security testing.

For more detail, read [SECURITY.md](docs/SECURITY.md).

## Contributing and translations

Issues, bug reports, reproducible build failures, code improvements and additional languages are welcome via [GitHub Issues](https://github.com/GerryCrooked/statusbar/issues). When proposing translations, include the language code, translator name, source, version and redistribution license. Please do not submit real AD details, secrets or customer data.

**License:** No open-source license is currently published. Until a license is added, public source visibility does not grant general reuse or redistribution rights. Contributions and white-label usage terms should be clarified before public release.

## Support the project

If Statusbar is useful to you and you'd like to support ongoing development:

[![Buy Me a Coffee](https://img.shields.io/badge/Buy%20Me%20a%20Coffee-Support%20development-FFDD00?logo=buymeacoffee&logoColor=000000)](https://buymeacoffee.com/crookedgerry)

**[buymeacoffee.com/crookedgerry](https://buymeacoffee.com/crookedgerry)**

---

<div align="center">

Built in Rust · [Source code](https://github.com/GerryCrooked/statusbar) · [Report an issue](https://github.com/GerryCrooked/statusbar/issues)

</div>
