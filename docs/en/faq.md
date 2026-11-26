---
id: faq
title: FAQ
description: Answers to common questions about Notiflag – operating systems, cloud, cost, privacy and limits.
lang: en
slug: faq
order: 17
---

# FAQ

**Does the client run on macOS or Linux?**
No, currently only on Windows 10 and 11 and on Windows terminal servers.

**Is Notiflag available as a cloud service?**
No. The server runs at your site (or at your IT service provider). There is no cloud and no telemetry.

**Does the server have to be reachable from the internet?**
No, if clients and monitoring are in the same network. Only cloud services (e.g. Azure Monitor, Jira Cloud, NinjaOne)
need a public HTTPS address – `/api/hooks/` is enough for them.

**What does it cost?**
Nothing up to 10 PCs, with all features. Beyond that there are licenses per PC; prices at
[notiflag.de](https://notiflag.de/en/). Customer servers on a hub need no license of their own. See [License](license.md).

**What happens when the license expires?**
Nothing is blocked. The web UI shows a notice, messages keep working.

**How fast does a message appear?**
In about a second. The clients keep an open connection to the server.

**What do users see when the server is down?**
The last known state. The client does not invent an all-clear and reconnects as soon as the server is back.

**Can users dismiss the bar?**
No. It disappears when the message is resolved.

**Does the bar cover my windows?**
No – it is a real desktop toolbar, maximised windows move out of the way. Over full-screen windows it overlays the edge
by default; this can be switched off. See [Windows client](windows-client.md#full-screen-windows-and-remote-desktop).

**Do I need Active Directory?**
No. Without AD you sign in with the `ADMIN_TOKEN` and send messages to everyone or to computers by name. With AD,
groups, OUs and users are added.

**Does it work with Entra ID (Azure AD) without on-premises AD?**
Sign-in and recipients by group currently need an LDAP-capable directory. Without one, messages to everyone, to
computers and to notification groups made of computers work.

**Can I schedule messages?**
Yes: *Show from* and *Resolve automatically at* on every message, e.g. for maintenance windows.

**Which languages?**
The web UI is available in English and German; more languages can be added as a JSON file. The texts on the bar are
written by you.

**Is the silent alarm an emergency call system?**
No. It is an additional, quiet channel without acknowledgement. See [Silent alarm](silent-alarm.md).

**Is the source code open?**
Yes, server and client are AGPL-3.0 on [GitHub](https://github.com/GerryCrooked/statusbar). The hub for service
providers is commercial.

**Why are files still called "Statusbar"?**
Notiflag used to be called Statusbar. Program, installer and folder names still carry the old name.
