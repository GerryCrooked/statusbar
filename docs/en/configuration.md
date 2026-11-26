---
id: configuration
title: Configuration
description: Every setting of the Notiflag server, in .env and in the web UI.
lang: en
slug: configuration
order: 4
---

# Configuration

Notiflag has two places for settings:

- **`.env`** next to `compose.yaml`: secrets, network, directory, license, hub, Microsoft 365. Changes apply after
  `docker compose up -d`.
- **Web UI → Settings**: everything users see and hear. Changes apply to all clients immediately.

## Settings in `.env`

### Basics

| Variable | Default | Meaning |
|---|---|---|
| `ADMIN_TOKEN` | – (required) | Sign-in to the web UI and admin API. Random, at least 32 characters: `openssl rand -hex 32` |
| `CLIENT_TOKEN` | – (required) | The clients read their messages with it. Random, at least 32 characters, different from `ADMIN_TOKEN` |
| `HTTP_PORT` | `8080` | Port of the container on the host |
| `BIND_ADDRESS` | `127.0.0.1` | Address on the host. `127.0.0.1` = only for a reverse proxy on the same host |
| `RETENTION_DAYS` | `90` | How long resolved messages stay in the history |
| `RUST_LOG` | `info` | Log detail (`warn`, `info`, `debug`) |
| `DATABASE_PATH` | `/data/statusbar.sqlite` | Database file in the container |
| `LOCALES_DIR` | `/data/locales` | Folder for extra UI languages (see [Appearance and sounds](appearance.md#language)) |
| `CLIENT_MSI` | `/data/Statusbar.msi`, else the one in the image | Installer offered on the **Computers** page |
| `ADMIN_SESSION_HOURS` | `8` | How long a web UI sign-in lasts |

Changing `CLIENT_TOKEN` disconnects every client until it has the new token (redeploy the MSI with
`CLIENTTOKEN=<new>`).

### Active Directory

| Variable | Default | Meaning |
|---|---|---|
| `AD_ENABLED` | `no` | `yes` = sign in with a directory account, recipients by group, OU and user |
| `AD_URL` | – | `ldap://dc01.example.local:389` (with StartTLS) or `ldaps://dc01.example.local:636` |
| `AD_BIND_DN` | – | Read-only account, e.g. `CN=svc_notiflag,OU=Service Accounts,DC=example,DC=local` |
| `AD_BIND_PASSWORD` | – | Its password |
| `AD_BASE_DN` | – | Search base, e.g. `DC=example,DC=local` |
| `AD_ADMIN_GROUP` | `Statusbar-Admins` | Members of this group may sign in to the web UI. Set it explicitly |
| `AD_USER_FILTER` | enabled user accounts | LDAP filter for the users Notiflag knows |
| `AD_SYNC_SECONDS` | `300` | Interval of the directory sync |

Details and certificates: [Active Directory](active-directory.md).

### License, hub, Microsoft 365

| Variable | Meaning |
|---|---|
| `LICENSE_KEY` | License key (`SBL-…`), needed from 11 PCs without a hub. See [License](license.md) |
| `LICENSE_URL` | Another license portal (testing only) |
| `HUB_URL`, `HUB_TOKEN` | Connection to your IT service provider's hub. See [For IT service providers](service-provider-hub.md) |
| `M365_TENANT_ID`, `M365_CLIENT_ID`, `M365_CLIENT_SECRET` or `M365_CLIENT_SECRET_FILE` | Microsoft 365 service health (in development). More `M365_*`: [guide](https://github.com/GerryCrooked/statusbar/blob/main/integrations/microsoft-365/README.md) |
| `WEBHOOK_TOKEN` | Only for set-ups from before 0.5 (`/api/webhooks/incidents`); new set-ups use integrations |

## Settings in the web UI

Under **Settings**:

| Section | What you set | More |
|---|---|---|
| **Branding** | Name, subtitle, colour, UI language, logo | [Appearance and sounds](appearance.md) |
| **Bar on the desktops** | Position, height, how long the all-clear shows, monitors, full-screen and Remote Desktop behaviour, labels | [Appearance and sounds](appearance.md#the-bar) |
| **Sounds** | A sound per kind of message | [Appearance and sounds](appearance.md#sounds) |
| **Silent alarm** | On/off, key combination, texts, recipients, limits | [Silent alarm](silent-alarm.md) |
| **Notification groups** | Bundle recipients under one name | [Recipients](recipients.md#notification-groups) |
| **Messages from your service provider** | Show right away, hold for approval, do not accept (with a hub only) | [For IT service providers](service-provider-hub.md) |
| **License** | State of the license | [License](license.md) |

Integrations and rules are under **Monitoring** – see [Connect your monitoring](monitoring.md).
