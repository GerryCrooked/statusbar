---
id: messages
title: Messages
description: Write, schedule, approve and resolve messages – and what the severities mean.
lang: en
slug: messages
order: 6
---

# Messages

A message is what the bar shows: a title, optionally a text, a severity and the recipients. It comes from an admin,
from monitoring, from the service provider's hub or as a silent alarm.

## The Messages page

| Section | Content |
|---|---|
| **Preview** at the top | What the bar looks like on the desks right now – or the message you are writing |
| **New message** | Form for your own message |
| **Waiting for approval** | Messages from monitoring or your service provider that nobody sees yet |
| **Active** | What is shown now or will appear at a scheduled time |
| **Recently resolved** | The last 15 resolved or discarded messages |

Every row shows who created, approved and resolved the message and whom it is for.

## Write a message

**Messages → New message**:

| Field | Notes |
|---|---|
| **Title** | At most 120 characters. Large on the bar – short and clear to users: "E-mail is disrupted" |
| **Message** | Optional, at most 1500 characters. What users should know or do: "We are working on it. No need to open tickets." |
| **Severity** | Outage, Degraded or Notice (see below) |
| **Recipients** | Empty = everyone. Otherwise computer patterns, notification groups and – with Active Directory – groups, OUs, users. See [Recipients](recipients.md) |
| **Schedule** (optional) | *Show from* and *Resolve automatically at* |

**Publish** – the message is on the screens in about a second.

Do not put confidential information in messages: the recipients decide which computers show a message – anyone with
the client token could read it (see [Security and privacy](security-and-privacy.md)).

## Severities

| Severity | Colour | For |
|---|---|---|
| **Outage** | Red | Something does not work: e-mail, ERP, internet |
| **Degraded** | Orange | Works, but slowly or partly |
| **Notice** | Blue | Planned maintenance, information |
| **All-clear** | Green | Shown automatically when a message is resolved |
| **Silent alarm** | Violet | Only for [silent alarms](silent-alarm.md) |

The labels in front of the title ("Outage", "Notice" …) can be changed under *Settings → Bar on the desktops →
Labels in front of the title*.

## Schedule

- **Show from**: the message appears only at this time. Until then it is listed under *Active* with "Show from …".
- **Resolve automatically at**: at this time it is resolved, as if someone had clicked *Resolve*.

Example maintenance window: title "ERP unavailable today 8–9 pm", Notice, *Show from* 8:00 am, *Resolve
automatically at* 9:00 pm.

## Approve and discard

Messages from monitoring without a matching rule – depending on the integration's setting – and messages from the
service provider – depending on *Messages from your service provider* – land under **Waiting for approval**. Nobody
sees them there yet.

- **Publish**: the message appears right away.
- **Discard**: a message from monitoring is deleted; if monitoring reports the same problem again later, a new one is
  created. A message from the service provider stays as *discarded* in the history and does not come back, even while
  the provider keeps it active.

If the recovery arrives before anyone approved, the message disappears by itself.

## Resolve

**Resolve** on an active message: the bar turns green with the message's title and disappears after the configured
time (*Show all-clear for*, default 90 seconds). Monitoring resolves its own messages as soon as the problem is
fixed; you can still resolve them by hand at any time.

## Who did what?

Every message records who created it and – where applicable – who approved and resolved it:

| Shown as | Origin |
|---|---|
| user name or `admin` | An admin in the web UI |
| *monitoring* + name of the integration | A monitoring system |
| *API* | A script with the `ADMIN_TOKEN` |
| *service provider* | The service provider's hub |
| *silent alarm* | A user with the key combination |
| *schedule* | Automatically at the scheduled end |

Resolved messages are kept for `RETENTION_DAYS` days (default 90).

## Messages from a script

```bash
curl -X POST https://status.example.com/api/admin/incidents \
  -H "Authorization: Bearer $ADMIN_TOKEN" -H 'Content-Type: application/json' \
  -d '{"title":"Maintenance tonight","message":"ERP offline 8-9 pm","severity":"info","targets":["host:PC-ACC-*"]}'
```

Monitoring systems have their own webhooks with all-clear – see [Connect your monitoring](monitoring.md). All fields:
[API](api.md).
