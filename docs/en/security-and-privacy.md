---
id: security
title: Security and privacy
description: Security model, stored data, data flows and limits of Notiflag – read before rolling out.
lang: en
slug: security-and-privacy
order: 14
---

# Security and privacy

Read this before rolling out. Notiflag is deliberately small: a server you run yourself, without cloud and without
telemetry.

## Security model

| Topic | How |
|---|---|
| **Transport** | The server speaks HTTP behind a TLS reverse proxy and by default listens on `127.0.0.1` only. The client refuses servers without HTTPS (except `localhost`). |
| **Web UI sign-in** | With `ADMIN_TOKEN` or a directory account from `AD_ADMIN_GROUP`. Session cookie `Secure`, `HttpOnly`, `SameSite=Strict`, valid for `ADMIN_SESSION_HOURS`. Failed sign-ins are delayed; there is no account lockout. |
| **Tokens** | `ADMIN_TOKEN` (everything) and `CLIENT_TOKEN` (read messages, raise the silent alarm) live in `.env`: random, at least 32 characters, different. Integration tokens are shown once and stored as SHA-256 hashes only; an integration token can only open and resolve its own messages. |
| **Shared client token** | All desktops use the same `CLIENT_TOKEN`, readable by local users. The client states its own user and computer name. Recipients therefore control **who is shown** a message, not who **could** read it. No confidential content in messages. |
| **Directory** | LDAP with StartTLS or LDAPS and certificate verification, read-only account, direct group memberships only. |
| **Container** | Read-only file system, no capabilities, `no-new-privileges`; only `/data` is writable. |
| **Desktop client** | Runs per user session without admin rights. It only creates and positions its own windows (the one exception: the explicitly experimental Remote Desktop option). Sign executable and MSI before distribution. |
| **Sounds** | Only admins upload WAV files (up to 1 MB, checked for a RIFF/WAVE header). Values other than the offered Windows sound names are dropped. |
| **Offline** | If the server is unreachable, clients keep the last state and never invent an all-clear. |

**Recommendations:** restrict the web UI to the admin network through your network or reverse proxy; expose at most
`/api/hooks/` to the outside; `.env` with permissions `600` (`scripts/setup.sh` does this); rotate tokens when in doubt.

## Not an alerting system

Delivery is best effort; there is no acknowledgement. Do not use Notiflag as the only channel for safety-critical
alerts – this applies to the [silent alarm](silent-alarm.md) in particular.

## What data is stored

Everything is in one SQLite file on your server:

| Data | Content | Retention |
|---|---|---|
| Messages | Title, text, severity, recipients, times, who created/approved/resolved (user name, `api`, integration, service provider, `alarm:<user>@<computer>`) | resolved: `RETENTION_DAYS` (default 90) days |
| Computers | Computer name, last signed-in user, client version, last contact | shown and counted for 30 days; one record per computer is kept (currently without automatic deletion) |
| Settings | Branding, bar, sounds, silent alarm, notification groups, integrations (token as hash), rules | until changed |
| Directory | User names, groups, OUs from AD | in memory only, re-read regularly |

**Personal data** are mainly user names (last user per computer, admins in the history, who raised a silent alarm)
and computer names that point to a person. The usual legal basis is the legitimate interest in running IT; for the
silent alarm, inform the users and, where applicable, involve the works council and data protection officer.

## What data leaves the server

| To | Only if | What |
|---|---|---|
| License portal | `LICENSE_KEY` set | Key, random installation id, version, name, number of computers – see [License](license.md#what-is-sent-to-the-license-portal) |
| Service provider's hub | `HUB_URL` set | Version, name, computer list, titles of active messages, group names – see [For IT service providers](service-provider-hub.md#what-is-sent-to-the-hub) |
| Microsoft Graph | `M365_*` set | Reads your tenant's service health issues; nothing is sent to Microsoft apart from the sign-in |
| Domain controller | `AD_ENABLED=yes` | Read-only LDAP queries |

Nothing else: no telemetry, no usage statistics, no external fonts or scripts in the web UI.

## Logs

- **Server log** (`docker compose logs`): starts, monitoring events (integration, key, outcome), directory sync,
  silent alarms with user, computer and sender address. Rotation through Docker's log settings.
- **Client log** `%LOCALAPPDATA%\Statusbar\client.log`: start, connection, display decisions. Never the token.

There is no separate audit log of settings changes yet.

## Backups

The SQLite file in the volume contains all data – see [Backup and restore](server-installation.md#backup-and-restore).
Treat backups and `.env` like the tokens themselves.

## Reporting security issues

Please not as a public issue, but confidentially to the contact address on [notiflag.de](https://notiflag.de/en/).
