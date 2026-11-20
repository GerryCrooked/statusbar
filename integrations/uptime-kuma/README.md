# Statusbar for Uptime Kuma

A monitor going **Down** opens a message on your users' desktops; when it is **Up** again, the message
gets its green all-clear and disappears. Statusbar reads Uptime Kuma's webhook format as it is.

| File | Purpose |
|---|---|
| [`test-down.json`](test-down.json), [`test-up.json`](test-up.json) | Payloads as Uptime Kuma sends them, for testing with `curl` |
| [`test-notification.json`](test-notification.json) | What the *Test* button sends (contains no state, so nothing happens) |

Tested with Uptime Kuma 1.23: notification created, *Test* sent, an HTTP monitor going down and up again.

## 1. Create the integration in Statusbar

*Monitoring → Add integration*, name `Uptime Kuma`, policy *Hold for approval*, delay `60`.
Copy the webhook URL `https://status.example.com/api/hooks/sb_…` – it is shown only once.
Details: [Create the integration](../../docs/INTEGRATIONS.md#create-the-integration).

Use one integration per Uptime Kuma instance: keys are built from monitor IDs, which start at 1 in every instance.

## 2. Set up Uptime Kuma

*Settings → Notifications → Setup Notification* (or in a monitor: *Notifications → Setup Notification*):

1. Notification Type: **Webhook**
2. Friendly Name: `Statusbar`
3. Post URL: the webhook URL from step 1
4. Request Body: **Preset – application/json**. *multipart/form-data* does not work (Statusbar answers
   `400 JSON body required`); *Custom Body* only if you know what you change.
5. *Additional Headers*: leave empty. (Alternative that keeps the token out of the URL: Post URL
   `https://status.example.com/api/hooks` and Additional Headers `{"Authorization": "Bearer sb_…"}`.)
6. *Default enabled* – tick it if new monitors should get it automatically; *Apply on all existing
   monitors* only if users should hear about every monitor.
7. *Test*, then *Save*.

Then enable the notification **Statusbar** on the monitors that matter to users (*Edit* monitor →
*Notifications*). Monitors that only interest the admins stay without it.

Worth setting on those monitors: **Retries** `1`–`3` (Uptime Kuma marks a monitor *Pending* first, which
Statusbar ignores) – this keeps short blips away from users together with Statusbar's delay.

## 3. How Uptime Kuma maps to Statusbar

| Uptime Kuma | Statusbar |
|---|---|
| `heartbeat.status` = 0 (Down) | problem, Outage (`error`) |
| `heartbeat.status` = 1 (Up) | recovery: message resolved |
| 2 (Pending), 3 (Maintenance) | ignored |
| `monitor.id` | key `kuma-<id>` |
| `monitor.name` | title and host; service = monitor type (`http`, `ping`, …) |
| `heartbeat.msg` (e.g. `timeout of 48000ms exceeded`) | message text without rule |

Uptime Kuma only notifies on state changes (and, if set, *Resend Notification if Down X times
consecutively* – answered with `duplicate`).

## 4. Test

**Test button** – Uptime Kuma shows *Sent Successfully.* The test contains no monitor and no state, so
Statusbar answers `{"events":[],"ok":true}` and nothing appears. That still proves URL, token and network.

**Sample payloads** – a full cycle:

```bash
URL='https://status.example.com/api/hooks/<token>'
curl -sS -X POST "$URL" -H 'Content-Type: application/json' --data-binary @test-down.json
# {"events":["draft"],"ok":true}
curl -sS -X POST "$URL" -H 'Content-Type: application/json' --data-binary @test-up.json
# {"events":["cancelled"],"ok":true}
```

**End to end** – add a monitor (type *HTTP(s)*) to an address that does not answer, e.g.
`http://192.0.2.1/`, Heartbeat Interval `20`, Retries `0`, notification *Statusbar*. After the first failed
check the message appears; change the URL to a working one and it is resolved with the next heartbeat.

## 5. Example rule

In Statusbar (*Monitoring → Add rule*):

| Field | Value |
|---|---|
| When host/service matches | `Intranet/*` |
| Show this title | `The intranet is not reachable` |
| And this message | `We are working on it.` |
| Severity | Outage |

The subject is `<monitor name>/<monitor type>`. To target a single monitor regardless of its name, use
its key: `kuma-7` (the number is in the monitor's address, `/dashboard/7`).

## Troubleshooting

| Symptom | Fix |
|---|---|
| Test: `Request failed with status code 401` | URL incomplete or integration deleted. |
| Test: `Request failed with status code 400` | Request Body is not *application/json*. |
| Test: `getaddrinfo ENOTFOUND` / `ECONNREFUSED` | The Uptime Kuma host/container cannot reach Statusbar (Docker network, DNS, proxy). |
| Test: `unable to verify the first certificate` / `self-signed certificate` | Uptime Kuma (Node.js) does not trust your CA. For Docker: mount the root CA and set `NODE_EXTRA_CA_CERTS=/path/to/root-ca.pem`. |
| Test works, but nothing appears when a monitor goes down | The notification is not enabled on that monitor, or the monitor is *Pending* (retries) or in maintenance. |
| Message stays | The monitor was paused or deleted while down – paused monitors send no *Up*. *Resolve* it in Statusbar. |
