---
id: monitoring
title: Connect your monitoring
description: Connect monitoring, RMM, ticket systems and Microsoft 365 to Notiflag – integrations, rules, delay, keys and troubleshooting.
lang: en
slug: monitoring
order: 11
---

# Connect your monitoring

A problem in your monitoring opens a message, the recovery resolves it. In between, rules decide in which words and
for whom – and everything without a rule waits for a one-click approval.

## Supported systems

The step-by-step guides per system are in the repository.

| System | Guide | How | Tested with |
|---|---|---|---|
| Zabbix 6.0+ | [zabbix](https://github.com/GerryCrooked/statusbar/blob/main/integrations/zabbix/README.md) | Media type (YAML import) | Zabbix 7.0 |
| Checkmk 2.0+ | [checkmk](https://github.com/GerryCrooked/statusbar/blob/main/integrations/checkmk/README.md) | Notification script | Checkmk 2.4 |
| PRTG | [prtg](https://github.com/GerryCrooked/statusbar/blob/main/integrations/prtg/README.md) | PowerShell script or HTTP action | PowerShell 7, without PRTG |
| Nagios, Naemon, Icinga | [nagios-icinga](https://github.com/GerryCrooked/statusbar/blob/main/integrations/nagios-icinga/README.md) | Shell script + commands | Nagios 4.5, Icinga 2.14 |
| Prometheus Alertmanager | [alertmanager](https://github.com/GerryCrooked/statusbar/blob/main/integrations/alertmanager/README.md) | `webhook_configs` | Alertmanager 0.28 |
| Grafana | [grafana](https://github.com/GerryCrooked/statusbar/blob/main/integrations/grafana/README.md) | Contact point *Webhook* | Grafana 13 |
| Uptime Kuma | [uptime-kuma](https://github.com/GerryCrooked/statusbar/blob/main/integrations/uptime-kuma/README.md) | Notification *Webhook* | Uptime Kuma 1.23 |
| LibreNMS | [librenms](https://github.com/GerryCrooked/statusbar/blob/main/integrations/librenms/README.md) | Alert transport *API* | LibreNMS 26.11 |
| Microsoft 365 service health *(in development)* | [microsoft-365](https://github.com/GerryCrooked/statusbar/blob/main/integrations/microsoft-365/README.md) | App registration + 3 lines in `.env`; the server polls | stand-in for Graph |
| Azure Monitor | [azure-monitor](https://github.com/GerryCrooked/statusbar/blob/main/integrations/azure-monitor/README.md) | Action group *Webhook* | vendor docs only |
| Power Automate (alert e-mails …) | [power-automate](https://github.com/GerryCrooked/statusbar/blob/main/integrations/power-automate/README.md) | Flow with HTTP action | vendor docs only |
| NinjaOne *(in development)* | [ninjaone](https://github.com/GerryCrooked/statusbar/blob/main/integrations/ninjaone/README.md) | Notification channel *Webhook* | vendor docs only |
| Atera *(in development)* | [atera](https://github.com/GerryCrooked/statusbar/blob/main/integrations/atera/README.md) | Poller script at the service provider | stand-in for the API |
| Datto RMM | [datto-rmm](https://github.com/GerryCrooked/statusbar/blob/main/integrations/datto-rmm/README.md) | Monitor webhook | vendor docs only |
| N-able N-central | [n-able](https://github.com/GerryCrooked/statusbar/blob/main/integrations/n-able/README.md) | HTTP-XML notification | vendor docs only |
| Tactical RMM | [tactical-rmm](https://github.com/GerryCrooked/statusbar/blob/main/integrations/tactical-rmm/README.md) | Webhook in alert template | vendor docs only |
| ConnectWise, Kaseya VSA, N-sight … | [other-rmm](https://github.com/GerryCrooked/statusbar/blob/main/integrations/other-rmm/README.md) | Alert e-mails or script | – |
| Zammad | [zammad](https://github.com/GerryCrooked/statusbar/blob/main/integrations/zammad/README.md) | Webhooks + triggers | Zammad 7.2 |
| GLPI | [glpi](https://github.com/GerryCrooked/statusbar/blob/main/integrations/glpi/README.md) | Webhooks with filter | GLPI 12 |
| Znuny / OTRS | [znuny-otrs](https://github.com/GerryCrooked/statusbar/blob/main/integrations/znuny-otrs/README.md) | Generic Agent module | Znuny 6.5 |
| Jira Service Management | [jira-service-management](https://github.com/GerryCrooked/statusbar/blob/main/integrations/jira-service-management/README.md) | Automation rules | vendor docs only |
| ServiceNow | [servicenow](https://github.com/GerryCrooked/statusbar/blob/main/integrations/servicenow/README.md) | Business rule | mocked objects |
| TOPdesk | [topdesk](https://github.com/GerryCrooked/statusbar/blob/main/integrations/topdesk/README.md) | Action Management | vendor docs only |
| curl, scripts, backup jobs | [curl](https://github.com/GerryCrooked/statusbar/blob/main/integrations/curl/README.md) | `curl` or `statusbar-notify.sh` | bash, dash, busybox |

"Tested with" means: a problem and its recovery were sent from the real system to a test server, and the message
appeared and was resolved. "Vendor docs only" means the payloads follow the vendor's documentation and were tested
against Notiflag, but not sent by the product itself. Systems without a guide use the [generic
format](#generic-format).

## How it works

```text
 monitoring ──POST──► /api/hooks/<token> ──► event (key, problem/recovery, host, service, title, text, severity)
                                                   │
        problem ──► already open for this key? ── yes ──► "duplicate" (nothing changes)
           │ no
           ├─ a rule matches ─────────────► message in the rule's words        "created"
           └─ no rule: integration policy ─► Hold for approval                 "draft"
                                             Publish as received               "created"
                                             Ignore                            "ignored"
        recovery ─► open message for this key? ─► published: green all-clear  "resolved"
                                                   draft or still in the delay: deleted silently  "cancelled"
                                                   none: nothing                "ignored"
```

- **One integration per monitoring system** (or per instance). Keys are only compared within one integration.
- **Rules apply to all integrations**, in the order they were created; the first match wins.
- **Delay**: a published message only appears after this time. If the recovery comes first, it is deleted and nobody
  saw it.
- **Recipients**: a rule's recipients win; without a rule the integration's *Default recipients* apply. Empty =
  everyone.

## Create an integration

1. **Monitoring → Add integration**:
   - **Name**, e.g. `Zabbix`. It appears as the origin in the history.
   - **Alerts without a matching rule**: start with **Hold for approval**. Such alerts wait under *Messages → Waiting
     for approval* until someone clicks *Publish* or *Discard*.
   - **Wait before showing (seconds)**: `60` is a good value against short blips, `0` while testing.
   - **Default recipients**: empty for everyone.
2. **Create**. Notiflag shows the **webhook URL** and a ready `curl` test:
   `https://status.example.com/api/hooks/sb_0123456789abcdef…`
3. **Copy it now** – only a hash of the token is stored. Lost it? Delete the integration and create a new one.

Treat the URL like a password: whoever has it can put messages on every screen. The integration list shows *last
event* and a counter – the quickest way to see whether anything arrives at all.

## Problem and recovery

Notiflag only acts on problems and recoveries. Acknowledgements, downtimes, flapping, comments and test
notifications without a state are ignored.

| Value (case-insensitive) | Meaning |
|---|---|
| `problem` `down` `critical` `crit` `warning` `warn` `unknown` `unreachable` `firing` `error` `fail` `failed` `failure` `alerting` `1` | **problem**: open a message |
| `ok` `up` `resolved` `recovery` `recovered` `resolve` `normal` `0` | **recovery**: resolve the message |
| anything else (`ACKNOWLEDGEMENT`, `pending`, …) | ignored |

**Severity** – decides the colour:

| Notiflag | Words that map to it |
|---|---|
| Outage | `error` `critical` `crit` `highest` `high` `disaster` `major` `page` `down` `unreachable` `unknown` |
| Degraded | `warning` `warn` `average` `minor` `moderate` `medium` `degraded` |
| Notice | `info` `information` `informational` `not classified` `none` `notice` `low` `lowest` |

Without one, a `warning` state becomes Degraded and every other problem Outage. A rule can override it.

## The key

Every event has a **key**. The recovery must carry the same key as the problem, otherwise the message stays. The
guides set it appropriately (e.g. `zabbix-<event id>`, `host/service` for Checkmk and Nagios, the fingerprint for
Alertmanager and Grafana).

While a message for a key is open (published, in its delay or waiting for approval), further problems with that key are
answered with `duplicate` – repeats, escalations and a change from *Warning* to *Critical* do not create a second
message. After it is resolved or discarded, the key is free again.

## Rules

Users should read "E-mail is disrupted. We are working on it." rather than `SMTP CRITICAL - connect to port 25 failed`.
That is what rules are for. **Monitoring → Add rule**:

| Field | Meaning |
|---|---|
| **When host/service matches** | Pattern with `*` (any text) and `?` (one character), case-insensitive. Compared with `host/service`, the key and the title; any of the three may match. The pattern must match the **whole** text: `mail01` does not match `mail01/SMTP`, `mail01/*` does. `*` also spans `/` |
| **Show this title** / **And this message** | What users read. Placeholders: `{host}` `{service}` `{title}` `{message}` |
| **Severity** | *As reported* keeps the alert's severity |
| **Recipients** | Who sees it. Empty = everyone |

A message created by a rule is published **without approval** (after the delay). The usual set-up: rules for the
alerts you have thought about, *Hold for approval* for everything else.

Examples:

| Pattern | Title | Message | Matches |
|---|---|---|---|
| `mail*/*` | E-mail is disrupted | Sending and receiving e-mail may not work. We are working on it. | every alert of hosts `mail…` |
| `*/SMTP*` | E-mail is disrupted | Outgoing e-mail is delayed. | the SMTP service on any host |
| `erp-db?/*` | The ERP system is unavailable | Please save your work. We will tell you when it is back. | `erp-db1/…` to `erp-db9/…` |
| `*vpn*` | VPN connections are failing | Working from home is currently not possible. | anything with "vpn" in host, service, key or title |

## Ticket systems: known outages for everyone

Monitoring reports what it detects. A ticket system covers the rest: when IT knows about an outage – reported by
users, by a supplier, found by an engineer – the ticket for it puts the message on every desk. Users see "IT is on it"
and do not open twenty more tickets. When the ticket is solved, the all-clear follows.

- **Only marked tickets**: a tag, a category, a queue, the highest priority or the system's major-incident flag.
- **Ticket number = key.** Opening or marking sends a problem, solving/closing the recovery.
- **Hold for approval** is recommended: ticket titles are written for IT, not for users.

Guides for Zammad, GLPI, Znuny/OTRS, Jira Service Management, ServiceNow and TOPdesk are in the table above. Other
systems (helpLine, Freshservice, HaloPSA, Autotask, …): an outgoing webhook, an HTTP step in automations or a script
action sends the generic format with `"key": "<system>-<ticket number>"`.

### The other way round: tickets from messages

Statusbar can open tickets itself: *Settings → Ticket system*. When a message is published or a silent alarm is
raised, it opens a ticket in Zammad, GLPI, Znuny/OTRS, Jira Service Management, ServiceNow, TOPdesk or through a JSON
webhook you define; when the message is resolved, the ticket gets a note and is closed. Title, text and priority come
from templates with placeholders. Delivery goes through a queue with retries, *Create test ticket* checks the
connection, and the message list links the ticket. Such a ticket never comes back as a second message through the
ticket integration (answer `own-ticket`).
Set-up per system: [Statusbar → ticket system](https://github.com/GerryCrooked/statusbar/blob/main/docs/INTEGRATIONS.md#statusbar--ticket-system-a-ticket-for-every-message).

## Generic format

For systems without a guide: `POST` to the webhook URL, a JSON object (or an array of up to 200). The `Content-Type`
is not checked; flat XML is accepted as well (for N-able N-central).

```json
{
  "status":   "problem",
  "host":     "mail01",
  "service":  "SMTP",
  "message":  "Port 25 does not answer",
  "severity": "error",
  "key":      "optional-stable-id",
  "title":    "optional, default: \"SMTP (mail01)\""
}
```

| Field | Also accepted | Notes |
|---|---|---|
| `status` | `state`, `type` | Required, see [Problem and recovery](#problem-and-recovery) |
| `host` | `hostname`, `instance` | One of host, service, key, title is required |
| `service` | `check`, `trigger` | |
| `key` | `id`, `event_id` | Recommended; default `host/service` |
| `title` | `summary`, `subject`, `name` | Default `service (host)`; at most 120 characters |
| `message` | `description`, `output`, `text`, `msg` | At most 1500 characters |
| `severity` | `priority` | see severity |

Answer: `{"ok": true, "events": ["created"]}` – per recognised event one of `created`, `draft`, `duplicate`,
`resolved`, `cancelled`, `ignored`, `own-ticket` (a ticket Statusbar opened itself). An empty list means: neither a problem nor a recovery was recognised.

**Authentication** – three equivalent ways:

| Way | Call | When |
|---|---|---|
| Token in the path | `POST /api/hooks/<token>` | everything that only takes a URL |
| Bearer header | `POST /api/hooks` with `Authorization: Bearer <token>` | keeps the token out of URLs and logs |
| Query parameter | `POST /api/hooks?token=<token>` | tools that can neither change the path nor set headers |

## Troubleshooting

Quickest: one manual call from the monitoring server itself – it tests network, TLS, token and policy in one go:

```bash
curl -sS -X POST 'https://status.example.com/api/hooks/<token>' -H 'Content-Type: application/json' \
  -d '{"status":"problem","host":"notiflag-test","service":"manual"}'
curl -sS -X POST 'https://status.example.com/api/hooks/<token>' -H 'Content-Type: application/json' \
  -d '{"status":"ok","host":"notiflag-test","service":"manual"}'
```

| Symptom | Cause and fix |
|---|---|
| `401 Unknown integration token` | Token copied incompletely or integration deleted |
| `400 JSON body required` | The tool sends form data, or inserts text with quotes unescaped into a JSON template |
| `SSL certificate problem` | The monitoring server does not trust your certificate – install the root CA there, do not switch verification off |
| `{"ok":true,"events":[]}` | Neither problem nor recovery – a test button, an acknowledgement, `pending` or an unknown state word |
| `draft`, but nothing on screen | As designed: *Hold for approval*. Approve it or add a rule |
| `created`, but nothing on screen yet | The delay is running |
| Message does not go away | The recovery never arrived (switched off in the monitoring) or arrived with a different key |
| `ignored` for a recovery | No open message for that key: already resolved, discarded, or sent to another integration |
| A discarded alert comes back | The monitoring repeats notifications; after *Discard* the key is free. Stop it at the source |
| Rule does not match | Patterns must match the whole text: `mail01/*` instead of `mail01`, `*/SMTP` instead of `SMTP` |

The server log has one line per event with integration, key, problem/recovery and outcome:

```bash
docker compose logs --since 1h statusbar | grep "monitoring event"
```

Full reference: [docs/INTEGRATIONS.md](https://github.com/GerryCrooked/statusbar/blob/main/docs/INTEGRATIONS.md).
