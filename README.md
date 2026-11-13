# VG Statusbar

Self-hosted Windows incident notification bar, written in Rust.

VG Statusbar displays IT incidents in a dedicated bar above the Windows taskbar. The bar reserves screen space while incidents are active and automatically disappears after resolution.

> **Project status:** Early development / pilot. Not yet validated for production use.

## Features

- Native Windows AppBar client written in Rust
- Rust backend using Axum and SQLite
- Docker Compose deployment
- Active Directory integration (LDAP StartTLS / LDAPS)
- AD group and OU based notification targeting
- AD administrator login with configurable admin group
- Web administration interface
- REST API for integrations and webhooks
- Multiple simultaneous incidents
- Automatic resolution notifications
- Windows 10/11 support (testing required)
- GitHub Actions Windows builds
- Prepared for unattended software deployment

## Architecture

    Monitoring / GLPI / Web Admin
                |
             HTTPS
                |
         Rust API Server
          /          \
       SQLite    Active Directory
                (LDAP / StartTLS)
                |
          Windows Clients
          (Native AppBar)

## Quick Start

Requirements:
- Docker Engine
- Docker Compose V2
- HTTPS reverse proxy
- Optional Active Directory server

Clone the repository:

    git clone https://github.com/GerryCrooked/statusbar.git
    cd statusbar

Generate configuration:

    ./scripts/bootstrap.sh

Edit `.env` and start:

    docker compose up -d --build

Check logs:

    docker compose logs -f status-server

By default, the application binds to localhost. Configure
HTTP_PORT and BIND_ADDRESS in `.env` as required.

## Active Directory

VG Statusbar supports LDAP with StartTLS on port 389
and LDAPS on port 636.

Example `.env`:

    AD_ENABLED=true
    AD_URL=ldap://dc01.example.local:389
    AD_BIND_DN=CN=svc_status,OU=Service Accounts,DC=example,DC=local
    AD_BIND_PASSWORD=CHANGE_ME
    AD_BASE_DN=DC=example,DC=local
    AD_SYNC_SECONDS=300
    AD_ADMIN_GROUP=Serveradmin
    ADMIN_SESSION_HOURS=8

LDAP connections on port 389 use StartTLS before binding.
Plaintext LDAP authentication is not supported.

The domain controller must have a valid TLS certificate.
Place the trusted issuing CA certificate in:

    server/ca/custom-root-ca.crt

Rebuild the container after adding the certificate:

    docker compose up -d --build

Only the public CA certificate is required. Never include
private keys, passwords or production certificates in Git.

### AD Administrator Authentication

Users who are direct members of the configured
AD_ADMIN_GROUP can log in with their AD credentials.

The browser uses an HTTPS-only session cookie.
The separate ADMIN_TOKEN remains available for API access.

This is AD authentication, not automatic Kerberos SSO.
Nested AD group membership is not yet supported.

## Notification Targeting

Supported target types:

| Type | Description |
|------|-------------|
| all | All connected clients |
| group | AD group membership |
| ou | AD user organizational unit |
| user | Individual AD user |

Targeting is user-based, not computer-OU-based.
The current client identity model is not suitable for
confidential or security-sensitive notifications.

## Windows Client

The client runs in the interactive Windows user session
and displays a native Windows AppBar.

When an incident is active:
- The bar appears above the Windows taskbar
- Desktop working area is reserved
- Incident text is displayed continuously

When an incident is resolved:
- A green resolution message appears
- The bar disappears after the configured duration

Default duration:

    RESOLVED_DISPLAY_SECONDS=90

### Download the Windows EXE

Open:

https://github.com/GerryCrooked/statusbar/actions

Select a successful "Build and test" workflow run.

Download the artifact:

    vg-status-client-windows-x64

The executable is not yet a signed release.

### Manual Installation

Run PowerShell as administrator from the project root:

    .\scripts\install-client.ps1 -ExePath ".\vg-status-client.exe" -ServerUrl "https://status.example.org" -Token "YOUR_CLIENT_TOKEN"

Sign out and sign back in.

The application runs in each user session, not as a
Windows service.

### Build from Source

Requirements:
- Rust stable
- MSVC toolchain / Visual Studio C++ Build Tools

Build:

    cd client
    cargo build --release --target x86_64-pc-windows-msvc

## REST API and Webhooks

External monitoring systems can create incidents via API.

Examples of potential integrations:
- GLPI
- Enginsight
- Checkmk
- Zabbix
- Custom monitoring services

Webhook authentication uses WEBHOOK_TOKEN.

Never expose API tokens publicly or commit them to Git.

## Security

- TLS certificate validation is mandatory for LDAP
- HTTPS is required for browser administrator sessions
- AD service accounts should have minimal permissions
- Environment secrets must remain outside version control
- Separate tokens for administration, webhooks and clients
- Restrict network access to trusted clients
- Review security limitations before production deployment

See [Security Documentation](docs/SECURITY.md).

## Deployment

The project is intended to support:
- Manual Windows installation
- Enterprise software deployment tools such as baramundi
- Multi-user environments, subject to testing

RDS, multi-monitor behavior and Explorer restarts require
further validation before enterprise rollout.

## Development Status

This project is in active development.

The server and Windows client have not yet completed
full integration and security testing.

Do not deploy organization-wide without validation.

## License

No open-source license has been selected yet.
A LICENSE file will be added before an official release.
