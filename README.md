# Statusbar

**Tell every desk what is going on – before the phones start ringing.**

Statusbar puts a slim coloured bar at the edge of your users' Windows desktops when something is down, degraded or
planned, and takes it away with a green all-clear when it is over. Messages come from your monitoring
automatically, or from a person with one click.

* **Self-hosted and small.** One server binary with SQLite, one native Windows executable. No agents, no runtime,
  no cloud.
* **Instant.** Clients hold a long-poll connection; a new message is on screen in about a second.
* **Driven by monitoring.** Zabbix, Checkmk, PRTG, Nagios/Icinga, Prometheus Alertmanager, Grafana, Uptime Kuma, LibreNMS, Azure Monitor, NinjaOne, Datto RMM, N-able N-central, Tactical RMM or any
  script post to a webhook, and so do ticket systems (Zammad, GLPI, Znuny/OTRS, Jira Service Management,
  ServiceNow, TOPdesk) for known outages; Microsoft 365 service health is polled by the server itself. Problems open a message,
  recoveries resolve it.
* **Speaks human.** Rules turn `mail01/SMTP CRITICAL` into "E-mail is disrupted. We are working on it."
  Everything without a rule waits for a one-click approval, and short blips are filtered by a delay.
* **Targeted.** Everyone, computers by name pattern (`PC-ACCOUNTING-*`), or – with Active Directory – groups,
  OUs and users. Notification groups bundle these under one name, such as *Finance team*.
* **Traceable.** Every message records who published, approved and resolved it.
* **Unobtrusive.** A real desktop toolbar: maximised windows move out of the way instead of being covered.
  Multi-monitor and high-DPI aware.
* **Your brand.** Name, colour, logo, language and all texts on the bar are yours.
* **Ready for service providers.** A server can connect to a provider's hub (separate product) over one outgoing
  connection: the provider sees its state and computers and can send messages to the whole organisation or a
  notification group. The customer decides whether these show right away, wait for approval or are refused.
  See [For IT service providers](docs/en/service-provider-hub.md).
* **Tickets from messages.** Optionally, every published message or silent alarm opens a ticket in Zammad, GLPI,
  Znuny/OTRS, Jira Service Management, ServiceNow, TOPdesk or any system with a webhook – closed with a note when
  the message is resolved, never looping back as a second message. See [Statusbar → ticket system](docs/INTEGRATIONS.md#statusbar--ticket-system-a-ticket-for-every-message).
* **Overview of computers.** Every computer with the client, its last user, client version and last contact.
* **Silent alarm (optional).** A user presses a key combination such as `Ctrl+Shift+F12` and everyone, or chosen
  notification groups, get a message like "Silent alarm from anna at PC-RECEPTION-01". Nothing appears on the
  computer where it was pressed, not even in its own bar. It has its own colour (violet), label and sound,
  keeps the left part of the bar while other messages rotate, and ends only on the server or in the hub.
  Off by default; see [Silent alarm](docs/en/silent-alarm.md).
* **Sounds (optional).** Each kind – silent alarm, outage, degraded, notice, all-clear – can play a standard
  Windows sound or your own WAV file when a new message appears.

## Documentation

Complete documentation in English and German: [docs/](docs/README.md) –
[English](docs/en/index.md) · [Deutsch](docs/de/index.md).

## Quick start

**1. Server** (Linux with Docker):

```bash
git clone <this repository> statusbar && cd statusbar
scripts/setup.sh                # creates .env with random tokens
docker compose up -d --build
```

Open `http://127.0.0.1:8080`, sign in with the `ADMIN_TOKEN` from `.env`. For real use put an HTTPS reverse proxy
in front (the server listens on localhost only by default).

**2. Client** (Windows 10/11): build or download `statusbar-client.exe` (see [Windows client](docs/en/windows-client.md)), then

```powershell
.\scripts\install-client.ps1 -ExePath .\statusbar-client.exe -ServerUrl https://status.example.com -Token '<CLIENT_TOKEN>'
```

**3. Monitoring:** *Monitoring → Add integration* gives you a webhook URL. Try it:

```bash
curl -X POST 'https://status.example.com/api/hooks/<token>' -H 'Content-Type: application/json' \
  -d '{"status":"problem","host":"mail01","service":"SMTP"}'
```

Step-by-step guides for Zabbix, Checkmk, PRTG, Nagios/Icinga, Alertmanager, Grafana, Uptime Kuma, LibreNMS, Microsoft 365 service health, Power Automate, Azure Monitor, the RMMs NinjaOne, Datto RMM, N-able N-central, Atera, Tactical RMM (and others via e-mail), the ticket systems Zammad, GLPI, Znuny/OTRS, Jira Service Management, ServiceNow, TOPdesk, and curl:
[docs/INTEGRATIONS.md](docs/INTEGRATIONS.md).

## How a message travels

```text
 Zabbix / Checkmk / Nagios / Alertmanager / Uptime Kuma / curl        Admin in the web UI
                     │  POST /api/hooks/<token>                              │
                     ▼                                                       ▼
        rule matches? ── yes ──► publish (after optional delay) ◄──── "Publish"
              │ no
              ▼
   hold for approval / publish as received / ignore
                                   │
                                   ▼   long-poll, ~1 s
                     Windows clients show the bar
                                   │
   recovery from monitoring / "Resolve" / scheduled end ──► green all-clear ──► bar disappears
```

## Configuration

Server environment (`.env`):

| Variable | Purpose |
|---|---|
| `ADMIN_TOKEN`, `CLIENT_TOKEN` | Required secrets, at least 32 characters, different from each other |
| `HTTP_PORT`, `BIND_ADDRESS` | Published port of the container (default `127.0.0.1:8080`) |
| `RETENTION_DAYS` | How long resolved messages are kept (default 90) |
| `AD_ENABLED` and `AD_*` | Optional Active Directory sign-in and targeting, see `.env.example` |
| `LOCALES_DIR` | Folder with extra UI languages as `<code>.json` (default `/data/locales`) |
| `HUB_URL`, `HUB_TOKEN` | Connect this server to a service provider's hub |
| `LICENSE_KEY` | License key (`SBL-…`), needed beyond 10 computers without a hub, see [License key](#license-key) |
| `M365_TENANT_ID`, `M365_CLIENT_ID`, `M365_CLIENT_SECRET` and `M365_*` | Optional Microsoft 365 service health, see [integrations/microsoft-365](integrations/microsoft-365/README.md) |
| `CLIENT_MSI` | Installer offered on the *Computers* page (default: `/data/Statusbar.msi`, else the one shipped in the image) |

Everything else – position and size of the bar, how long the all-clear stays, labels, branding, language, the
silent alarm, sounds – is set under *Settings* in the web UI and applies to all clients immediately.

To add a UI language, copy `server/locales/en.json`, translate the values and place it as `/data/locales/<code>.json`.

## API in one glance

| | |
|---|---|
| `POST /api/hooks/<token>` | Monitoring webhook (problem / recovery) |
| `GET  /api/client/state?user=&computer=&rev=&wait=` | What a desktop should show (Bearer `CLIENT_TOKEN`, long-poll) |
| `POST /api/client/alarm` `{"user","computer"}` | Silent alarm from a desktop (Bearer `CLIENT_TOKEN`, only when switched on) |
| `GET/POST /api/admin/incidents`, `POST …/{id}/resolve`, `POST …/{id}/publish`, `DELETE …/{id}` | Messages (Bearer `ADMIN_TOKEN` or session) |
| `GET/POST /api/admin/integrations`, `POST /api/admin/rules` | Monitoring set-up |
| `GET/PUT /api/admin/settings`, `PUT/DELETE /api/admin/logo` | Appearance |
| `GET /api/admin/sounds`, `PUT/DELETE /api/admin/sounds/{kind}` | Own WAV sounds per kind (`alarm`, `error`, `warning`, `info`, `resolved`) |
| `GET /api/sounds/{kind}` | An uploaded sound (Bearer `CLIENT_TOKEN` or admin session) |
| `GET /api/admin/computers` | Computers with the client: name, last user, client version, last contact |
| `GET/PUT /api/admin/ticketing`, `POST …/test`, `POST …/retry` | Outbound tickets: configuration, status, test ticket |
| `GET/POST /api/admin/teams`, `PUT/DELETE …/{id}` | Notification groups |
| `GET /health`, `GET /api/meta` | Health and version |

Create a message from a script:

```bash
curl -X POST https://status.example.com/api/admin/incidents -H "Authorization: Bearer $ADMIN_TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"title":"Maintenance tonight","message":"ERP offline 20:00-21:00","severity":"info","targets":["host:PC-ACC-*"]}'
```

Optional `starts_at` / `ends_at` (Unix seconds) schedule a message and resolve it automatically.

## Upgrading from 0.4

The 0.5 server migrates an existing database on first start (messages, branding and logo are carried over, the old
tables are kept with a `_v04` suffix) and still answers 0.4 clients, so server and clients can be updated
independently. Changes to plan for:

* The default database path is now `/data/statusbar.sqlite`; set `DATABASE_PATH` to keep an existing file.
* `AD_ADMIN_GROUP` no longer has an organisation-specific default – set it explicitly.
* Bar settings moved from `BAR_*` variables to the web UI (existing values are read once as initial defaults).
* "Custom groups" from 0.4 are taken over as notification groups on first start.
* `WEBHOOK_TOKEN` and `/api/webhooks/incidents` keep working if the variable is set; new set-ups should use integrations.
* The client is now `statusbar-client.exe` with its configuration in `%ProgramData%\Statusbar\client.json`.
* The experimental remote-desktop window resizing is gone; see [Windows client](docs/en/windows-client.md#full-screen-windows-and-remote-desktop).

## Development

```bash
cd server && cargo test && cargo run        # needs ADMIN_TOKEN, CLIENT_TOKEN, DATABASE_PATH in the environment
scripts/smoke-test.sh                       # end-to-end check against a running server
```

The server is a library (`statusbar_core`) plus a thin binary. Add-ons run the same server with an `Extension`
(extra routes, recipient kinds, tables and an admin UI script loaded as `/ext.js`, which calls `Statusbar.extend`).

Layout: `server/` (Rust, Axum, SQLite; web UI in `server/static/`), `client/` (Rust, Win32),
`installer/` (MSI), `integrations/` (one folder per monitoring system: guide, scripts, sample payloads), `scripts/`, `docs/`.

Limits and security model: [Security and privacy](docs/en/security-and-privacy.md).

## License key

Statusbar is free for up to 10 computers, and for any number when the server is connected to a service
provider's hub (`HUB_URL`). Beyond that, enter a license key as `LICENSE_KEY` and restart. Hubs always need a
hub license.

Nothing is ever blocked: without a valid license the web UI shows a notice, messages keep working.

The server asks the license portal (`https://licensestatusbar.it-klein.com`, `LICENSE_URL` to override) every
6 hours. It sends the key, a random id of this installation, server version, the display name from the
settings and the number of computers seen in the last 30 days – no user names, no messages. Answers are
signed (Ed25519, public key built in, `server/src/license.rs`); the last valid one stays good for 14 days, so
an unreachable portal does not matter for that long. Without `LICENSE_KEY` nothing is sent.

## Licence

The code in this repository is licensed under the GNU Affero General Public License, version 3 or later (AGPL-3.0-or-later), see
`LICENSE`. Features for service providers (central hub, multi-customer messaging, white-label builds) are
available under a separate commercial licence.

External code contributions are not accepted at the moment; bug reports and ideas are welcome as issues.

If Statusbar saves you a few phone calls: [buy me a coffee](https://buymeacoffee.com/crookedgerry).
