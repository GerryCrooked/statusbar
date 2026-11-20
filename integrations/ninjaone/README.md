# Statusbar for NinjaOne

> **Status: in development.** This integration is being tested against a real NinjaOne tenant. Settings and
> payload handling may still change; feedback and captured payloads are welcome.

A NinjaOne condition – a server offline, a service stopped, a disk full – opens a message on your users'
desktops; when the condition resets, the message gets its green all-clear. Statusbar reads NinjaOne's
webhook format (its *activity* objects) as it is. NinjaOne itself can only reach end users through
scripts that pop up a toast; Statusbar shows the status to everyone it concerns.

| File | Purpose |
|---|---|
| [`test-triggered.json`](test-triggered.json), [`test-reset.json`](test-reset.json) | A condition triggering and resetting, for testing with `curl` |

**Not tested against a NinjaOne tenant.** The payload follows NinjaOne's published activity schema;
NinjaOne publishes no sample delivery. If a real condition does not behave as described below, please
send the JSON body you received (Troubleshooting shows how to find it) so the parser can be adjusted.

## 1. Create the integration in Statusbar

*Monitoring → Add integration*, name `NinjaOne`, policy *Hold for approval*, delay `60` (`0` while testing).
Copy the webhook URL `https://status.example.com/api/hooks/sb_…` – it is shown only once.
Details: [Create the integration](../../docs/INTEGRATIONS.md#create-the-integration).

NinjaOne sends from its cloud: Statusbar must be reachable from the internet over HTTPS with a publicly
trusted certificate.

## 2. Set up NinjaOne

### 2.1 Notification channel (recommended)

A notification channel is only used by the conditions you attach it to, which is what you want for users.

1. *Administration → Apps → Notification Channels → Add* (new UI), type **Webhook**.
2. Name `Statusbar`, URL: the webhook URL from step 1. NinjaOne cannot add headers here, so the token
   stays in the URL.
3. Save.

Then, in the policies whose devices matter to users (*Administration → Policies →* policy *→ Conditions*),
edit each condition users should hear about – e.g. *Device down*, *Windows service* for your line-of-business
service, *Disk free space* on file servers:

* **Notification channels**: add `Statusbar`.
* Severity and priority as usual – Statusbar maps them (section 3).
* **Auto-reset**: on. The reset is what takes the message down; a condition reset by hand
  or never reset leaves the Statusbar message open.

Notification channel deliveries do **not** include the device name – only its ID. The title therefore
comes from the condition (`subject` / `sourceName`), and rules match on the key (section 5).

### 2.2 API webhook (alternative, with device names)

NinjaOne's API webhook (`PUT /v2/webhook`, system administrator API client) can add the device object and
an authorization header, but sends the activities of *all* devices of the chosen organizations.
Statusbar ignores everything that is not a condition, but every condition would reach it – use this
only together with rules and *Ignore* as policy for alerts without a rule.

```bash
curl -sS -X PUT 'https://eu.ninjarmm.com/v2/webhook' -H "Authorization: Bearer $NINJA_API_TOKEN" \
  -H 'Content-Type: application/json' -d '{
    "url": "https://status.example.com/api/hooks",
    "activities": {"CONDITION": ["*"]},
    "expand": ["device"],
    "headers": [{"name": "Authorization", "value": "Bearer sb_…"}]
  }'
# 204 No Content
```

Use your region's host (`app.ninjarmm.com`, `eu.ninjarmm.com`, `oc.ninjarmm.com`, …).

## 3. How NinjaOne maps to Statusbar

| NinjaOne | Statusbar |
|---|---|
| `activityType` = `CONDITION`, `statusCode` = `TRIGGERED` | problem: message opens |
| `activityType` = `CONDITION`, `statusCode` = `RESET` | recovery: message resolved |
| any other activity (patching, scripts, tickets …) | ignored |
| `deviceId` + `sourceConfigUid` (the condition) | key `ninja-<device id>-<condition uid>` – identical for trigger and reset |
| `severity` `CRITICAL`, `MAJOR` / `MODERATE`, `MINOR` / `NONE` | Outage / Degraded / Notice (without severity: `priority` `HIGH` / `MEDIUM` / `LOW`) |
| `subject`, else `sourceName (device)` | title |
| `message` | message text without rule |
| `device.displayName` or `systemName` (API webhook with `expand`), else `deviceId` | host |
| `sourceName` | service |

## 4. Test

NinjaOne's *Test* on a notification channel, if offered, sends a message without a condition state;
Statusbar answers `{"events":[],"ok":true}` – that proves URL, token and network.

**Sample payloads**:

```bash
URL='https://status.example.com/api/hooks/<token>'
curl -sS -X POST "$URL" -H 'Content-Type: application/json' --data-binary @test-triggered.json
# {"events":["draft"],"ok":true}
curl -sS -X POST "$URL" -H 'Content-Type: application/json' --data-binary @test-reset.json
# {"events":["cancelled"],"ok":true}
```

**End to end** – on a test device, a condition *Disk free space less than 99 %* with auto-reset and channel
`Statusbar`. It triggers within minutes; remove the condition or raise the threshold and it resets.

## 5. Example rules

In Statusbar (*Monitoring → Add rule*):

| When host/service matches | Show this title | And this message |
|---|---|---|
| `*/File server offline*` | `File server not reachable` | `Network drives are not available. We are working on it.` |
| `ninja-42-*` | `Problem with the accounting server` | `{message}` |

Write the condition names in NinjaOne for people (`File server offline`) – they become title and service.
`ninja-42-*` matches every condition of the device with ID 42 (the number in the device's URL in NinjaOne).

## Troubleshooting

| Symptom | Fix |
|---|---|
| Nothing arrives | The channel is not attached to the condition, or the policy is not the device's policy. |
| Conditions trigger, NinjaOne delivers, but no message or draft appears | The delivery does not look like a condition activity to Statusbar. Capture the JSON body (e.g. in your reverse proxy's request log, or with a request inspector you control) and send it in. |
| Message never resolves | Auto-reset is off for the condition, or it was reset by hand. *Resolve* it in Statusbar. |
| `401` | URL incomplete or integration deleted. |
| Many unexpected drafts | API webhook: it sends all conditions. Switch to a notification channel or set the integration's policy to *Ignore* and use rules. |
