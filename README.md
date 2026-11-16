# Statusbar

**Tell every desk what is going on – before the phones start ringing.**

Statusbar puts a slim coloured bar at the edge of your users' Windows desktops when something is down, degraded or
planned, and takes it away with a green all-clear when it is over. Messages come from your monitoring
automatically, or from a person with one click.

* **Self-hosted and small.** One server binary with SQLite, one native Windows executable. No agents, no runtime,
  no cloud.
* **Instant.** Clients hold a long-poll connection; a new message is on screen in about a second.
* **Driven by monitoring.** Zabbix, Checkmk, Nagios/Icinga, Prometheus Alertmanager, Grafana, Uptime Kuma or any
  script post to a webhook. Problems open a message, recoveries resolve it.
* **Speaks human.** Rules turn `mail01/SMTP CRITICAL` into "E-mail is disrupted. We are working on it."
  Everything without a rule waits for a one-click approval, and short blips are filtered by a delay.
* **Targeted.** Everyone, computers by name pattern (`PC-ACCOUNTING-*`), or – with Active Directory – groups,
  OUs and users.
* **Unobtrusive.** A real desktop toolbar: maximised windows move out of the way instead of being covered.
  Multi-monitor and high-DPI aware.
* **Your brand.** Name, colour, logo, language and all texts on the bar are yours.

## Quick start

**1. Server** (Linux with Docker):

```bash
git clone <this repository> statusbar && cd statusbar
scripts/setup.sh                # creates .env with random tokens
docker compose up -d --build
```

Open `http://127.0.0.1:8080`, sign in with the `ADMIN_TOKEN` from `.env`. For real use put an HTTPS reverse proxy
in front (the server listens on localhost only by default).

**2. Client** (Windows 10/11): build or download `statusbar-client.exe` (see [docs/CLIENT.md](docs/CLIENT.md)), then

```powershell
.\scripts\install-client.ps1 -ExePath .\statusbar-client.exe -ServerUrl https://status.example.com -Token '<CLIENT_TOKEN>'
```

**3. Monitoring:** *Monitoring → Add integration* gives you a webhook URL. Try it:

```bash
curl -X POST 'https://status.example.com/api/hooks/<token>' -H 'Content-Type: application/json' \
  -d '{"status":"problem","host":"mail01","service":"SMTP"}'
```

Recipes for each monitoring system: [docs/INTEGRATIONS.md](docs/INTEGRATIONS.md).

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
| `CLIENT_MSI` | Installer offered on the *Computers* page (default `/data/Statusbar.msi`) |

Everything else – position and size of the bar, how long the all-clear stays, labels, branding, language – is set
under *Settings* in the web UI and applies to all clients immediately.

To add a UI language, copy `server/locales/en.json`, translate the values and place it as `/data/locales/<code>.json`.

## API in one glance

| | |
|---|---|
| `POST /api/hooks/<token>` | Monitoring webhook (problem / recovery) |
| `GET  /api/client/state?user=&computer=&rev=&wait=` | What a desktop should show (Bearer `CLIENT_TOKEN`, long-poll) |
| `GET/POST /api/admin/incidents`, `POST …/{id}/resolve`, `POST …/{id}/publish`, `DELETE …/{id}` | Messages (Bearer `ADMIN_TOKEN` or session) |
| `GET/POST /api/admin/integrations`, `POST /api/admin/rules` | Monitoring set-up |
| `GET/PUT /api/admin/settings`, `PUT/DELETE /api/admin/logo` | Appearance |
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
* "Custom groups" are replaced by selecting several recipients per message.
* `WEBHOOK_TOKEN` and `/api/webhooks/incidents` keep working if the variable is set; new set-ups should use integrations.
* The client is now `statusbar-client.exe` with its configuration in `%ProgramData%\Statusbar\client.json`.
* The experimental remote-desktop window resizing is gone; see "Full-screen windows" in [docs/CLIENT.md](docs/CLIENT.md).

## Development

```bash
cd server && cargo test && cargo run        # needs ADMIN_TOKEN, CLIENT_TOKEN, DATABASE_PATH in the environment
scripts/smoke-test.sh                       # end-to-end check against a running server
```

Layout: `server/` (Rust, Axum, SQLite; web UI in `server/static/`), `client/` (Rust, Win32),
`installer/` (MSI), `integrations/` (monitoring recipes), `scripts/`, `docs/`.

Limits and security model: [docs/SECURITY.md](docs/SECURITY.md).

## Licence

The code in this repository is licensed under the GNU Affero General Public License v3.0 (AGPL-3.0-only), see
`LICENSE`. Features for service providers (central hub, multi-customer messaging, white-label builds) are
available under a separate commercial licence.

External code contributions are not accepted at the moment; bug reports and ideas are welcome as issues.
