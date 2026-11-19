# Statusbar for Zabbix

A Zabbix problem opens a message on your users' desktops; when the trigger recovers (or someone
closes the problem), the message gets its green all-clear and disappears.

| File | Purpose |
|---|---|
| [`media_statusbar.yaml`](media_statusbar.yaml) | Media type for import (Zabbix 7.0 and later), contains the script and all parameters |
| [`statusbar-webhook.js`](statusbar-webhook.js) | The same script on its own, for Zabbix 6.0 – 6.4 or to read |

Tested with Zabbix 7.0: trigger → action → media type → Statusbar, problem and recovery, imported media type.

## 1. Create the integration in Statusbar

*Monitoring → Add integration*, name `Zabbix`, policy *Hold for approval*, delay `60`.
Copy the webhook URL `https://status.example.com/api/hooks/sb_…` – it is shown only once.
Details: [Create the integration](../../docs/INTEGRATIONS.md#create-the-integration).

## 2. Set up Zabbix

### 2.1 Media type

**Zabbix 7.0 and later**

1. *Alerts → Media types → Import*, choose `media_statusbar.yaml`, *Import*.
2. The media type **Statusbar** appears, enabled. Nothing in it needs editing – the URL is entered per
   user in step 2.2, so the token is not part of the media type.

**Zabbix 6.0 – 6.4** (the 7.0 export cannot be imported there)

1. *Administration → Media types → Create media type*, Name `Statusbar`, Type **Webhook**.
2. Parameters (Name = Value):

   | Name | Value |
   |---|---|
   | `url` | `{ALERT.SENDTO}` |
   | `event_id` | `{EVENT.ID}` |
   | `value` | `{EVENT.VALUE}` |
   | `update` | `{EVENT.UPDATE.STATUS}` |
   | `host` | `{HOST.NAME}` |
   | `trigger` | `{EVENT.NAME}` |
   | `severity` | `{EVENT.SEVERITY}` |
   | `message` | `{ALERT.MESSAGE}` |
   | `proxy` | *(only if Zabbix needs an HTTP proxy)* `http://proxy.example.local:3128` |

3. *Script*: paste the content of `statusbar-webhook.js`. *Timeout* `10s`.
4. Tab *Message templates*: add *Problem* (message e.g. `{EVENT.NAME} on {HOST.NAME} since {EVENT.TIME}. {EVENT.OPDATA}`)
   and *Problem recovery* (any text, it is not shown).

### 2.2 A user that receives the alerts

Zabbix only sends an alert to a user who has **read permission on the host**. Use a dedicated user:

1. *Users → Users → Create user*, Username `statusbar`, a long random password, group e.g. *No access to the frontend*
   plus a user group (or role) that has **read** access to the host groups users should hear about.
2. Tab *Media → Add*:
   * Type **Statusbar**
   * Send to: **the webhook URL** from step 1
   * When active: `1-7,00:00-24:00`
   * Use if severity: tick the severities that matter to users (often *Average*, *High*, *Disaster*).
3. *Add*, *Add* (save the user).

### 2.3 Trigger action

*Alerts → Actions → Trigger actions → Create action* (6.x: *Configuration → Actions*):

1. Tab *Action*: Name `Statusbar`. Conditions – only what users should see, for example
   * *Tag name* equals `audience` **and** tag value equals `users` (set this tag on the relevant triggers or templates), or
   * *Host group* equals `Mail servers`, or
   * *Trigger severity* is greater than or equals *Average*.
2. Tab *Operations*:
   * *Operations → Add*: Send to users `statusbar`, Send only to **Statusbar**. Steps 1–1.
   * *Recovery operations → Add*: *Notify all involved* (or the user `statusbar`, Send only to *Statusbar*).
     **Without the recovery operation the message never goes away.**
   * *Update operations*: not needed. The script ignores them anyway (acknowledging a problem does not
     change anything on the users' screens).
3. *Add*.

## 3. How Zabbix maps to Statusbar

| Zabbix | Statusbar |
|---|---|
| Problem event (`{EVENT.VALUE}` = 1) | problem: message opens |
| Recovery event, problem closed manually (`{EVENT.VALUE}` = 0) | recovery: message resolved |
| Update operation (acknowledge, comment, severity change) | skipped by the script |
| `{EVENT.ID}` | key `zabbix-<id>`; the recovery carries the ID of the problem, so they match |
| `{HOST.NAME}` / `{EVENT.NAME}` | host / service; title without rule: `SMTP service is down (mail01)` |
| `{ALERT.MESSAGE}` (message template) | message text without rule |
| *Not classified*, *Information* | Notice (`info`) |
| *Warning*, *Average* | Degraded (`warning`) |
| *High*, *Disaster* | Outage (`error`) |

A trigger with *Problem event generation mode: Multiple* creates one message per problem event.

## 4. Test

**From the media type** – *Alerts → Media types*, *Test* in the row *Statusbar*. Fill in:

| Parameter | Problem test | Recovery test |
|---|---|---|
| `url` | your webhook URL | same |
| `event_id` | `test-1` | `test-1` |
| `value` | `1` | `0` |
| `update` | `0` | `0` |
| `host` | `zabbix-test` | `zabbix-test` |
| `trigger` | `Statusbar test` | `Statusbar test` |
| `severity` | `High` | `High` |
| `message` | `This is a test` | |

*Test* shows *Media type test successful* and the response `{"events":["draft"],"ok":true}` (or `created`).
The draft appears under *Messages → Waiting for approval*. Run the recovery test and the response is
`cancelled` – the draft is gone again.

If parameters are left as macros (`{EVENT.VALUE}`), the test fails with a message that says which
parameter is missing – that is expected, macros are only resolved for real events.

**End to end** – make a trigger fire that your action covers (e.g. stop a test service), wait for the
action, then let it recover. *Reports → Action log* (6.x: *Reports → Action log* / *Audit*) shows each
alert with status *Sent* or the error.

## 5. Example rule

Zabbix trigger names are often technical (`Linux: Zabbix agent is not available (for 3m)`). In Statusbar
(*Monitoring → Add rule*):

| Field | Value |
|---|---|
| When host/service matches | `mail0?/*` |
| Show this title | `E-mail is disrupted` |
| And this message | `Sending and receiving e-mail may be delayed. We are working on it.` |
| Severity | As reported |

Matches every alert of `mail01` … `mail09`. A second rule for one specific trigger on any host:
`*/*Zabbix agent is not available*` → title `{host} is not reachable`.

## Troubleshooting

| Symptom | Fix |
|---|---|
| Action log: *Media type disabled* | *Alerts → Media types*, enable *Statusbar*. |
| No alert at all in the action log | The action conditions do not match, the user has no media of type Statusbar, the media's severities exclude this one, or the user lacks **read permission** on the host. |
| `Statusbar: parameter "url" (Send to) must be the webhook URL` | The user media's *Send to* is empty or not a URL. |
| `Statusbar answered HTTP 401: Unknown integration token` | URL in *Send to* is incomplete or the integration was deleted. |
| `Couldn't resolve host name` / `Connection timed out` | The **Zabbix server** (or proxy that runs the webhook) cannot reach Statusbar. Test with `curl` on that machine; set the `proxy` parameter if you need one. |
| `SSL certificate problem` | Install your internal root CA on the Zabbix server (`update-ca-certificates` / `update-ca-trust`) and restart it. |
| Message stays after the trigger recovered | The action has no *Recovery operation*, or it notifies a user without Statusbar media. |
| Timeout | Raise the media type's *Timeout* (default 30 s in Zabbix, 10 s in the import). |

Debug output of the script (`[Statusbar] sending …`) goes to the Zabbix server log at DebugLevel 4.
