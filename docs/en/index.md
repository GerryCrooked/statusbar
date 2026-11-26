---
id: overview
title: What is Notiflag?
description: Notiflag shows every desk a status bar when something is down, degraded or planned – automatically from your monitoring or with one click.
lang: en
slug: overview
order: 1
---

# What is Notiflag?

Notiflag puts a slim coloured bar at the edge of your users' Windows desktops when something is down, degraded or
planned – and takes it away with a green all-clear when it is over. Users know before the phones start ringing.

Messages come from your monitoring automatically (Zabbix, Checkmk, PRTG, Microsoft 365 and more than 20 other
systems) or from an admin with one click.

## The parts

```text
 Monitoring, ticket system, script          Admin in the browser
          │  webhook                              │
          ▼                                       ▼
 ┌──────────────────────────────────────────────────────┐
 │ Notiflag server (Docker, one SQLite file)            │
 │  messages · rules · recipients · branding · sounds   │
 └───────────────────────────┬──────────────────────────┘
                             │ HTTPS, long-poll (~1 second)
                             ▼
          Windows client on every desk
          (shows the bar, optional: silent alarm)
```

| Part | What it does | Where it runs |
|---|---|---|
| **Server** | Takes messages in, decides who sees them, delivers them. Web UI for admins. | Linux with Docker, on your premises or in your cloud |
| **Windows client** | Shows the bar at the top or bottom of the screen, plays sounds, raises the silent alarm. | Every Windows 10/11 desk and every terminal server |
| **Hub** (optional, for IT service providers) | Connects the servers of many customers: overview and messages to one, several or all customers. | At the service provider |

## What Notiflag does

- **Self-hosted and small.** One server with SQLite, one Windows program. No cloud, no runtime on the desks, no
  service with admin rights.
- **Instant.** A new message is on every screen in about a second.
- **Driven by monitoring.** A problem opens a message, the recovery resolves it. Guides for monitoring systems, RMM
  tools, ticket systems and Microsoft 365: [Connect your monitoring](monitoring.md).
- **Speaks human.** Rules turn `mail01/SMTP CRITICAL` into "E-mail is disrupted. We are working on it." Everything
  without a rule waits for a one-click approval.
- **Targeted.** Everyone, computers by name pattern (`PC-ACCOUNTING-*`), or – with Active Directory – groups, OUs
  and users. Notification groups bundle these under one name.
- **Traceable.** Every message records who published, approved and resolved it.
- **Unobtrusive.** A real desktop toolbar: maximised windows move out of the way instead of being covered.
  Multi-monitor, high DPI and terminal servers are supported.
- **Your brand.** Name, colour, logo, language and all texts on the bar are yours.
- **Silent alarm** (optional). A key combination calls for help quietly – nothing appears on the computer where it
  was pressed. See [Silent alarm](silent-alarm.md).
- **Sounds** (optional). A Windows sound or your own WAV file per kind of message.
- **For IT service providers.** Customer servers connect outbound to the hub. See [For IT service
  providers](service-provider-hub.md).

## How a message travels

1. Your monitoring reports a problem, or an admin writes a message.
2. If a [rule](monitoring.md#rules) matches, the message is published in the rule's words – after an optional delay
  against short blips. Without a rule it waits for approval (or, depending on the setting, is published as received
  or ignored).
3. The clients of the chosen recipients show the bar.
4. The recovery from monitoring, a click on *Resolve* or the scheduled end resolves the message: the bar turns green
  and disappears after a short time.

## Where to go next

| You want to … | Read |
|---|---|
| try Notiflag in 15 minutes | [Quick start](quickstart.md) |
| run the server properly | [Install and run the server](server-installation.md) |
| roll out the clients | [Windows client](windows-client.md) |
| write and understand messages | [Messages](messages.md) |
| connect your monitoring | [Connect your monitoring](monitoring.md) |
| review security and privacy | [Security and privacy](security-and-privacy.md) |

> **About the name:** Notiflag used to be called *Statusbar*. Technical names still carry the old name, for example
> `statusbar-client.exe`, `Statusbar.msi`, `%ProgramData%\Statusbar` and the repository
> [GerryCrooked/statusbar](https://github.com/GerryCrooked/statusbar). This documentation uses them as they are.
