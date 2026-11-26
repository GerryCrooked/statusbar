---
id: quickstart
title: Quick start
description: Start a Notiflag server, connect a Windows client and send the first message – in about 15 minutes.
lang: en
slug: quickstart
order: 2
---

# Quick start

In about 15 minutes a test server runs, a Windows PC shows the bar, and a message from the command line appears and
goes away again. For production, continue with [Install and run the server](server-installation.md).

You need:

- a Linux machine with Docker and Docker Compose (a small VM is enough),
- a Windows 10/11 PC for testing,
- for a test across the network: an HTTPS address for the server. The client only talks HTTPS, except to `localhost`.

## 1. Start the server

```bash
git clone https://github.com/GerryCrooked/statusbar.git notiflag
cd notiflag
scripts/setup.sh                 # creates .env with two random tokens
docker compose up -d --build
```

`scripts/setup.sh` writes two secrets to `.env`:

| Token | Used for |
|---|---|
| `ADMIN_TOKEN` | Signing in to the web UI and the admin API – treat it like an administrator password |
| `CLIENT_TOKEN` | The Windows clients read their messages with it |

Open `http://127.0.0.1:8080` on the server (or through an SSH tunnel) and sign in with the `ADMIN_TOKEN`.

The server only listens on `127.0.0.1`. For Windows PCs to reach it, put an HTTPS reverse proxy in front – examples
for Caddy and nginx are in [Install and run the server](server-installation.md#https-reverse-proxy).

## 2. Connect a Windows client

In the web UI under **Computers**:

1. **Download the package for this server** – a ZIP with `Statusbar.msi`, `install.cmd` and `uninstall.cmd`, already
   filled in with the server address and client token.
2. Unpack it on the test PC and run `install.cmd` as administrator.
3. Sign out and in again (or start `C:\Program Files\Statusbar\statusbar-client.exe` once).

After a few seconds the PC is listed under **Computers** with user and client version.

## 3. The first message

Under **Messages → New message**: title "Test", severity *Notice*, **Publish**. The bar appears on the test PC.
**Resolve** turns it green; after 90 seconds it is gone.

## 4. A message from monitoring

Under **Monitoring → Add integration**: name `Test`, *Alerts without a matching rule* = **Publish as received**,
delay `0`, **Create**. Notiflag shows the webhook URL once – copy it.

```bash
URL='https://status.example.com/api/hooks/sb_…'      # the copied URL
curl -X POST "$URL" -H 'Content-Type: application/json' \
  -d '{"status":"problem","host":"mail01","service":"SMTP","message":"Port 25 does not answer"}'
# {"events":["created"],"ok":true}  → the bar appears
curl -X POST "$URL" -H 'Content-Type: application/json' \
  -d '{"status":"ok","host":"mail01","service":"SMTP"}'
# {"events":["resolved"],"ok":true} → green all-clear
```

## Next steps

- [Install and run the server](server-installation.md): HTTPS, updates, backup
- [Windows client](windows-client.md): rollout with Group Policy, Intune or a deployment tool
- [Connect your monitoring](monitoring.md): Zabbix, Checkmk, PRTG, Microsoft 365 and more
- [Appearance and sounds](appearance.md): name, colour, logo, position of the bar
