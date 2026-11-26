---
id: troubleshooting
title: Troubleshooting
description: Common problems with server, client, monitoring, Active Directory and license – and how to fix them.
lang: en
slug: troubleshooting
order: 16
---

# Troubleshooting

The two most important sources:

- **Server log**: `docker compose logs --since 1h statusbar`
- **Client log**: `%LOCALAPPDATA%\Statusbar\client.log` in the affected user's profile

## Server

| Symptom | Cause and fix |
|---|---|
| Container does not start, log names `ADMIN_TOKEN` or `CLIENT_TOKEN` | Both must be set, at least 32 characters and different. `scripts/setup.sh` creates suitable values |
| Container does not start: `M365_CLIENT_ID is missing` | `M365_TENANT_ID` is set, but not all three Microsoft 365 values |
| Web UI not reachable from other machines | Default is `BIND_ADDRESS=127.0.0.1`. Put a reverse proxy in front (recommended) or change `BIND_ADDRESS` |
| Sign-in fails | Without AD: enter exactly the `ADMIN_TOKEN` from `.env`. With AD: see below |
| **Computers** page offers no installer | Self-built server without MSI: put `Statusbar.msi` at `/data/Statusbar.msi` or use the released image |

## Client

| Symptom | Cause and fix |
|---|---|
| PC does not appear under **Computers** | Client not running (autostart only from the next sign-in), wrong `SERVERURL`, proxy/firewall, or the certificate is not valid for Windows. Check the client log |
| Client log: `server_url must start with https://` | The client only talks HTTPS (except to `localhost`). Use an address with `https://` |
| Client log: `401` | Wrong `CLIENTTOKEN` – run the MSI again with the current token |
| PC is connected but does not show a message | Recipients do not match: check computer name, user name or AD membership; for scheduled messages mind *Show from* |
| No bar in a full-screen Remote Desktop session | Mode *Hide* without the client on the session host. Install it there or choose *Show the bar on top of it* |
| Bar covers window content in full screen | As designed with *Stay visible above full-screen windows*. Switch it off to let full-screen windows cover the bar |
| No sound | Sound for this kind set to *No sound*, own sound not uploaded yet, computer muted, or the message was already there when the client started |
| Silent alarm does not trigger | Not switched on, key combination taken by another program or character missing in the keyboard layout (client log), cooldown after the last alarm, or the limit of 5 alarms in 10 minutes reached (server log) |
| No connection after moving the server | Deploy the new address with the MSI (`SERVERURL=<new>`) or point the old DNS name at the new server |

## Monitoring

The most common cases – `401`, `400 JSON body required`, message does not go away, rule does not match – are listed
with their fixes in [Connect your monitoring → Troubleshooting](monitoring.md#troubleshooting).

## Active Directory

| Symptom | Cause and fix |
|---|---|
| Log: `certificate verify failed` | The container does not trust the domain controller's CA – see [Active Directory → certificates](active-directory.md#encryption-and-certificates) |
| Log: `AD_URL must use ldap:// (StartTLS) or ldaps://` | Fix the scheme in `AD_URL` |
| Sign-in with a directory account fails | Account not a direct member of `AD_ADMIN_GROUP` (the group's name, not its DN), not covered by `AD_USER_FILTER`, or wrong password |
| Group messages do not reach a user | Only direct memberships count; the sync runs every `AD_SYNC_SECONDS` |
| Recipient picker shows no groups | `AD_ENABLED=yes` missing or the sync fails (log) |

## License and hub

| Symptom | Cause and fix |
|---|---|
| *The license portal has not been reachable since …* | Allow outbound HTTPS to `licensestatusbar.it-klein.com`. The license stays valid for 14 days from the last confirmation |
| *meant for the other kind (server/hub)* | A hub key in a server, or the other way round |
| *Not connected to the service provider* | `HUB_URL` wrong, `HUB_TOKEN` revoked (customer deleted in the hub) or outbound HTTPS blocked |
| Messages from the service provider do not appear | *Messages from your service provider* is set to *Hold for approval* or *Do not accept* |

## Getting help

Bugs and ideas: [issues on GitHub](https://github.com/GerryCrooked/statusbar/issues). Include the version
(`/api/meta`), the relevant lines from server and client log (without tokens) and your steps. Licensed customers reach
support through [notiflag.de](https://notiflag.de/en/).
