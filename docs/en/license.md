---
id: license
title: License
description: Free up to 10 PCs, beyond that with a license key – how PCs are counted, how to enter the key and what is sent to the license portal.
lang: en
slug: license
order: 13
---

# License

| | |
|---|---|
| **Up to 10 PCs** | free, all features, no expiry |
| **From 11 PCs** | license key needed – prices and purchase at [notiflag.de](https://notiflag.de/en/) |
| **Customer servers on a hub** | no license of their own, any number of PCs |
| **Hub for IT service providers** | hub license, partner terms on request |

**Nothing is ever blocked.** Without a valid license the web UI shows a notice; messages keep working without
restriction.

## How PCs are counted

Counted are the computers that connected to the server with the client in the last 30 days. The **Computers** page
lists them with name, last user, client version and last contact. A terminal server counts as one computer.

## Enter the license key

After purchase the key (`SBL-…`) arrives by e-mail. In the server's `.env`:

```dotenv
LICENSE_KEY=SBL-…
```

```bash
docker compose up -d
```

**Settings → License** then shows "Licensed to …", the number of licensed and connected computers and the expiry date.

## States and notices

| Notice | Meaning and fix |
|---|---|
| *No license – free up to 10 computers* | All fine up to 10 PCs |
| *{n} computers use Notiflag … please enter a license* | More than 10 PCs without a key |
| *The license covers {max} computers, {n} are connected* | More PCs than licensed – extend the license |
| *The license key does not work: expired / revoked / unknown key* | Check, renew, or ask support |
| *meant for the other kind (server/hub)* | A hub key in a server, or the other way round |
| *The license portal cannot be reached right now. The license stays valid until …* | Nothing to do until that date |
| *The license portal has not been reachable since …* | Check the outbound connection to `licensestatusbar.it-klein.com` (HTTPS) |

## What is sent to the license portal

Only with `LICENSE_KEY` – without a key the server sends nothing. Every 6 hours it asks
`https://licensestatusbar.it-klein.com` and sends:

- the license key,
- a random id of this installation,
- the server version,
- the name from the settings,
- the number of computers in the last 30 days.

No user names, no computer names, no messages. Answers are signed (Ed25519, public key built into the server). The
last valid answer stays good for 14 days – an unreachable portal does not matter for that long.

## Open source

The code of server and client is licensed under the GNU Affero General Public License v3 or later
(AGPL-3.0-or-later) in the repository [GerryCrooked/statusbar](https://github.com/GerryCrooked/statusbar). Features for
service providers (central hub, messages to several customers, white-label builds) are available under a separate
commercial licence.
