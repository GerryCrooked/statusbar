# Monitoring integrations

Statusbar turns alerts from your monitoring into messages on your users' screens, and takes them
down again with a green all-clear when the system has recovered. This page explains how Statusbar
reads alerts. Each monitoring system has its own step-by-step guide:

| Monitoring system | Guide | What you install | Tested with |
|---|---|---|---|
| **Zabbix** 6.0+ | [integrations/zabbix](../integrations/zabbix/README.md) | Media type (import YAML) | Zabbix 7.0, real trigger → action → webhook |
| **Checkmk** 2.0+ | [integrations/checkmk](../integrations/checkmk/README.md) | Notification script | Checkmk 2.4 Raw site |
| **PRTG** | [integrations/prtg](../integrations/prtg/README.md) | PowerShell script or HTTP action | PowerShell 7, simulated HTTP action |
| **Nagios / Naemon / Icinga** | [integrations/nagios-icinga](../integrations/nagios-icinga/README.md) | Shell script + commands | Nagios Core 4.5, Icinga 2.14 |
| **Prometheus Alertmanager** | [integrations/alertmanager](../integrations/alertmanager/README.md) | `webhook_configs` | Alertmanager 0.28 |
| **Grafana** alerting | [integrations/grafana](../integrations/grafana/README.md) | Contact point *Webhook* | Grafana 13, UI and file provisioning |
| **Uptime Kuma** | [integrations/uptime-kuma](../integrations/uptime-kuma/README.md) | Notification *Webhook* | Uptime Kuma 1.23 |
| **curl**, scripts, backup jobs, anything else | [integrations/curl](../integrations/curl/README.md) | `curl` or `statusbar-notify.sh` | bash, dash, busybox |

"Tested with" means: a problem and its recovery were sent from that system to a throwaway Statusbar
server, and the message appeared and was resolved. [`scripts/test-integrations.sh`](../scripts/test-integrations.sh)
replays all sample payloads from `integrations/` against your own server.

## Contents

* [How it works](#how-it-works)
* [Create the integration](#create-the-integration)
* [Problem and recovery](#problem-and-recovery)
* [The key: how a recovery finds its problem](#the-key-how-a-recovery-finds-its-problem)
* [Rules: from technical alert to plain words](#rules-from-technical-alert-to-plain-words)
* [Generic payload reference](#generic-payload-reference)
* [Response and outcomes](#response-and-outcomes)
* [Authentication](#authentication)
* [Troubleshooting](#troubleshooting)

## How it works

```
 monitoring ──POST──► /api/hooks/<token> ──► parse ──► event (key, problem/recovery, host, service, title, text, severity)
                                                          │
             problem ──► already open for this key? ── yes ──► "duplicate" (nothing changes)
                │ no
                ├─ a rule matches ─────────────► message published (rule's words, rule's recipients)  "created"
                └─ no rule: integration policy ─► Hold for approval → "Waiting for approval"          "draft"
                                                  Publish as received → published as sent              "created"
                                                  Ignore → nothing                                    "ignored"
             recovery ─► open message for this key? ─► published: green all-clear, then gone       "resolved"
                                                        draft or still in the delay: deleted silently "cancelled"
                                                        none: nothing                                 "ignored"
```

* **One integration per monitoring system** (or per instance). Keys are only compared within one
  integration, so two Zabbix servers with the same event IDs do not collide if each has its own integration.
* **Rules apply to all integrations.** They are checked in the order they were created; the first match wins.
* **Delay** ("Wait before showing", default 60 s in the form): a published message only reaches screens after
  this time. If the recovery arrives first, the message is deleted and nobody ever saw it.
  Set it to `0` while you test.
* **Recipients**: a rule's recipients win; without a rule the integration's *Default recipients* are used.
  Empty means everyone.

## Create the integration

1. Sign in to the Statusbar web UI as an administrator and open **Monitoring**.
2. Under **Add integration** enter:
   * **Name** – e.g. `Zabbix`. It appears in the history as `monitoring:Zabbix`.
   * **Alerts without a matching rule** – start with **Hold for approval**. Alerts then wait under
     *Messages → Waiting for approval* until someone clicks *Publish* or *Discard*.
   * **Wait before showing (seconds)** – `60` is a good default against short blips; `0` while testing.
   * **Default recipients** – leave empty for everyone.
3. Click **Create**. A green box shows the **webhook URL** and a ready-made `curl` test:
   ```
   https://status.example.com/api/hooks/sb_0123456789abcdef…
   ```
   **Copy it now.** Only a hash of the token is stored; if you lose it, delete the integration and create a new one.
4. Treat the URL like a password: whoever has it can put messages on every screen. Keep it in your
   monitoring system's secret store or in a file readable only by the monitoring service (the guides show how).

The integration list shows *last event* and a counter, which is the quickest way to see whether
anything arrives at all.

## Problem and recovery

Statusbar only acts on two kinds of events. Everything else – acknowledgements, downtimes, flapping,
comments, test notifications without a state – is ignored, so it never disturbs your users.

| Field value (case-insensitive) | Meaning |
|---|---|
| `problem` `down` `critical` `crit` `warning` `warn` `unknown` `unreachable` `firing` `error` `fail` `failed` `failure` `alerting` `1` | **problem**: open a message |
| `ok` `up` `resolved` `recovery` `recovered` `resolve` `normal` `0` | **recovery**: resolve the message |
| anything else (`ACKNOWLEDGEMENT`, `pending`, `Paused`, `Down (Acknowledged)` …) | ignored |

The state is read from `status`, `state` or `type` (generic payload), from `alerts[].status`
(Alertmanager, Grafana) or from `heartbeat.status` (Uptime Kuma: `0` = down, `1` = up, `2` pending and
`3` maintenance are ignored).

**Severity** decides the colour of the bar:

| Statusbar | In the UI | Words that map to it |
|---|---|---|
| `error` | Outage | `error` `critical` `crit` `high` `disaster` `major` `page` `down` `unreachable` `unknown` |
| `warning` | Degraded | `warning` `warn` `average` `minor` `degraded` |
| `info` | Notice | `info` `information` `informational` `not classified` `none` `notice` |

Severity comes from `severity` or `priority` (generic) or `labels.severity` (Alertmanager, Grafana).
Without one, a `warning`/`warn` state becomes *Degraded* and every other problem *Outage*.
A rule can override it.

## The key: how a recovery finds its problem

Every event has a **key**. The recovery must carry the same key as the problem, otherwise the message stays.

| Source | Key |
|---|---|
| Generic JSON | `key` (or `id`, `event_id`); without one `host/service`, without both the title |
| Zabbix (bundled script) | `zabbix-<EVENT.ID>` – the problem event ID is also present in the recovery |
| Checkmk, Nagios, Icinga (bundled scripts) | `host/service`; host problems `host/` |
| PRTG (bundled script / payload) | `prtg-<sensor id>` |
| Alertmanager, Grafana | the alert's `fingerprint` (stable for one label set) |
| Uptime Kuma | `kuma-<monitor id>` |

While a message for a key is open (published, delayed or waiting for approval), further problems with
that key are answered with `duplicate` – repeated notifications, escalations and state changes from
*Warning* to *Critical* do not create a second message. A message that was resolved, or a draft that was
discarded, no longer blocks the key: the next problem opens a new one.

## Rules: from technical alert to plain words

Users should read "E-mail is disrupted. We are working on it." rather than `SMTP CRITICAL - connect to
port 25 failed`. Rules do that. **Monitoring → Add rule**:

| Field | Meaning |
|---|---|
| **When host/service matches** | Pattern with `*` (any text) and `?` (one character), case-insensitive. It is compared with `host/service`, with the key and with the title; any of the three may match. The pattern must match the **whole** text: `mail01` does not match `mail01/SMTP`, `mail01/*` does. `*` also spans the `/`. |
| **Show this title** / **And this message** | What users read. Placeholders: `{host}` `{service}` `{title}` `{message}` (the alert's own values). |
| **Severity** | *As reported* keeps the alert's severity. |
| **Recipients** | Who sees it. Empty = everyone. |

A message created by a rule is **published without approval** (after the delay). So the usual set-up is:
rules for the alerts you have thought about, *Hold for approval* for everything else.

What `host/service` looks like per system – this is what you write patterns against:

| Source | `host` | `service` | Example subject |
|---|---|---|---|
| Zabbix | `{HOST.NAME}` | trigger name `{EVENT.NAME}` | `mail01/SMTP service is down` |
| Checkmk, Nagios, Icinga | host name | service description (empty for host problems) | `mail01/SMTP`, `mail01/` |
| PRTG | device | sensor name | `mail01.example.local/SMTP` |
| Alertmanager, Grafana | label `instance` (or `host`, `hostname`, `node`) | label `service`, else `job`, else `alertname` | `mail01:25/smtp` |
| Uptime Kuma | monitor name | monitor type | `Intranet/http` |
| Generic | `host` / `hostname` / `instance` | `service` / `check` / `trigger` | whatever you send |

Examples:

| Pattern | Title | Message | Matches |
|---|---|---|---|
| `mail*/*` | E-mail is disrupted | Sending and receiving e-mail may not work. We are working on it. | every alert of hosts `mail…` |
| `*/SMTP*` | E-mail is disrupted | Outgoing e-mail is delayed. | the SMTP service on any host |
| `erp-db?/*` | The ERP system is unavailable | Please save your work. We will tell you when it is back. | `erp-db1/…` to `erp-db9/…` |
| `*vpn*` | VPN connections are failing | Working from home is currently not possible. | anything with "vpn" in host, service, key or title |
| `kuma-12` | The intranet is not reachable | {message} | exactly one Uptime Kuma monitor (by key) |

## Generic payload reference

`POST` a JSON object, or an array of up to 200 objects, to the webhook URL. The `Content-Type` header is
not checked, but the body must be valid JSON.

```json
{
  "status":   "problem",
  "host":     "mail01",
  "service":  "SMTP",
  "message":  "Connection to port 25 timed out",
  "severity": "error",
  "key":      "optional-stable-id",
  "title":    "optional, default: \"SMTP (mail01)\""
}
```

| Field | Also accepted | Required | Notes |
|---|---|---|---|
| `status` | `state`, `type` | yes | see [Problem and recovery](#problem-and-recovery) |
| `host` | `hostname`, `instance` | one of host, service, key, title | |
| `service` | `check`, `trigger` | | |
| `key` | `id`, `event_id` | recommended | default `host/service` |
| `title` | `summary`, `subject`, `name` | | default `service (host)`, `service` or `host`; max. 120 characters |
| `message` | `description`, `output`, `text`, `msg` | | max. 1500 characters; empty = title |
| `severity` | `priority` | | see severity table |

Numbers are accepted where text is expected (`"event_id": 4711`). Leading and trailing spaces are removed.

## Response and outcomes

A successful call answers `HTTP 200` with one outcome per recognised event:

```json
{"ok": true, "events": ["created"]}
```

| Outcome | Meaning |
|---|---|
| `created` | message published (rule or *Publish as received*); visible after the delay |
| `draft` | waiting for approval under *Messages → Waiting for approval* |
| `duplicate` | a message for this key is already open; nothing changed |
| `resolved` | the open message got its all-clear |
| `cancelled` | the recovery came while the message was a draft or still in its delay; deleted, nobody saw it |
| `ignored` | recovery without open message, or problem without rule while the policy is *Ignore* |
| `[]` (empty list) | the payload contained no problem or recovery (e.g. a test notification, an acknowledgement) |

| HTTP status | Body | Cause |
|---|---|---|
| `401` | `Unknown integration token` | token wrong, shortened, or the integration was deleted |
| `400` | `JSON body required` | body is not valid JSON (form data, unescaped quotes, empty body); checked before the token |
| `404` / `405` | | wrong path, or `GET` instead of `POST` |

## Authentication

Three equivalent ways, use whatever your tool supports best:

| Way | URL | When |
|---|---|---|
| Token in the path | `POST https://status.example.com/api/hooks/<token>` | everything that only takes a URL |
| Bearer header | `POST https://status.example.com/api/hooks` with `Authorization: Bearer <token>` (exactly this spelling) | Alertmanager, Grafana, Uptime Kuma: keeps the token out of URLs and access logs |
| Query parameter | `POST https://status.example.com/api/hooks?token=<token>` | tools that can neither change the path nor set headers |

The pre-0.5 endpoint `/api/webhooks/incidents` (`WEBHOOK_TOKEN`) creates messages but can never resolve
them. Use integrations instead.

## Troubleshooting

Start with one manual call from the monitoring server itself – it tests network, TLS, token and the
policy in one go:

```bash
curl -sS -X POST 'https://status.example.com/api/hooks/<token>' -H 'Content-Type: application/json' \
  -d '{"status":"problem","host":"statusbar-test","service":"manual","message":"test"}'
curl -sS -X POST 'https://status.example.com/api/hooks/<token>' -H 'Content-Type: application/json' \
  -d '{"status":"ok","host":"statusbar-test","service":"manual"}'
```

| Symptom | Cause and fix |
|---|---|
| `401 Unknown integration token` | Token incomplete (copied without the `sb_` prefix or cut off) or the integration was deleted. Create a new integration. |
| `400 JSON body required` | The tool sends form data (Uptime Kuma: choose *application/json*) or inserts text with quotes into a hand-written JSON template (PRTG HTTP action: use the PowerShell script). |
| `curl: (60) SSL certificate problem` / `certificate verify failed` | The monitoring server does not trust your Statusbar certificate. Install your internal root CA on that server; do not switch verification off. |
| Connection refused / timeout | Firewall or proxy between monitoring and Statusbar. Statusbar listens on `BIND_ADDRESS:HTTP_PORT` (default `127.0.0.1:8080`) behind your reverse proxy. |
| `{"ok":true,"events":[]}` | Nothing in the payload was a problem or a recovery – e.g. a tool's *Test* button, an acknowledgement, a `pending` state, or a status word Statusbar does not know. Compare with the table in [Problem and recovery](#problem-and-recovery). |
| `draft`, but nothing on screen | Working as designed: *Hold for approval*. Publish it under *Messages → Waiting for approval*, or add a rule. |
| `created`, but nothing on screen yet | The delay ("Wait before showing") is running. The default in the form is 60 seconds. |
| Message does not go away | The recovery never arrived (recovery notifications switched off in the monitoring tool), or it arrived with a different key – compare the `key=` of both events in the server log (`docker compose logs statusbar | grep "monitoring event"`). |
| `ignored` for a recovery | No open message with that key: it was already resolved, discarded, or the problem was sent to a different integration. |
| A discarded alert comes back | The monitoring tool repeats notifications (re-notification, escalation, Alertmanager `repeat_interval`). After *Discard* the key is free again. Stop the alert at the source or route it away from Statusbar. |
| Rule does not match | Patterns must match the whole text: `mail01/*` instead of `mail01`, `*/SMTP` instead of `SMTP`. Check the `host/service` subject table above for what your tool sends. |

The server log has one line per event, with integration, key, problem/recovery and outcome:

```bash
docker compose logs --since 1h statusbar | grep "monitoring event"
```
