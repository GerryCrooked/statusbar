---
id: server
title: Install and run the server
description: Install the Notiflag server with Docker, make it reachable over HTTPS, update and back it up.
lang: en
slug: server-installation
order: 3
---

# Install and run the server

The Notiflag server is a single container with a SQLite database. It needs an HTTPS reverse proxy in front and
nothing else.

## Requirements

| | |
|---|---|
| Operating system | Linux with Docker Engine and Docker Compose (plugin `docker compose`) |
| Size | A small VM is enough; the database grows with the number of messages, not of PCs |
| Inbound | HTTPS (443) at the reverse proxy: from all Windows PCs and from the monitoring systems that send messages |
| Outbound | only when used: license portal `licensestatusbar.it-klein.com` (with `LICENSE_KEY`), your service provider's hub (`HUB_URL`), Microsoft Graph (Microsoft 365), your domain controller (Active Directory) |
| Certificate | One the Windows PCs trust (public, or from your internal CA) |

## Installation

There are two ways. Both use `compose.yaml` from the repository and a volume `status-data` for the data.

### Option A: Released image (recommended)

The released image contains the server and the Windows installer `Statusbar.msi`.

```bash
git clone https://github.com/GerryCrooked/statusbar.git notiflag && cd notiflag
scripts/setup.sh                       # creates .env with random tokens
```

Create `compose.override.yaml` next to it so Compose pulls the image instead of building:

```yaml
services:
  statusbar:
    image: ghcr.io/gerrycrooked/statusbar:latest   # or a fixed version: :<version>
    build: !reset null
```

```bash
docker compose pull && docker compose up -d
```

Tags: `latest` = latest release, `<version>` = a specific release, `edge` = current development state (not for
production).

### Option B: Build from source

```bash
git clone https://github.com/GerryCrooked/statusbar.git notiflag && cd notiflag
scripts/setup.sh
docker compose up -d --build
```

A self-built server contains no Windows installer. Put `Statusbar.msi` at `/data/Statusbar.msi` in the volume (or
set `CLIENT_MSI`) so the **Computers** page offers the package – see [Windows client](windows-client.md#build-the-client).

### First start

```bash
docker compose ps          # status "healthy"
curl http://127.0.0.1:8080/health
# ok
```

Sign in at `http://127.0.0.1:8080` with the `ADMIN_TOKEN` from `.env` (or with a directory account, see [Active
Directory](active-directory.md)). Every setting in `.env` is explained in [Configuration](configuration.md).

The container runs with a read-only file system, no capabilities and `no-new-privileges`; only `/data` is writable.

## HTTPS reverse proxy

The server speaks HTTP and by default listens on `127.0.0.1:8080` only. Put a reverse proxy with TLS in front.
Important:

- **Allow long requests.** Clients keep a connection open for up to 55 seconds (long-poll). A read timeout of at least
  60 seconds is needed; most proxies have that by default.
- **Set `X-Forwarded-For`.** The server logs the sender address of a silent alarm with it.
- **Do not buffer.** Responses are small; buffering only adds delay.

**Caddy** (gets the certificate itself):

```caddy
status.example.com {
    reverse_proxy 127.0.0.1:8080
}
```

**nginx**:

```nginx
server {
    listen 443 ssl;
    server_name status.example.com;
    ssl_certificate     /etc/ssl/status.example.com.crt;
    ssl_certificate_key /etc/ssl/status.example.com.key;

    location / {
        proxy_pass http://127.0.0.1:8080;
        proxy_set_header Host $host;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_read_timeout 75s;
        proxy_buffering off;
    }
}
```

If the proxy runs in another container, set `BIND_ADDRESS` in `.env` to an address it can reach, or put both
containers on the same Docker network.

**Who needs to reach the server?**

| From | Path |
|---|---|
| Windows clients | `/api/client/…`, `/api/sounds/…` |
| Monitoring, ticket systems, scripts | `/api/hooks/…` |
| Admins in the browser | everything else |

Cloud services (Azure Monitor, Jira Cloud, NinjaOne, Datto RMM …) send from the internet. To keep the web UI
internal, expose only `/api/hooks/` to the outside.

## Updating

**Released image:**

```bash
docker compose pull && docker compose up -d
```

**From source:**

```bash
git pull --ff-only && docker compose build --no-cache && docker compose up -d
```

The database is migrated automatically at start. Server and clients can be updated independently; update the
server first, then the clients (new MSI through your deployment tool).

## Backup and restore

All data – messages, settings, logo, own sounds – is in the database `/data/statusbar.sqlite` in the volume
`status-data`, plus any own language files in `/data/locales`. Back up the whole volume:

```bash
docker compose stop
docker run --rm -v notiflag_status-data:/data -v "$PWD":/backup alpine \
  tar czf /backup/notiflag-data-$(date +%F).tgz -C /data .
docker compose start
```

The volume is named `<folder name>_status-data` (here `notiflag`); `docker volume ls` shows it. To restore: stop the
container, unpack the archive into the (empty) volume, start. Back up `.env` as well – without the tokens every
client has to be redeployed.

Without stopping, if `sqlite3` is installed on the host:
`sqlite3 <volume path>/statusbar.sqlite ".backup '/backup/statusbar.sqlite'"`.

## Moving the server

1. Back up the old server (see above), take `.env` along.
2. Install on the new server, restore the volume, start with the same `.env`.
3. Point the server's DNS name at the new address. Clients and monitoring notice nothing.

If the address changes, redeploy the MSI with `SERVERURL=<new>`: a running client picks up the new value within half
a minute.

## Logs and status

```bash
docker compose logs -f statusbar                         # everything
docker compose logs statusbar | grep "monitoring event"  # incoming monitoring events
```

`GET /health` answers `ok`, `GET /api/meta` returns version and name. The web UI shows the state of the license, the
hub connection and Microsoft 365 under **Settings** and **Monitoring**.

## Upgrading from 0.4

From 0.5 on, the server migrates an existing database at first start (messages, branding and logo; the old tables
are kept with a `_v04` suffix) and still answers 0.4 clients. Plan for:

- The default database path is now `/data/statusbar.sqlite`; set `DATABASE_PATH` to keep an existing file.
- `AD_ADMIN_GROUP` no longer has an organisation-specific default – set it explicitly.
- Bar settings moved from `BAR_*` variables to the web UI (existing values are read once as initial defaults).
- "Custom groups" from 0.4 become notification groups at first start.
- `WEBHOOK_TOKEN` and `/api/webhooks/incidents` keep working if the variable is set; new set-ups use integrations.
- The client is now `statusbar-client.exe` with its configuration in `%ProgramData%\Statusbar\client.json`.
