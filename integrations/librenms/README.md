# Statusbar for LibreNMS

A LibreNMS alert – a switch, firewall, UPS or server going down – opens a message on your users'
desktops; when the alert clears, LibreNMS sends the recovery and the message gets its green all-clear.
LibreNMS's built-in **API** transport does it, with a JSON body you paste in.

| File | Purpose |
|---|---|
| [`api-body.json`](api-body.json) | Body for the API transport (LibreNMS template syntax) |

Tested with LibreNMS 26.11: a ping-only device going down and up again under the rule *Devices up/down*,
including device and rule names with quotes and backslashes, and the transport's *Test* button.

## 1. Create the integration in Statusbar

*Monitoring → Add integration*, name `LibreNMS`, policy *Hold for approval*, delay `60` (`0` while testing).
Copy the webhook URL `https://status.example.com/api/hooks/sb_…` – it is shown only once. You need the token
part (`sb_…`) in step 2.
Details: [Create the integration](../../docs/INTEGRATIONS.md#create-the-integration).

## 2. Set up LibreNMS

### 2.1 Alert transport

*Alerts → Alert Transports → Create alert transport*:

| Field | Value |
|---|---|
| Transport name | `Statusbar` |
| Transport type | **API** |
| Default alert | off – only the rules you pick should reach users |
| API Method | `POST` |
| API URL | `https://status.example.com/api/hooks` |
| Options | empty |
| Headers | two lines: `Content-Type=application/json` and `Authorization=Bearer sb_…` |
| Body | the content of [`api-body.json`](api-body.json) |
| Send as form | off |
| Username / Password | empty |

```
{
  "key": "librenms-{{ $alert_id }}",
  "status": "{{ $state }}",
  "host": {{ $sysName|json_encode }},
  "service": {{ $name|json_encode }},
  "message": {{ $title|json_encode }},
  "severity": "{{ $severity }}"
}
```

`|json_encode` writes the value with its quotes and escapes `"` and `\` – so do **not** put quotes
around those placeholders. It needs a LibreNMS version whose templates support filters (tested with
26.11). On older versions use `"{{ $sysName }}"` etc. and keep quotes out of device and rule names.

The header keeps the token out of LibreNMS's logs. Alternative: API URL with the token
(`…/api/hooks/sb_…`) and only the `Content-Type` header.

### 2.2 Send the right rules to Statusbar

**LibreNMS 26 and later** (*Alerts → Operations* exists):

1. *Alerts → Operations → Create operation*: name `Statusbar`, add a segment with **Transports**
   `Statusbar` (start `0`, steps `1` to `1`). Leave *Muted* off.
2. Edit each alert rule users should hear about: **Operation** `Statusbar`.

  Only one operation per rule: if on-call already gets the rule via another operation, add the Statusbar
  transport to that operation's segment instead.

**Older versions**: edit the alert rule and add `Statusbar` under **Transports**.

Keep *Recovery alerts* on in the rule (the default): the recovery takes the message down.
Rules that suit users: *Devices up/down*, *Port status up/down* on uplinks, *Service up/down*, UPS on battery.

## 3. How LibreNMS maps to Statusbar

| LibreNMS | Statusbar |
|---|---|
| `$state` `1` (alert) | problem: message opens |
| `$state` `0` (recovered) | recovery: message resolved |
| `$state` `2` (acknowledged), `3` (worse), `4` (better), `5` (changed) | ignored |
| `$alert_id` | key `librenms-<id>` – one per rule and device, identical in alert and recovery |
| `$severity` `critical` / `warning` | Outage / Degraded |
| `$sysName` / rule name `$name` | host / service; title `rule (sysName)` |
| `$title` (from the alert template) | message text without rule |

Use `$hostname` instead of `$sysName` if that is the name your users know.

## 4. Test

**Test button** – *Alerts → Alert Transports*, *Test* next to `Statusbar`. LibreNMS sends a fake alert for
rule `Test-Rule` with key `librenms-000` and shows *Test successful*. Statusbar creates a draft; LibreNMS
never sends a recovery for it, so *Discard* it under *Messages → Waiting for approval*.

**Without LibreNMS** – the body with values filled in:

```bash
URL='https://status.example.com/api/hooks/<token>'
curl -sS -X POST "$URL" -H 'Content-Type: application/json' \
  -d '{"key":"librenms-1","status":"1","host":"sw-core01","service":"Devices up/down","severity":"critical"}'
# {"events":["draft"],"ok":true}
curl -sS -X POST "$URL" -H 'Content-Type: application/json' \
  -d '{"key":"librenms-1","status":"0","host":"sw-core01","service":"Devices up/down","severity":"critical"}'
# {"events":["cancelled"],"ok":true}
```

**End to end** – add a ping-only device for an address that does not answer (*Devices → Add Device*,
*SNMP* off, *Force add*), rule *Devices up/down* with operation `Statusbar`. After the next poll the
message appears; delete the device or point it at a reachable address to see the recovery.

## 5. Example rule

In Statusbar (*Monitoring → Add rule*):

| Field | Value |
|---|---|
| When host/service matches | `fw-*/Devices up/down` |
| Show this title | `Internet access is down` |
| And this message | `We are working on it. Internal systems are not affected.` |
| Severity | Outage |

The subject is `<sysName>/<rule name>`, e.g. `fw-site1/Devices up/down`.

## Troubleshooting

To see what a transport did, run the alerting by hand as the librenms user:
`cd /opt/librenms && ./alerts.php -d` (prints `Transport api => OK` or the HTTP error).

| Symptom | Fix |
|---|---|
| Transport test: `401` | Token wrong or integration deleted; the header must read `Authorization=Bearer sb_…`. |
| Transport test: `400 JSON body required` | Quotes added around a `\|json_encode` placeholder, a placeholder that is not filled, or *Send as form* is on. |
| `Muted Alert-UID` in `alerts.php -d` (26+) | The rule has no operation, or the operation is muted or has no segment with a transport. |
| Message never resolves | *Recovery alerts* off in the rule, or the rule was deleted/disabled while the alert was active. *Resolve* it in Statusbar. |
| `cURL error 60: SSL certificate problem` | Add your internal root CA to the LibreNMS host (container: mount it and update the CA store). |
| Every poll sends again | Not expected: further alerts with the same `$alert_id` are answered with `duplicate`. Check that the body uses `$alert_id`, not `$uid` (`$uid` changes with every alert). |
