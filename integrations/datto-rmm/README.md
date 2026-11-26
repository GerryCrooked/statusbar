# Notiflag for Datto RMM

A Datto RMM monitor alert – a server offline, a service stopped, a disk full – opens a message on your
users' desktops; when Datto RMM resolves the alert automatically, the message gets its green all-clear.
Datto RMM's monitor **webhooks** do it with a JSON payload you paste in. Datto RMM itself has no way to
show such status messages to end users.

| File | Purpose |
|---|---|
| [`alert-raised.json`](alert-raised.json) | *Alert Raised Payload* |
| [`alert-resolved.json`](alert-resolved.json) | *Alert Resolved Payload* |

Tested by sending both payloads, with the Datto RMM variables filled in, to a throwaway Notiflag server
(`scripts/test-integrations.sh` does the same). **Not tested in a Datto RMM account.** The settings below
follow the Datto RMM help on webhooks.

## 1. Create the integration in Notiflag

*Monitoring → Add integration*, name `Datto RMM`, policy *Hold for approval*, delay `60` (`0` while testing).
Copy the webhook URL `https://status.example.com/api/hooks/sb_…` – it is shown only once.
Details: [Create the integration](../../docs/INTEGRATIONS.md#create-the-integration).

Datto RMM sends from the internet: Notiflag must be reachable over HTTPS with a publicly trusted certificate.

## 2. Set up Datto RMM

Configure the webhook **on the monitors users should hear about**, not as a global webhook – a global
webhook would send every alert of every device.

Open the monitor (*Policies → Monitoring →* policy *→* monitor, or *Add Monitor*), section **Response**:

1. **Webhook**: on.
2. **URL**: the webhook URL from step 1 (Datto RMM cannot set an authorization header).
3. **Content Type**: `application/json`.
4. **When Alert is Triggered** – *Alert Raised Payload*:
   ```json
   {"key": "datto-[alert_uid]", "status": "problem", "host": "[device_hostname]", "service": "Online status", "severity": "error"}
   ```
5. **When Alert is Resolved**: on – *Alert Resolved Payload*:
   ```json
   {"key": "datto-[alert_uid]", "status": "ok", "host": "[device_hostname]", "service": "Online status"}
   ```
6. *Test Webhook*, then save the monitor (and push the policy).

Adjust per monitor:

* `service` – what the monitor watches, in words: `Online status`, `Print spooler`, `Disk C:`.
* `severity` – `error` (Outage), `warning` (Degraded) or `info` (Notice).
* Optional `"message": "[alert_message]"` adds Datto RMM's alert text. Datto RMM inserts variables as they
  are; a message containing `"` or `\` (event log texts, file paths) then breaks the JSON and Notiflag
  answers `400`. Prefer a rule in Notiflag for the text users read.

Keep **auto-resolve** on for the monitor: Datto RMM sends the resolved payload **only when it resolves the
alert automatically**. An alert resolved by hand in Datto RMM leaves the Notiflag message open – resolve
it in Notiflag too.

## 3. How Datto RMM maps to Notiflag

| Datto RMM | Notiflag |
|---|---|
| Alert Raised Payload, `"status": "problem"` | problem: message opens |
| Alert Resolved Payload, `"status": "ok"` | recovery: message resolved |
| `[alert_uid]` | key `datto-<alert uid>` – the same in raised and resolved |
| `[device_hostname]` | host |
| fixed `service` per monitor | service; title `service (host)` |
| fixed `severity` per monitor | Outage / Degraded / Notice |

Other useful variables: `[sitename]` (customer site), `[device_description]`, `[alert_type]`,
`[alert_category]`. Device descriptions and site names may contain quotes – the same caveat as for
`[alert_message]` applies.

## 4. Test

**Test Webhook** in the monitor sends the raised payload and, if configured, the resolved payload right
after it. Notiflag answers `draft` and then `cancelled` – nothing stays behind. Datto RMM shows a
confirmation or the error (URL, content type, payload or the answer from Notiflag).

Sent payloads with the values filled in and any error are listed in the alert's **Timeline** card
(*Single Alert View*).

**Without Datto RMM**:

```bash
URL='https://status.example.com/api/hooks/<token>'
sed 's/\[alert_uid\]/test-1/; s/\[device_hostname\]/SRV-FS01/' alert-raised.json |
  curl -sS -X POST "$URL" -H 'Content-Type: application/json' --data-binary @-
# {"events":["draft"],"ok":true}
sed 's/\[alert_uid\]/test-1/; s/\[device_hostname\]/SRV-FS01/' alert-resolved.json |
  curl -sS -X POST "$URL" -H 'Content-Type: application/json' --data-binary @-
# {"events":["cancelled"],"ok":true}
```

**End to end** – an *Online Status Monitor* on a test device, offline after 5 minutes, with the webhook.
Shut the device down; when it is back online, Datto RMM resolves the alert.

## 5. Example rule

In Notiflag (*Monitoring → Add rule*):

| Field | Value |
|---|---|
| When host/service matches | `SRV-FS*/Online status` |
| Show this title | `File server not reachable` |
| And this message | `Network drives are not available. We are working on it.` |
| Severity | As reported |

The subject is `<hostname>/<service from the payload>`.

## Troubleshooting

| Symptom | Fix |
|---|---|
| Timeline shows `401` | URL incomplete or integration deleted. |
| Timeline shows `400` | Invalid JSON after the variables were filled in – usually a quote in `[alert_message]`, `[device_description]` or `[sitename]`. Remove that variable. |
| Message never resolves | *When Alert is Resolved* off, the alert was resolved by hand, or the monitor has no auto-resolve. |
| Every device's alerts arrive | A global webhook (*Setup → Integrations → Global Webhooks*) is on. Use per-monitor webhooks. |
| Test fails with an SSL or connection error | Notiflag is not reachable from the internet or its certificate is not publicly trusted. |
